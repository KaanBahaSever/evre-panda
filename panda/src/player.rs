//! One guild's player: the voice connection, the queue and the song that
//! is playing.
//!
//! A playback task per song reads the [`Decoder`], applies the volume and
//! pushes PCM into the voice track (paced by the SDK's audio queue). When a
//! song ends the next one starts from its prefetched decoder, so there is
//! no gap. A failing song is tried once more (from where it broke off),
//! then skipped. A supervisor leaves the channel after a while alone or
//! with nothing to play.
//!
//! Each song gets a now-playing panel with buttons ([`panel`]) where it was
//! asked for. A panel task posts it, edits it when what it shows changes
//! (debounced) and takes the buttons off once the song is over.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use era_bot_sdk::components::ActionRow;
use era_bot_sdk::protocol::rest::UpdateMessageRequest;
use era_bot_sdk::protocol::{ApiError, Snowflake};
use era_bot_sdk::voice::{AudioSink, SAMPLE_RATE, VoiceConnection, VoiceOptions};
use era_bot_sdk::{Bot, Error};
use tokio::sync::{Mutex, MutexGuard, Notify};
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::lang::{self, Locale, fill, strings};
use crate::media::{self, Decoder, Pcm, apply_volume};
use crate::panel::{self, View};
use crate::queue::{LoopMode, Queue, QueueFull, Track};
use crate::store::Store;
use crate::text;

/// Prefetch the next song this long before the current one ends.
const PREFETCH_BEFORE_END: Duration = Duration::from_secs(25);
/// A song that broke off later than this is retried from there.
const RESUME_AFTER: Duration = Duration::from_secs(5);
/// Changes within this long become one edit of the panel.
const PANEL_DEBOUNCE: Duration = Duration::from_secs(1);

/// What `!çal` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Queued {
    /// It plays now (the "Şimdi çalıyor" message follows).
    Now,
    /// It waits at this 1-based position.
    At(usize),
}

/// Why something could not be done; the text is the reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub String);

impl Refusal {
    fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}

/// The reply for a failed voice join.
#[must_use]
pub fn join_error(lang: Locale, error: &Error) -> Refusal {
    let s = strings(lang);
    Refusal::new(match error {
        Error::Api(ApiError::Forbidden) => s.join_forbidden,
        Error::Api(ApiError::PlanLimitReached { .. }) => s.join_plan_full,
        Error::Api(ApiError::Validation { .. }) => s.join_channel_full,
        Error::Voice(reason) if reason.contains("SPEAK") => s.join_no_speak,
        other => {
            log::warn!("voice join failed: {other}");
            s.join_failed
        }
    })
}

enum End {
    Finished,
    Failed(String),
}

struct Current {
    track: Track,
    generation: u64,
    task: JoinHandle<()>,
    attempt: u32,
}

struct State {
    voice: Option<VoiceConnection>,
    queue: Queue,
    current: Option<Current>,
    prefetch: Option<(u64, Decoder)>,
    generation: u64,
    /// Where unprompted messages go (the last command's channel).
    text_channel: Option<Snowflake>,
    alone_since: Option<Instant>,
    idle_since: Option<Instant>,
}

/// The now-playing panel message of the current song.
#[derive(Debug, Clone)]
struct PanelMessage {
    channel_id: Snowflake,
    message_id: Snowflake,
    /// The song it is for.
    track_id: u64,
    /// What it shows now.
    content: String,
    components: Vec<ActionRow>,
}

