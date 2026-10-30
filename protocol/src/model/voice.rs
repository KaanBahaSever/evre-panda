use serde::{Deserialize, Serialize};

use crate::model::Platform;
use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// A login session's device class (ADR-076): an account holds one voice
/// connection per class. From the platform its gateway `identify` named
/// (`ClientInfo.os`); sessions that never said, and bots, are `desktop`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum VoiceDevice {
    /// Windows, macOS, Linux (and anything that does not say).
    #[default]
    Desktop,
    /// Android, iOS.
    Mobile,
}

impl VoiceDevice {
    /// The class of a platform.
    #[must_use]
    pub const fn of_platform(platform: Platform) -> Self {
        match platform {
            Platform::Android | Platform::Ios => Self::Mobile,
            Platform::Windows | Platform::MacOs | Platform::Linux | Platform::Unknown => {
                Self::Desktop
            }
        }
    }

    /// The wire name (`"desktop"`, `"mobile"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
        }
    }

    /// The class of a wire name; `None` for others.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "desktop" => Some(Self::Desktop),
            "mobile" => Some(Self::Mobile),
            _ => None,
        }
    }

    /// The other class.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Desktop => Self::Mobile,
            Self::Mobile => Self::Desktop,
        }
    }
}

/// Where a voice connection's camera video comes from (ADR-076).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CameraSource {
    /// The account's phone, as the desktop connection's camera companion
    /// (LiveKit identity `<userId>#cam`).
    Phone,
    /// The desktop's own camera, a `camera` track of the user's own
    /// identity.
    Desktop,
}

impl CameraSource {
    /// The wire name (`"phone"`, `"desktop"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Desktop => "desktop",
        }
    }

    /// The source of a wire name; `None` for others.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "phone" => Some(Self::Phone),
            "desktop" => Some(Self::Desktop),
            _ => None,
        }
    }
}

/// Suffix of a camera companion's LiveKit identity (`<userId>#cam`).
pub const CAMERA_IDENTITY_SUFFIX: &str = "#cam";

/// A user's voice connection state in a guild.
///
/// **Joining voice is always an explicit client action** (architecture.md
/// §6.6, ADR-019). The server reports `channel_id: Some(..)` only after the
/// user clicked or keyed a join, after an automatic rejoin of the *same*
/// room within the reconnect window following a dropped connection, or after
/// a moderator moved a user who was *already* in voice (`moved_by` is set).
/// Opening the app or a guild never puts anyone into a room, and there is no
/// default channel.
///
/// A user has at most one voice state per device class (`device`, ADR-076):
/// one desktop and one mobile connection, in any rooms but never both in
/// one room; `connection_id` tells a class's devices apart (ADR-050).
/// Clients that predate `device` key states by user and see the newest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceState {
    /// The guild.
    pub guild_id: Snowflake,
    /// The user.
    pub user_id: Snowflake,
    /// The voice channel the user is in; `None` means not in voice.
    pub channel_id: Option<Snowflake>,
    /// Muted their own microphone (possible before joining).
    #[serde(default)]
    pub self_mute: bool,
    /// Deafened themselves.
    #[serde(default)]
    pub self_deaf: bool,
    /// Muted by a moderator (`MUTE_MEMBERS`).
    #[serde(default)]
    pub server_mute: bool,
    /// Deafened by a moderator (`DEAFEN_MEMBERS`).
    #[serde(default)]
    pub server_deaf: bool,
    /// Sharing a screen or application.
    #[serde(default)]
    pub streaming: bool,
    /// When the current screen share went live (server clock), so everyone,
    /// including people who join later, sees how long it has run. `None`
    /// while not streaming, for streams that started before the server
    /// recorded it, and from older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streaming_since: Option<Timestamp>,
    /// Whose streams this user is watching (user IDs in the same channel),
    /// so a sharer sees how many and who watch (§7.4).
    #[serde(default)]
    pub watching: Vec<Snowflake>,
    /// Era is not in front for this user (another window, minimised): a
    /// watcher who is not really looking (owner, 2026-10-22).
    #[serde(default)]
    pub away: bool,
    /// Set when this update is a moderator move, so the moved user's client
    /// can show a notice (§6.6).
    pub moved_by: Option<Snowflake>,
    /// Which voice connection this is: the ID issued with its join token
    /// ([`VoiceJoinResponse::connection_id`](crate::rest::VoiceJoinResponse)).
    /// An account is in voice on one device at a time (§6.6, ADR-050): the
    /// device whose join returned this ID is the one in voice; the user's
    /// other devices show the user in the channel like any member but are
    /// not in voice, never follow moves meant for it and never update it.
    /// IDs grow with every join, so a higher one is the newer connection.
    /// Absent from older servers and for connections made with their tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<Snowflake>,
    /// The device class of this connection (ADR-076); `desktop` from older
    /// servers.
    #[serde(default)]
    pub device: VoiceDevice,
    /// A camera video goes with this connection (ADR-076): the desktop's own
    /// camera or its phone companion's. Viewers subscribe to it explicitly.
    #[serde(default)]
    pub camera: bool,
    /// Where the camera video comes from, while `camera`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_source: Option<CameraSource>,
}

impl VoiceState {
    /// The LiveKit identity of `user_id`'s phone camera companion.
    #[must_use]
    pub fn camera_identity(user_id: Snowflake) -> String {
        format!("{user_id}{CAMERA_IDENTITY_SUFFIX}")
    }
}
