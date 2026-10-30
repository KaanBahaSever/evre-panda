//! Voice for bots (feature `voice`): join a voice channel, publish one
//! stereo 48 kHz music track, push PCM, listen to others, leave.
//!
//! Joining goes through the server's voice route
//! (`POST /v1/channels/{channel}/voice/join`), which checks `CONNECT` and
//! the plan and returns a LiveKit URL and token plus the plan's music
//! bitrate for bots. The track is published as the bot's main audio
//! (`microphone` source, needs `SPEAK`) with the music profile of
//! architecture §6.3: stereo, Opus `audio` mode (libwebrtc picks it for two
//! channels), no DTX, RED on, at most the server's music bitrate.
//!
//! PCM is interleaved stereo `i16` at 48 kHz. [`AudioSink::push`] takes any
//! whole number of sample pairs and returns once the audio is queued; the
//! queue holds ~100–200 ms and drains in real time, so a decode loop that
//! pushes as fast as `push` returns is paced automatically. When nothing is
//! pushed the track sends silence.
//!
//! ```no_run
//! use era_bot_sdk::voice::{FRAME_SAMPLES, VoiceConnection, VoiceOptions};
//! # async fn play(bot: &era_bot_sdk::Bot, channel: era_bot_sdk::protocol::Snowflake) -> era_bot_sdk::Result<()> {
//! let voice = VoiceConnection::join(bot, channel, VoiceOptions::default()).await?;
//! let silence = vec![0_i16; FRAME_SAMPLES];
//! for _ in 0..100 {
//!     voice.sink().push(&silence).await?; // 10 ms each
//! }
//! voice.leave().await;
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use livekit::options::{AudioEncoding, TrackPublishOptions};
use livekit::prelude::*;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::AudioSourceOptions;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::peer_connection_factory::IceServer as RtcIceServer;
use protocol::Snowflake;
use protocol::rest::{VoiceGrants, VoiceJoinRequest, VoiceSource};
use tokio::task::JoinHandle;

use crate::client::Bot;
use crate::error::{Error, Result};
use crate::rest::install_crypto_provider;

/// Sample rate of every PCM buffer.
pub const SAMPLE_RATE: u32 = 48_000;
/// Channels of every PCM buffer (interleaved left, right).
pub const CHANNELS: u32 = 2;
/// Samples (both channels) in 10 ms.
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE / 100 * CHANNELS) as usize;
/// The music profile's top bitrate (§6.3); used when the plan allows more
/// and the bot asks for nothing else.
pub const DEFAULT_MUSIC_BITRATE_KBPS: u32 = 256;
/// Lowest bitrate the SDK publishes at.
const MIN_BITRATE_KBPS: u32 = 16;
/// Audio queued ahead of the encoder (the source keeps up to twice this).
const QUEUE_MS: u32 = 100;
/// How long [`VoiceConnection::listen`] waits for the other track.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(10);