/// What the panel task has to do.
#[derive(Debug, Default)]
struct PanelState {
    message: Option<PanelMessage>,
    /// A song started: post its panel (the old one loses its buttons).
    new: Option<Track>,
    /// Something the panel shows changed.
    dirty: bool,
    /// Shutting down: the task ends.
    closed: bool,
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Inner {
    bot: Bot,
    config: Arc<Config>,
    store: Arc<Store>,
    guild_id: Snowflake,
    state: Mutex<State>,
    volume: AtomicU32,
    paused: AtomicBool,
    unpaused: Notify,
    /// Samples per channel of the current song pushed so far.
    position: AtomicU64,
    panel: std::sync::Mutex<PanelState>,
    panel_wake: Arc<Notify>,
}

/// A guild's player. Cheap to clone.
#[derive(Clone)]
pub struct Player {
    inner: Arc<Inner>,
}

impl Player {
    /// A player for `guild_id` with its saved volume and repeat mode, and
    /// its supervisor.
    #[must_use]
    pub fn new(bot: Bot, config: Arc<Config>, store: Arc<Store>, guild_id: Snowflake) -> Self {
        let settings = store.get(guild_id);
        let mut queue = Queue::new(config.max_queue);
        queue.loop_mode = settings.loop_mode;
        let inner = Arc::new(Inner {
            bot,
            config,
            store,
            guild_id,
            state: Mutex::new(State {
                voice: None,
                queue,
                current: None,
                prefetch: None,
                generation: 0,
                text_channel: None,
                alone_since: None,
                idle_since: None,
            }),
            volume: AtomicU32::new(settings.volume.min(200)),
            paused: AtomicBool::new(false),
            unpaused: Notify::new(),
            position: AtomicU64::new(0),
            panel: std::sync::Mutex::default(),
            panel_wake: Arc::new(Notify::new()),
        });
        tokio::spawn(panel_task(
            Arc::downgrade(&inner),
            Arc::clone(&inner.panel_wake),
        ));
        let weak = Arc::downgrade(&inner);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(5));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let Some(inner) = weak.upgrade() else { return };
                inner.supervise().await;
            }
        });
        Self { inner }
    }

    /// Queues a song, joining `caller_channel` first if Panda is not in
    /// voice (or idle in another channel).
    pub async fn play(
        &self,
        track: Track,
        caller_channel: Option<Snowflake>,
    ) -> Result<Queued, Refusal> {
        let inner = &self.inner;
        let mut state = inner.state.lock().await;
        state.text_channel = Some(track.text_channel);
        let idle = state.current.is_none() && state.queue.is_empty();
        let in_voice = state
            .voice
            .as_ref()
            .is_some_and(VoiceConnection::is_connected);
        match (in_voice, caller_channel) {
            (false, None) => {
                return Err(Refusal::new(strings(inner.config.lang).voice_first_call));
            }
            (false, Some(channel)) => inner.connect(&mut state, channel).await?,
            (true, Some(channel))
                if idle
                    && state
                        .voice
                        .as_ref()
                        .is_some_and(|v| v.channel_id() != channel) =>
            {
                inner.connect(&mut state, channel).await?;
            }
            (true, _) => {}
        }
        if state.current.is_none() {
            state.queue.push_front(track);
            inner.start_next(&mut state);
            return Ok(Queued::Now);
        }
        let position = state.queue.push(track).map_err(|QueueFull(max)| {
            let lang = inner.config.lang;
            Refusal::new(fill(
                strings(lang).queue_full,
                &[("songs", &lang::songs(lang, max))],
            ))
        })?;
        inner.prefetch_if_due(&mut state);
        inner.panel_refresh();
        Ok(Queued::At(position))
    }

    /// `!gel`: joins (or moves to) a channel and plays on. While Panda plays
    /// to another channel, moving it is a playback control like `!çık`: only
    /// someone listening there may (third security review).
    pub async fn join(&self, channel: Snowflake, text_channel: Snowflake) -> Result<bool, Refusal> {
        let inner = &self.inner;
        let mut state = inner.state.lock().await;
        let idle = state.current.is_none() && state.queue.is_empty();
        let connected_to = state
            .voice
            .as_ref()
            .filter(|voice| voice.is_connected())
            .map(VoiceConnection::channel_id);
        match connected_to {
            Some(current) if current == channel => {
                state.text_channel = Some(text_channel);
                return Ok(false);
            }
            Some(_) if !idle => {
                return Err(Refusal::new(text::not_in_channel(inner.config.lang)));
            }
            _ => {}
        }
        state.text_channel = Some(text_channel);
        inner.connect(&mut state, channel).await?;
        inner.start_next(&mut state);
        inner.panel_refresh();
        Ok(true)
    }

    /// `!geç`: the next song. `None` when nothing played.
    pub async fn skip(&self) -> Option<Track> {
        let inner = &self.inner;
        let mut state = inner.state.lock().await;
        let current = inner.stop_current(&mut state)?;
        let skipped = current.clone();
        if state.queue.loop_mode == LoopMode::All {
            state.queue.finished(current);
        }
        inner.start_next(&mut state);
        inner.panel_refresh();
        Some(skipped)
    }

    /// `!dur`: clears the queue, stops and leaves.
    pub async fn stop(&self) -> bool {
        let inner = &self.inner;
        let mut state = inner.state.lock().await;
        let was_active =
            state.current.is_some() || !state.queue.is_empty() || state.voice.is_some();
        inner.stop_current(&mut state);
        state.queue.clear();
        state.prefetch = None;
        inner.disconnect(&mut state).await;
        inner.panel_refresh();
        was_active
    }

    /// `!çık`: leaves; the current song goes back to the front of the queue
    /// so `!gel` carries on.
    pub async fn leave(&self) -> bool {
        let inner = &self.inner;
        let mut state = inner.state.lock().await;
        if state.voice.is_none() {
            return false;
        }
        if let Some(track) = inner.stop_current(&mut state) {
            state.queue.push_front(track);
        }
        inner.disconnect(&mut state).await;
        inner.panel_refresh();
        true
    }

    /// `!duraklat`. `false` when nothing plays or it is already paused.
    pub async fn pause(&self) -> bool {
        let state = self.inner.state.lock().await;
        let (Some(_), Some(voice)) = (&state.current, &state.voice) else {
            return false;
        };
        if self.inner.paused.swap(true, Ordering::SeqCst) {
            return false;
        }
        voice.set_paused(true);
        self.inner.panel_refresh();
        true
    }

    /// `!devam`. `false` when nothing is paused.
    pub async fn resume(&self) -> bool {
        let state = self.inner.state.lock().await;
        if !self.inner.paused.swap(false, Ordering::SeqCst) {
            return false;
        }
        if let Some(voice) = &state.voice {
            voice.set_paused(false);
        }
        self.inner.unpaused.notify_waiters();
        self.inner.panel_refresh();
        true
    }

    /// Whether the song is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(Ordering::SeqCst)
    }

    /// `!ses`: sets (and saves) or reads the volume.
    pub fn volume(&self, percent: Option<u32>) -> u32 {
        if let Some(percent) = percent {
            let percent = percent.min(200);
            self.inner.volume.store(percent, Ordering::SeqCst);
            self.inner
                .store
                .update(self.inner.guild_id, |settings| settings.volume = percent);
            self.inner.panel_refresh();
        }
        self.inner.volume.load(Ordering::SeqCst)
    }

    /// `!karıştır`: how many songs were shuffled.
    pub async fn shuffle(&self) -> usize {
        let mut state = self.inner.state.lock().await;
        state.queue.shuffle();
        state.prefetch = None;
        self.inner.prefetch_if_due(&mut state);
        self.inner.panel_refresh();
        state.queue.len()
    }

    /// `!tekrar`: sets the mode, or cycles it without one.
    pub async fn set_loop(&self, mode: Option<LoopMode>) -> LoopMode {
        let mut state = self.inner.state.lock().await;
        let mode = mode.unwrap_or_else(|| state.queue.loop_mode.cycle());
        state.queue.loop_mode = mode;
        self.inner
            .store
            .update(self.inner.guild_id, |settings| settings.loop_mode = mode);
        self.inner.panel_refresh();
        mode
    }

    /// `!sil <#>`.
    pub async fn remove(&self, position: usize) -> Option<Track> {
        let mut state = self.inner.state.lock().await;
        let removed = state.queue.remove(position);
        if removed.is_some() {
            self.inner.prefetch_if_due(&mut state);
            self.inner.panel_refresh();
        }
        removed
    }

    /// `!temizle`: how many songs were removed.
    pub async fn clear(&self) -> usize {
        let mut state = self.inner.state.lock().await;
        let removed = state.queue.len();
        state.queue.clear();
        state.prefetch = None;
        self.inner.panel_refresh();
        removed
    }

    /// The language Panda speaks here.
    #[must_use]
    pub fn lang(&self) -> Locale {
        self.inner.config.lang
    }

    /// `!sıra`.
    pub async fn queue_text(&self) -> String {
        let state = self.inner.state.lock().await;
        text::queue(
            self.lang(),
            &self.inner.config.prefix,
            state.current.as_ref().map(|c| &c.track),
            &state.queue,
        )
    }

    /// `!şimdi`. `None` when nothing plays.
    pub async fn now_text(&self) -> Option<String> {
        let state = self.inner.state.lock().await;
        let current = state.current.as_ref()?;
        Some(text::now(
            self.lang(),
            &current.track,
            self.inner.played(),
            self.inner.paused.load(Ordering::SeqCst),
            self.inner.volume.load(Ordering::SeqCst),
            state.queue.loop_mode,
        ))
    }

    /// The song playing now.
    pub async fn current(&self) -> Option<Track> {
        let state = self.inner.state.lock().await;
        state.current.as_ref().map(|c| c.track.clone())
    }

    /// The voice channel Panda is in.
    pub async fn voice_channel(&self) -> Option<Snowflake> {
        let state = self.inner.state.lock().await;
        state.voice.as_ref().map(VoiceConnection::channel_id)
    }

    /// A moderator moved Panda: follow into the new channel.
    pub async fn moved_to(&self, channel: Snowflake) {
        let inner = &self.inner;
        let mut state = inner.state.lock().await;
        if state
            .voice
            .as_ref()
            .is_none_or(|voice| voice.channel_id() == channel)
        {
            return;
        }
        if let Some(track) = inner.stop_current(&mut state) {
            state.queue.push_front(track);
        }
        if let Err(Refusal(reason)) = inner.connect(&mut state, channel).await {
            inner.say_in(&state, reason);
            return;
        }
        inner.start_next(&mut state);
    }

    /// Whether `message_id` is the panel of the song playing (an older
    /// panel's buttons do nothing).
    #[must_use]
    pub fn is_panel(&self, message_id: Snowflake) -> bool {
        lock(&self.inner.panel)
            .message
            .as_ref()
            .is_some_and(|panel| panel.message_id == message_id)
    }

    /// The text and buttons the panel `message_id` should show now, if it is
    /// the panel of the song playing (a button's answer). `None` once that
    /// song is over: the panel task takes care of it.
    pub async fn panel_now(&self, message_id: Snowflake) -> Option<(String, Vec<ActionRow>)> {
        let current = lock(&self.inner.panel)
            .message
            .clone()
            .filter(|panel| panel.message_id == message_id)?;
        let view = self
            .inner
            .view()
            .await
            .filter(|view| view.track.id == current.track_id)?;
        Some((panel::content(&view), panel::components(&view)))
    }

    /// A button's answer updated the panel to this (so the panel task does
    /// not edit it again).
    pub fn panel_shown(&self, message_id: Snowflake, content: String, components: Vec<ActionRow>) {
        if let Some(panel) = lock(&self.inner.panel)
            .message
            .as_mut()
            .filter(|panel| panel.message_id == message_id)
        {
            panel.content = content;
            panel.components = components;
        }
    }

    /// Stops everything and leaves (shutdown); the panel loses its buttons.
    pub async fn shutdown(&self) {
        self.stop().await;
        let last = {
            let mut panel = lock(&self.inner.panel);
            panel.closed = true;
            panel.new = None;
            panel.message.take()
        };
        self.inner.panel_wake.notify_one();
        if let Some(last) = last {
            self.inner.strip(last).await;
        }
    }
}

