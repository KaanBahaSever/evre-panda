use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::model::PermissionOverwrite;
use crate::snowflake::Snowflake;

/// What kind of channel this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChannelKind {
    /// Groups other channels; has no content of its own.
    Category,
    /// Text chat.
    Text,
    /// Voice room (one LiveKit room per channel).
    Voice,
    /// Text channel for announcements.
    Announcement,
    /// A kind introduced after this build. Decoded only, never sent; clients
    /// should hide such channels.
    #[serde(other)]
    Unknown,
}

/// Default codec profile for voice tracks in a channel (architecture.md §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioProfile {
    /// Mono speech, Opus `voip`, 64 kbit/s by default.
    Voice,
    /// Mono speech at higher bitrate.
    HdVoice,
    /// Stereo, Opus `audio`, no DTX.
    Studio,
}

impl AudioProfile {
    /// The bitrate range (kbit/s) this profile is designed for (§6.3). The
    /// effective maximum is further capped by the guild's plan.
    #[must_use]
    pub const fn bitrate_range_kbps(self) -> RangeInclusive<u32> {
        match self {
            Self::Voice => 32..=128,
            Self::HdVoice => 96..=128,
            Self::Studio => 128..=256,
        }
    }
}

/// TeamSpeak-style voice channel options (§10).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettings {
    /// Maximum users in the room; `None` means no per-channel cap (the plan's
    /// concurrent voice limit still applies).
    pub max_users: Option<u32>,
    /// Default codec profile for voice tracks.
    pub audio_profile: AudioProfile,
    /// Maximum voice bitrate in kbit/s, capped by the guild's plan.
    pub max_bitrate_kbps: u32,
    /// Everyone must use push-to-talk. Voice activation is allowed only when
    /// this is `false` and the member has `USE_VOICE_ACTIVITY`.
    #[serde(default)]
    pub force_push_to_talk: bool,
}

/// A channel in a guild.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    /// Channel ID.
    pub id: Snowflake,
    /// Owning guild.
    pub guild_id: Snowflake,
    /// Channel kind.
    pub kind: ChannelKind,
    /// Display name.
    pub name: String,
    /// The category this channel sits in; `None` for top-level channels and
    /// categories.
    pub parent_id: Option<Snowflake>,
    /// Sort order among siblings, ascending.
    pub position: i32,
    /// Channel topic shown in the header.
    pub topic: Option<String>,
    /// Voice options; set for voice channels only.
    pub voice: Option<VoiceSettings>,
    /// The channel's effective permission overwrites. Keeping them in sync
    /// with the parent category is a server concern.
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwrite>,
}
