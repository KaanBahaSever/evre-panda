use serde::{Deserialize, Serialize};

use crate::gateway::Token;
use crate::model::AudioProfile;
use crate::timestamp::Timestamp;

/// `POST /v1/channels/{channel}/voice/join` (needs `CONNECT`).
///
/// This is the **only** way into a voice room, and clients call it only on an
/// explicit user action or to rejoin the same room after a dropped connection
/// (§6.6, ADR-019). The server never issues tokens on its own. The voice
/// state appears once the client actually connects to the room. An account
/// holds one voice connection per device class (ADR-076): once connected,
/// this connection replaces the account's connection on any other device of
/// the same class (ADR-050). While the other class's connection is in this
/// room the join answers `409`/`40921` unless `replaceOtherDevice`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceJoinRequest {
    /// Join with the microphone muted (toggled before joining).
    #[serde(default)]
    pub self_mute: bool,
    /// Join deafened.
    #[serde(default)]
    pub self_deaf: bool,
    /// The account's connection of the other device class is in this room
    /// (`409` with code `40921`, ADR-076): take voice over from it. That
    /// connection ends like an ADR-050 replacement once this one connects.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replace_other_device: bool,
}

/// A track source a participant may publish. Wire names are camelCase;
/// [`VoiceSource::livekit_name`] gives LiveKit's grant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSource {
    /// Microphone audio (`SPEAK`).
    Microphone,
    /// Screen or window video (`STREAM`).
    ScreenShare,
    /// Audio of a screen share (`STREAM`).
    ScreenShareAudio,
}

impl VoiceSource {
    /// The source name in LiveKit's `canPublishSources` grant.
    #[must_use]
    pub const fn livekit_name(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::ScreenShare => "screen_share",
            Self::ScreenShareAudio => "screen_share_audio",
        }
    }
}

/// What the issued voice token allows, derived from the caller's channel
/// permissions and the guild's plan. The token enforces it on the SFU; the
/// client uses it to grey out controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceGrants {
    /// May receive others' tracks (`false` while server-deafened).
    pub can_subscribe: bool,
    /// Sources the caller may publish, sorted. Empty means listen-only.
    pub can_publish_sources: Vec<VoiceSource>,
    /// Highest voice bitrate (kbit/s): the channel setting capped by the plan.
    pub max_bitrate_kbps: u32,
    /// The channel's default codec profile for voice tracks.
    pub audio_profile: AudioProfile,
    /// Voice activation is not allowed; the client must use push-to-talk.
    pub force_push_to_talk: bool,
    /// Bots only (§4.4): the highest bitrate (kbit/s) for a music track, the
    /// plan's `max_music_bitrate_kbps`. Not capped by the channel's voice
    /// bitrate, which is meant for speech. `None` (absent on the wire) for
    /// people. The bot SDK publishes at most this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_music_bitrate_kbps: Option<u32>,
    /// May publish a camera (`STREAM`, desktop sessions, ADR-076): the
    /// token's LiveKit grant then lists the `camera` source too. A field of
    /// its own rather than a [`VoiceSource`], so older clients still read
    /// the grants. Publish at most `max_camera_resolution`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub can_publish_camera: bool,
}

/// An ICE server for the WebRTC connection. Only Era's own STUN/TURN servers
/// are ever listed; clients must not add third-party defaults (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IceServer {
    /// `stun:`, `turn:` or `turns:` URLs.
    pub urls: Vec<String>,
    /// TURN username, if the server needs one.
    #[serde(default)]
    pub username: Option<String>,
    /// TURN credential, if the server needs one.
    #[serde(default)]
    pub credential: Option<String>,
}

/// Response of [`VoiceJoinRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceJoinResponse {
    /// LiveKit URL to connect to (`wss://…`).
    pub url: String,
    /// Short-lived LiveKit access token for this room only. LiveKit refreshes
    /// it while connected, so it only has to be valid at connect time.
    pub token: Token,
    /// The LiveKit room, `v_{guild_id}_{channel_id}`.
    pub room: String,
    /// The participant identity (the user ID).
    pub identity: String,
    /// When the token stops being accepted for new connections.
    pub expires_at: Timestamp,
    /// The ICE servers to use instead of any SDK default.
    pub ice_servers: Vec<IceServer>,
    /// What the token allows.
    pub grants: VoiceGrants,
    /// The ID of the connection this token makes, new with every join and
    /// growing (a Snowflake). The voice state of the connection carries it
    /// ([`VoiceState::connection_id`](crate::VoiceState)), so this device
    /// knows that state is its own. Connecting replaces the account's voice
    /// connection on any other device (§6.6, ADR-050). Absent from older
    /// servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<crate::Snowflake>,
}