/// Posts, edits and closes the guild's panels, one at a time, until the
/// player is gone or shut down.
async fn panel_task(weak: Weak<Inner>, wake: Arc<Notify>) {
    loop {
        wake.notified().await;
        loop {
            let Some(inner) = weak.upgrade() else { return };
            let (new, dirty, closed) = {
                let mut panel = lock(&inner.panel);
                (
                    panel.new.take(),
                    std::mem::take(&mut panel.dirty),
                    panel.closed,
                )
            };
            if closed {
                return;
            }
            if let Some(track) = new {
                inner.post_panel(track).await;
                continue;
            }
            if !dirty {
                break;
            }
            drop(inner);
            tokio::time::sleep(PANEL_DEBOUNCE).await;
            let Some(inner) = weak.upgrade() else { return };
            let next_song = {
                let mut panel = lock(&inner.panel);
                panel.dirty = false;
                panel.new.is_some()
            };
            // A new song's panel shows the latest state anyway.
            if !next_song {
                inner.render_panel().await;
            }
        }
    }
}

impl Inner {
    /// A song started: its panel follows.
    fn panel_new(&self, track: Track) {
        lock(&self.panel).new = Some(track);
        self.panel_wake.notify_one();
    }

    /// Something the panel shows changed (or the song ended).
    fn panel_refresh(&self) {
        lock(&self.panel).dirty = true;
        self.panel_wake.notify_one();
    }