/// Checks that the media stack works on this machine without any server: a
/// track is created (which starts libwebrtc), a 440 Hz tone is pushed
/// through its source and must come out of the track again. Returns the
/// tone's level in dBFS. For `--check` options of bots and installers.
pub async fn self_check() -> Result<f32> {
    use futures::StreamExt as _;

    install_crypto_provider();
    let source = NativeAudioSource::new(
        AudioSourceOptions::default(),
        SAMPLE_RATE,
        CHANNELS,
        QUEUE_MS,
    );
    let track =
        LocalAudioTrack::create_audio_track("self-check", RtcAudioSource::Native(source.clone()));
    let mut stream = NativeAudioStream::new(track.rtc_track(), SAMPLE_RATE as i32, CHANNELS as i32);
    let sink = AudioSink {
        source,
        lock: Arc::new(tokio::sync::Mutex::new(())),
    };
    let feeder = tokio::spawn(async move {
        let mut phase = 0.0_f32;
        let mut frame = vec![0_i16; FRAME_SAMPLES];
        for _ in 0..100 {
            for [left, right] in frame.as_chunks_mut::<2>().0 {
                #[allow(clippy::cast_possible_truncation)]
                let sample = (phase.sin() * 8000.0) as i16;
                (*left, *right) = (sample, sample);
                phase = (phase + std::f32::consts::TAU * 440.0 / SAMPLE_RATE as f32)
                    % std::f32::consts::TAU;
            }
            if sink.push(&frame).await.is_err() {
                return;
            }
        }
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut loudest = f32::NEG_INFINITY;
    while tokio::time::Instant::now() < deadline {
        let Ok(Some(frame)) = tokio::time::timeout(Duration::from_secs(2), stream.next()).await
        else {
            break;
        };
        let power = frame
            .data
            .iter()
            .map(|&s| (f32::from(s) / 32768.0).powi(2))
            .sum::<f32>()
            / frame.data.len().max(1) as f32;
        loudest = loudest.max(10.0 * power.max(1e-12).log10());
        if loudest > -30.0 {
            break;
        }
    }
    feeder.abort();
    if loudest > -30.0 {
        Ok(loudest)
    } else {
        Err(Error::Voice(format!(
            "the audio pipeline returned no tone (loudest {loudest:.1} dBFS)"
        )))
    }
}

/// Options for [`VoiceConnection::join`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct VoiceOptions {
    /// Wanted music bitrate (kbit/s). Capped by the server's limit for the
    /// guild's plan; `None` means [`DEFAULT_MUSIC_BITRATE_KBPS`] (capped).
    pub bitrate_kbps: Option<u32>,
    /// Join undeafened so [`VoiceConnection::listen`] can receive others.
    /// Off by default: a music bot needs no one's audio.
    pub listen: bool,
    /// Name of the published track.
    pub track_name: String,
}

impl Default for VoiceOptions {
    fn default() -> Self {
        Self {
            bitrate_kbps: None,
            listen: false,
            track_name: "music".into(),
        }
    }
}

impl VoiceOptions {
    /// Sets [`VoiceOptions::bitrate_kbps`].
    #[must_use]
    pub fn bitrate_kbps(mut self, kbps: u32) -> Self {
        self.bitrate_kbps = Some(kbps);
        self
    }

    /// Sets [`VoiceOptions::listen`].
    #[must_use]
    pub fn listen(mut self, listen: bool) -> Self {
        self.listen = listen;
        self
    }
}

/// The bitrate to publish at: the wish (or the default), capped by the
/// server's music limit for bots, or by the channel's voice limit from a
/// server that sends no music limit.
#[must_use]
pub fn music_bitrate_kbps(grants: &VoiceGrants, wanted: Option<u32>) -> u32 {
    let cap = grants
        .max_music_bitrate_kbps
        .unwrap_or(grants.max_bitrate_kbps);
    wanted
        .unwrap_or(DEFAULT_MUSIC_BITRATE_KBPS)
        .min(cap)
        .max(MIN_BITRATE_KBPS)
}

/// Where PCM goes: the published track's source. Cheap to clone, so a
/// decoder task can own one while the connection stays elsewhere.
#[derive(Clone)]
pub struct AudioSink {
    source: NativeAudioSource,
    /// The source takes one capture at a time.
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl std::fmt::Debug for AudioSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioSink").finish_non_exhaustive()
    }
}

impl AudioSink {
    /// Queues interleaved stereo 48 kHz PCM (any whole number of sample
    /// pairs) and returns once it fits in the queue, so pushing in a loop
    /// runs in real time.
    pub async fn push(&self, pcm: &[i16]) -> Result<()> {
        if pcm.is_empty() {
            return Ok(());
        }
        if !pcm.len().is_multiple_of(CHANNELS as usize) {
            return Err(Error::Voice(
                "PCM must hold whole stereo sample pairs".into(),
            ));
        }
        let frame = AudioFrame {
            data: Cow::Borrowed(pcm),
            sample_rate: SAMPLE_RATE,
            num_channels: CHANNELS,
            samples_per_channel: u32::try_from(pcm.len() / CHANNELS as usize)
                .map_err(|_| Error::Voice("PCM buffer too large".into()))?,
        };
        let _one_at_a_time = self.lock.lock().await;
        // A capture abandoned mid-way (its task was cancelled, e.g. on skip)
        // keeps the source busy until the next 10 ms tick: wait for it.
        let mut attempts = 0;
        loop {
            match self.source.capture_frame(&frame).await {
                Ok(()) => return Ok(()),
                Err(_) if attempts < 10 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(Error::Voice(error.message)),
            }
        }
    }

    /// Drops the queued audio (on skip or stop, so the old track does not
    /// play on for 100–200 ms).
    pub fn clear(&self) {
        self.source.clear_buffer();
    }
}

/// Received audio of another participant: 10 ms frames of interleaved
/// stereo 48 kHz PCM.
pub struct AudioReceiver {
    stream: NativeAudioStream,
    publication: RemoteTrackPublication,
}

impl std::fmt::Debug for AudioReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioReceiver").finish_non_exhaustive()
    }
}

impl AudioReceiver {
    /// The next frame; `None` when the track ended.
    pub async fn recv(&mut self) -> Option<Vec<i16>> {
        use futures::StreamExt as _;
        self.stream
            .next()
            .await
            .map(|frame| frame.data.into_owned())
    }
}

impl Drop for AudioReceiver {
    fn drop(&mut self) {
        self.publication.set_subscribed(false);
    }
}