/// `PATCH /v1/guilds/{guild}/voice-states/@me`: the caller's own mute and
/// deafen flags and watched streams while in voice. Absent fields stay.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVoiceStateRequest {
    /// Microphone muted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_mute: Option<bool>,
    /// Deafened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_deaf: Option<bool>,
    /// The streams being watched (replaces the list). Users who are not
    /// streaming in the caller's channel are dropped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watching: Option<Vec<crate::Snowflake>>,
    /// Era is not in front (see [`VoiceState::away`](crate::VoiceState)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub away: Option<bool>,
    /// Change the state only while it is this connection's (the
    /// `connectionId` of the caller's join), else `404`: a device whose
    /// connection another device replaced cannot change the new one
    /// (ADR-050). Absent (older clients): whatever connection is current.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<crate::Snowflake>,
}

/// Response of `POST /v1/channels/{channel}/voice/camera` (ADR-076): a
/// mobile session's token to join the room of its account's desktop voice
/// connection as that connection's camera companion. Video only: the token
/// may publish the `camera` source and nothing else, and may not subscribe.
/// `409`/`40922` while the account's desktop connection is not in that
/// channel, `409`/`40923` while the desktop's own camera runs there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraJoinResponse {
    /// LiveKit URL to connect to (`wss://…`).
    pub url: String,
    /// Short-lived LiveKit access token for this room only.
    pub token: Token,
    /// The LiveKit room, `v_{guild_id}_{channel_id}`.
    pub room: String,
    /// The companion's identity, `<userId>#cam`.
    pub identity: String,
    /// When the token stops being accepted for new connections.
    pub expires_at: Timestamp,
    /// The ICE servers to use instead of any SDK default.
    pub ice_servers: Vec<IceServer>,
    /// The companion's own connection ID (a Snowflake).
    pub connection_id: crate::Snowflake,
    /// The desktop connection it belongs to.
    pub paired_connection_id: crate::Snowflake,
    /// The plan's `max_camera_resolution`, e.g. `"720p30"`: publish at most
    /// this (simulcast 720p / 360p / 180p).
    pub max_resolution: String,
}

/// `POST /v1/guilds/{guild}/voice-states/{user}/move`: a moderator moves a
/// member who is already in voice to another voice channel (§6.6). Needs
/// `MOVE_MEMBERS` in the member's current channel and in `channel_id` (which
/// implies `CONNECT` there), a higher rank than the member (the owner may
/// move anyone), and a free seat. The member's own sessions get a
/// `voiceStateUpdate` with the new `channelId` and `movedBy`; their client
/// then switches rooms with a normal join, which the move authorises.
/// `DELETE /v1/guilds/{guild}/voice-states/{user}` disconnects instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveVoiceMemberRequest {
    /// The destination voice channel.
    pub channel_id: crate::Snowflake,
    /// Move this connection of the member (ADR-076). Absent: the member's
    /// desktop connection if the caller sees it, else their mobile one (two
    /// connections of one account cannot share a room).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<crate::Snowflake>,
}

/// `DELETE /v1/guilds/{guild}/voice-states/{user}?connectionId=…`: which
/// connection to disconnect (ADR-076); absent, every connection of the
/// member in the guild that the caller sees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectVoiceMemberQuery {
    /// The connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<crate::Snowflake>,
}

/// `PATCH /v1/guilds/{guild}/voice-states/{user}`: a moderator server-mutes
/// or server-deafens a member who is in voice (§6.6, ADR-068). Absent fields
/// stay. `serverMute` needs `MUTE_MEMBERS` and `serverDeaf` needs
/// `DEAFEN_MEMBERS`, both in the member's current channel, and a higher rank
/// than the member (the owner may act on anyone, nobody on the owner;
/// oneself skips the rank check). A member in a channel the caller cannot
/// see is not in voice to them (`400`).
///
/// The flags belong to the membership, not to the connection: they stay
/// through moves, leaving and rejoining voice and other devices, until a
/// moderator clears them or the member leaves the guild. The SFU applies
/// them at once: a server-muted member cannot publish their microphone, a
/// server-deafened one receives nothing. Everyone who sees the channel gets
/// the `voiceStateUpdate`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModerateVoiceStateRequest {
    /// Server mute: no microphone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_mute: Option<bool>,
    /// Server deafen: hears nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_deaf: Option<bool>,
}