    /// What the panel shows now; `None` when nothing plays.
    async fn view(&self) -> Option<View> {
        let state = self.state.lock().await;
        let current = state.current.as_ref()?;
        Some(View {
            lang: self.config.lang,
            track: current.track.clone(),
            paused: self.paused.load(Ordering::SeqCst),
            volume: self.volume.load(Ordering::SeqCst),
            loop_mode: state.queue.loop_mode,
            queued: state.queue.len(),
        })
    }

    /// Takes the buttons off a panel whose song is over.
    async fn strip(&self, old: PanelMessage) {
        let request = UpdateMessageRequest {
            content: panel::ended(&old.content),
            components: Some(Vec::new()),
        };
        if let Err(error) = self
            .bot
            .rest()
            .edit_message_with(old.channel_id, old.message_id, &request)
            .await
        {
            log::info!("closing the panel failed: {error}");
        }
    }

    /// Closes the old panel and posts the new song's, if it still plays.
    async fn post_panel(&self, track: Track) {
        let old = lock(&self.panel).message.take();
        if let Some(old) = old {
            self.strip(old).await;
        }
        let Some(view) = self.view().await.filter(|view| view.track.id == track.id) else {
            return;
        };
        let content = panel::content(&view);
        let components = panel::components(&view);
        match self
            .bot
            .send_components(track.text_channel, content.clone(), components.clone())
            .await
        {
            Ok(message) => {
                lock(&self.panel).message = Some(PanelMessage {
                    channel_id: message.channel_id,
                    message_id: message.id,
                    track_id: track.id,
                    content,
                    components,
                });
            }
            Err(error) => log::warn!("posting the panel failed: {error}"),
        }
    }