/// The bot in one voice channel.
pub struct VoiceConnection {
    room: Arc<Room>,
    track: LocalAudioTrack,
    sink: AudioSink,
    guild_id: Snowflake,
    channel_id: Snowflake,
    bitrate_kbps: u32,
    disconnected: Arc<AtomicBool>,
    events: Option<JoinHandle<()>>,
    left: bool,
}

impl std::fmt::Debug for VoiceConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoiceConnection")
            .field("guild_id", &self.guild_id)
            .field("channel_id", &self.channel_id)
            .field("bitrate_kbps", &self.bitrate_kbps)
            .finish_non_exhaustive()
    }
}

impl VoiceConnection {
    /// Joins a voice channel and publishes the music track (silent until
    /// PCM arrives). Fails with `Api(Forbidden)` without `CONNECT`,
    /// `Api(PlanLimitReached)` when the guild's voice slots are full, and
    /// `Voice` without `SPEAK`.
    pub async fn join(bot: &Bot, channel_id: Snowflake, options: VoiceOptions) -> Result<Self> {
        let join = bot
            .rest()
            .join_voice(
                channel_id,
                &VoiceJoinRequest {
                    self_mute: false,
                    self_deaf: !options.listen,
                    replace_other_device: false,
                },
            )
            .await?;
        if !join
            .grants
            .can_publish_sources
            .contains(&VoiceSource::Microphone)
        {
            return Err(Error::Voice(
                "the bot may not speak in this channel (SPEAK)".into(),
            ));
        }
        let guild_id = match bot.cache().guild_of_channel(channel_id) {
            Some(guild_id) => guild_id,
            None => bot.rest().channel(channel_id).await?.guild_id,
        };
        let bitrate_kbps = music_bitrate_kbps(&join.grants, options.bitrate_kbps);

        let mut room_options = RoomOptions::default();
        // Nothing is received unless asked for (`listen`).
        room_options.auto_subscribe = false;
        room_options.rtc_config.ice_servers = join
            .ice_servers
            .iter()
            .map(|server| RtcIceServer {
                urls: server.urls.clone(),
                username: server.username.clone().unwrap_or_default(),
                password: server.credential.clone().unwrap_or_default(),
            })
            .collect();
        install_crypto_provider();
        let (room, mut room_events) = Room::connect(&join.url, join.token.expose(), room_options)
            .await
            .map_err(|error| Error::Voice(format!("connecting to the room failed: {error}")))?;
        let room = Arc::new(room);

        // No echo cancellation, noise suppression or AGC: this is music.
        let source = NativeAudioSource::new(
            AudioSourceOptions::default(),
            SAMPLE_RATE,
            CHANNELS,
            QUEUE_MS,
        );
        let track = LocalAudioTrack::create_audio_track(
            &options.track_name,
            RtcAudioSource::Native(source.clone()),
        );
        let publish = TrackPublishOptions {
            source: TrackSource::Microphone,
            dtx: false,
            red: true,
            // Announced as stereo, or the SFU's answer may make it mono.
            stereo: true,
            audio_encoding: Some(AudioEncoding {
                max_bitrate: u64::from(bitrate_kbps) * 1000,
            }),
            ..TrackPublishOptions::default()
        };
        if let Err(error) = room
            .local_participant()
            .publish_track(LocalTrack::Audio(track.clone()), publish)
            .await
        {
            let _ = room.close().await;
            return Err(Error::Voice(format!(
                "publishing the track failed: {error}"
            )));
        }

        let disconnected = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&disconnected);
        let events = tokio::spawn(async move {
            while let Some(event) = room_events.recv().await {
                match event {
                    RoomEvent::Disconnected { reason } => {
                        log::info!("voice: disconnected ({reason:?})");
                        flag.store(true, Ordering::SeqCst);
                    }
                    RoomEvent::Reconnecting => log::info!("voice: reconnecting"),
                    RoomEvent::Reconnected => log::info!("voice: reconnected"),
                    _ => {}
                }
            }
        });
        log::info!("voice: joined {channel_id} at {bitrate_kbps} kbit/s");
        Ok(Self {
            room,
            track,
            sink: AudioSink {
                source,
                lock: Arc::new(tokio::sync::Mutex::new(())),
            },
            guild_id,
            channel_id,
            bitrate_kbps,
            disconnected,
            events: Some(events),
            left: false,
        })
    }

    /// The voice channel.
    #[must_use]
    pub fn channel_id(&self) -> Snowflake {
        self.channel_id
    }

    /// Its guild.
    #[must_use]
    pub fn guild_id(&self) -> Snowflake {
        self.guild_id
    }

    /// The bitrate the track was published with (kbit/s).
    #[must_use]
    pub fn bitrate_kbps(&self) -> u32 {
        self.bitrate_kbps
    }

    /// Where to push PCM.
    #[must_use]
    pub fn sink(&self) -> AudioSink {
        self.sink.clone()
    }

    /// Pushes PCM; see [`AudioSink::push`].
    pub async fn push(&self, pcm: &[i16]) -> Result<()> {
        self.sink.push(pcm).await
    }

    /// Mutes the track (others see it muted and receive nothing) and drops
    /// queued audio, or unmutes it.
    pub fn set_paused(&self, paused: bool) {
        if paused {
            self.sink.clear();
            self.track.mute();
        } else {
            self.track.unmute();
        }
    }

    /// Whether the room connection is up. LiveKit reconnects on its own
    /// after short drops; after a permanent disconnect, join again.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        !self.disconnected.load(Ordering::SeqCst)
            && self.room.connection_state() != ConnectionState::Disconnected
    }

    /// The other participants in the room (user IDs), as LiveKit sees them
    /// right now.
    #[must_use]
    pub fn participants(&self) -> Vec<Snowflake> {
        let mut ids: Vec<Snowflake> = self
            .room
            .remote_participants()
            .keys()
            .filter_map(|identity| identity.0.parse().ok())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Receives a participant's main audio (their microphone, or another
    /// bot's music). Needs [`VoiceOptions::listen`]. Waits up to 10 s for
    /// the participant and the track to appear.
    pub async fn listen(&self, user_id: Snowflake) -> Result<AudioReceiver> {
        let identity = ParticipantIdentity::from(user_id.to_string());
        let deadline = tokio::time::Instant::now() + LISTEN_TIMEOUT;
        let publication = loop {
            let found = self
                .room
                .remote_participants()
                .get(&identity)
                .and_then(|participant| {
                    participant
                        .track_publications()
                        .into_values()
                        .find(|p| p.source() == TrackSource::Microphone)
                });
            if let Some(publication) = found {
                break publication;
            }
            if tokio::time::Instant::now() > deadline {
                return Err(Error::Voice(format!("{user_id} has no audio in this room")));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        publication.set_subscribed(true);
        loop {
            if let Some(RemoteTrack::Audio(track)) = publication.track() {
                let stream =
                    NativeAudioStream::new(track.rtc_track(), SAMPLE_RATE as i32, CHANNELS as i32);
                return Ok(AudioReceiver {
                    stream,
                    publication,
                });
            }
            if tokio::time::Instant::now() > deadline {
                publication.set_subscribed(false);
                return Err(Error::Voice(format!("{user_id}'s audio did not arrive")));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// The LiveKit room, for anything this API does not cover.
    #[must_use]
    pub fn room(&self) -> &Room {
        &self.room
    }

    /// Leaves the channel.
    pub async fn leave(mut self) {
        self.left = true;
        if let Some(events) = self.events.take() {
            events.abort();
        }
        if let Err(error) = self.room.close().await {
            log::debug!("voice: closing the room: {error}");
        }
        log::info!("voice: left {}", self.channel_id);
    }
}

impl Drop for VoiceConnection {
    fn drop(&mut self) {
        if let Some(events) = self.events.take() {
            events.abort();
        }
        if !self.left
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            let room = Arc::clone(&self.room);
            runtime.spawn(async move {
                let _ = room.close().await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use protocol::AudioProfile;

    use super::*;

    fn grants(music: Option<u32>, voice: u32) -> VoiceGrants {
        VoiceGrants {
            can_subscribe: true,
            can_publish_sources: vec![VoiceSource::Microphone],
            max_bitrate_kbps: voice,
            audio_profile: AudioProfile::Voice,
            force_push_to_talk: false,
            max_music_bitrate_kbps: music,
            can_publish_camera: false,
        }
    }

    #[test]
    fn bitrate_follows_the_servers_limit() {
        // Standard plan: 192 kbit/s, whatever the channel says.
        assert_eq!(music_bitrate_kbps(&grants(Some(192), 64), None), 192);
        // Internal plan allows 510: the profile's 256 unless asked for more.
        assert_eq!(music_bitrate_kbps(&grants(Some(510), 64), None), 256);
        assert_eq!(music_bitrate_kbps(&grants(Some(510), 64), Some(320)), 320);
        assert_eq!(music_bitrate_kbps(&grants(Some(192), 64), Some(96)), 96);
        // An older server without a music limit: the channel's voice cap.
        assert_eq!(music_bitrate_kbps(&grants(None, 96), None), 96);
        // Never absurdly low.
        assert_eq!(
            music_bitrate_kbps(&grants(Some(0), 64), None),
            MIN_BITRATE_KBPS
        );
    }

    #[test]
    fn frame_size() {
        assert_eq!(FRAME_SAMPLES, 960);
    }
}