    /// Brings the panel up to date, or closes it once its song is over.
    async fn render_panel(&self) {
        let Some(current) = lock(&self.panel).message.clone() else {
            return;
        };
        match self.view().await {
            Some(view) if view.track.id == current.track_id => {
                let content = panel::content(&view);
                let components = panel::components(&view);
                if content == current.content && components == current.components {
                    return;
                }
                let request = UpdateMessageRequest {
                    content: content.clone(),
                    components: Some(components.clone()),
                };
                match self
                    .bot
                    .rest()
                    .edit_message_with(current.channel_id, current.message_id, &request)
                    .await
                {
                    Ok(_) => {
                        if let Some(panel) = lock(&self.panel)
                            .message
                            .as_mut()
                            .filter(|panel| panel.message_id == current.message_id)
                        {
                            panel.content = content;
                            panel.components = components;
                        }
                    }
                    Err(error) => log::info!("updating the panel failed: {error}"),
                }
            }
            _ => {
                let over = {
                    let mut panel = lock(&self.panel);
                    if panel
                        .message
                        .as_ref()
                        .is_some_and(|panel| panel.message_id == current.message_id)
                    {
                        panel.message.take()
                    } else {
                        None
                    }
                };
                if let Some(over) = over {
                    self.strip(over).await;
                }
            }
        }
    }

    fn played(&self) -> Duration {
        Duration::from_secs_f64(
            self.position.load(Ordering::SeqCst) as f64 / f64::from(SAMPLE_RATE),
        )
    }

    fn say(&self, channel: Snowflake, text: String) {
        let bot = self.bot.clone();
        tokio::spawn(async move {
            if let Err(error) = bot.send_message(channel, text).await {
                log::warn!("sending a message failed: {error}");
            }
        });
    }

    /// A text with the command prefix filled in.
    fn with_prefix(&self, template: &str) -> String {
        fill(template, &[("p", &self.config.prefix)])
    }

    fn say_in(&self, state: &State, text: impl Into<String>) {
        if let Some(channel) = state.text_channel {
            self.say(channel, text.into());
        }
    }

    /// Joins `channel`, leaving any other first.
    async fn connect(
        &self,
        state: &mut MutexGuard<'_, State>,
        channel: Snowflake,
    ) -> Result<(), Refusal> {
        if let Some(track) = self.stop_current(state) {
            state.queue.push_front(track);
        }
        self.disconnect(state).await;
        let mut options = VoiceOptions::default();
        options.bitrate_kbps = self.config.bitrate_kbps;
        let voice = VoiceConnection::join(&self.bot, channel, options)
            .await
            .map_err(|error| join_error(self.config.lang, &error))?;
        log::info!(
            "guild {}: in voice channel {channel} at {} kbit/s",
            self.guild_id,
            voice.bitrate_kbps()
        );
        state.voice = Some(voice);
        state.alone_since = None;
        state.idle_since = None;
        Ok(())
    }

    async fn disconnect(&self, state: &mut MutexGuard<'_, State>) {
        self.paused.store(false, Ordering::SeqCst);
        self.unpaused.notify_waiters();
        state.alone_since = None;
        state.idle_since = None;
        if let Some(voice) = state.voice.take() {
            voice.leave().await;
        }
    }

    /// Ends the current song (its task and queued audio); returns it.
    fn stop_current(&self, state: &mut State) -> Option<Track> {
        let current = state.current.take()?;
        current.task.abort();
        if let Some(voice) = &state.voice {
            voice.sink().clear();
            voice.set_paused(false);
        }
        self.paused.store(false, Ordering::SeqCst);
        self.unpaused.notify_waiters();
        Some(current.track)
    }

    /// Starts the next song if nothing plays.
    fn start_next(self: &Arc<Self>, state: &mut State) {
        if state.current.is_some() || state.voice.is_none() {
            return;
        }
        match state.queue.pop_next() {
            Some(track) => self.start_track(state, track, 0, Duration::ZERO),
            None => {
                state.idle_since.get_or_insert_with(Instant::now);
            }
        }
    }

    fn start_track(
        self: &Arc<Self>,
        state: &mut State,
        track: Track,
        attempt: u32,
        from: Duration,
    ) {
        let Some(voice) = &state.voice else { return };
        let sink = voice.sink();
        let prefetched = attempt == 0
            && state
                .prefetch
                .as_ref()
                .is_some_and(|(id, _)| *id == track.id);
        let decoder = match state.prefetch.take() {
            Some((_, decoder)) if prefetched => Ok(decoder),
            other => {
                // A prefetch for another song stays for later.
                state.prefetch = other;
                Decoder::start(&self.config, &track.source, from)
            }
        };
        state.generation += 1;
        let generation = state.generation;
        self.position.store(
            (from.as_secs_f64() * f64::from(SAMPLE_RATE)) as u64,
            Ordering::SeqCst,
        );
        state.idle_since = None;
        let prefetch_at = track
            .duration
            .filter(|_| !track.live)
            .map(|total| total.saturating_sub(PREFETCH_BEFORE_END));
        let task = match decoder {
            Ok(decoder) => {
                tokio::spawn(Arc::clone(self).playback(generation, decoder, sink, prefetch_at))
            }
            Err(reason) => {
                let inner = Arc::clone(self);
                tokio::spawn(async move { inner.on_end(generation, End::Failed(reason)).await })
            }
        };
        if attempt == 0 {
            self.panel_new(track.clone());
        }
        log::info!(
            "guild {}: playing {:?} (attempt {})",
            self.guild_id,
            track.title,
            attempt + 1
        );
        state.current = Some(Current {
            track,
            generation,
            task,
            attempt,
        });
        if prefetch_at.is_some_and(|at| at.is_zero()) {
            self.prefetch_now(state);
        }
    }

    /// Starts decoding the next song if the current one ends soon.
    fn prefetch_if_due(&self, state: &mut State) {
        let due = state.current.as_ref().is_some_and(|current| {
            current
                .track
                .duration
                .filter(|_| !current.track.live)
                .is_some_and(|total| self.played() + PREFETCH_BEFORE_END >= total)
        });
        if due {
            self.prefetch_now(state);
        }
    }

    fn prefetch_now(&self, state: &mut State) {
        let Some(next) = state.queue.front() else {
            state.prefetch = None;
            return;
        };
        if state
            .prefetch
            .as_ref()
            .is_some_and(|(id, _)| *id == next.id)
        {
            return;
        }
        match Decoder::start(&self.config, &next.source, Duration::ZERO) {
            Ok(decoder) => {
                log::debug!("prefetching {:?}", next.title);
                state.prefetch = Some((next.id, decoder));
            }
            Err(reason) => {
                log::warn!("prefetching {:?} failed: {reason}", next.title);
                state.prefetch = None;
            }
        }
    }

    async fn playback(
        self: Arc<Self>,
        generation: u64,
        mut decoder: Decoder,
        sink: AudioSink,
        prefetch_at: Option<Duration>,
    ) {
        let mut pushed = 0_u64;
        let mut prefetched = false;
        let end = loop {
            match decoder.next().await {
                Pcm::Chunk(mut pcm) => {
                    loop {
                        let unpaused = self.unpaused.notified();
                        if !self.paused.load(Ordering::SeqCst) {
                            break;
                        }
                        unpaused.await;
                    }
                    apply_volume(&mut pcm, self.volume.load(Ordering::SeqCst));
                    if let Err(error) = sink.push(&pcm).await {
                        break End::Failed(error.to_string());
                    }
                    pushed += (pcm.len() / 2) as u64;
                    self.position
                        .fetch_add((pcm.len() / 2) as u64, Ordering::SeqCst);
                    if !prefetched && prefetch_at.is_some_and(|at| self.played() >= at) {
                        prefetched = true;
                        let mut state = self.state.lock().await;
                        if state
                            .current
                            .as_ref()
                            .is_some_and(|c| c.generation == generation)
                        {
                            self.prefetch_now(&mut state);
                        }
                    }
                }
                Pcm::End(Ok(())) if pushed > 0 => break End::Finished,
                Pcm::End(Ok(())) => break End::Failed("empty".into()),
                Pcm::End(Err(reason)) => break End::Failed(reason),
            }
        };
        let inner = Arc::clone(&self);
        tokio::spawn(async move { inner.on_end(generation, end).await });
    }

    async fn on_end(self: Arc<Self>, generation: u64, end: End) {
        let mut state = self.state.lock().await;
        if state
            .current
            .as_ref()
            .is_none_or(|c| c.generation != generation)
        {
            return; // skipped or stopped meanwhile
        }
        let Some(current) = state.current.take() else {
            return;
        };
        match end {
            End::Finished => state.queue.finished(current.track),
            End::Failed(reason)
                if current.attempt == 0 && state.voice.is_some() && media::retryable(&reason) =>
            {
                let played = self.played();
                let from = if played > RESUME_AFTER && !current.track.live {
                    played
                } else {
                    Duration::ZERO
                };
                log::info!(
                    "{:?} failed ({reason}); trying again from {from:?}",
                    current.track.title
                );
                self.start_track(&mut state, current.track, 1, from);
                return;
            }
            End::Failed(reason) => {
                log::warn!("{:?} failed again: {reason}", current.track.title);
                let lang = self.config.lang;
                self.say(
                    current.track.text_channel,
                    fill(
                        strings(lang).track_failed,
                        &[
                            ("title", &text::clean_title(lang, &current.track.title)),
                            ("reason", &text::media_error(lang, &reason)),
                        ],
                    ),
                );
            }
        }
        self.start_next(&mut state);
        self.panel_refresh();
    }

    /// Every few seconds: leave after a while alone or with nothing to
    /// play, and notice a lost voice connection.
    async fn supervise(self: Arc<Self>) {
        let mut state = self.state.lock().await;
        let Some(voice) = &state.voice else { return };
        if !voice.is_connected() {
            log::warn!("guild {}: voice connection lost", self.guild_id);
            if let Some(track) = self.stop_current(&mut state) {
                state.queue.push_front(track);
            }
            state.voice = None;
            self.say_in(
                &state,
                self.with_prefix(strings(self.config.lang).voice_lost),
            );
            self.panel_refresh();
            return;
        }
        let cache = self.bot.cache();
        let listeners = voice
            .participants()
            .into_iter()
            .filter(|id| !cache.user(*id).is_some_and(|user| user.bot))
            .count();
        let now = Instant::now();
        if listeners == 0 {
            let since = *state.alone_since.get_or_insert(now);
            if now.duration_since(since) >= self.config.alone_timeout {
                log::info!("guild {}: alone, leaving", self.guild_id);
                self.stop_current(&mut state);
                state.queue.clear();
                state.prefetch = None;
                self.disconnect(&mut state).await;
                self.say_in(&state, strings(self.config.lang).alone_leaving);
                self.panel_refresh();
                return;
            }
        } else {
            state.alone_since = None;
        }
        if state.current.is_none() && state.queue.is_empty() {
            let since = *state.idle_since.get_or_insert(now);
            if now.duration_since(since) >= self.config.idle_timeout {
                log::info!("guild {}: idle, leaving", self.guild_id);
                self.disconnect(&mut state).await;
                self.say_in(
                    &state,
                    self.with_prefix(strings(self.config.lang).idle_leaving),
                );
            }
        } else {
            state.idle_since = None;
        }
    }
}
