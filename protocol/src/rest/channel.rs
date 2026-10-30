use serde::{Deserialize, Serialize};

use crate::model::{ChannelKind, PermissionOverwrite, VoiceSettings};
use crate::serde_util::double_option;
use crate::snowflake::Snowflake;

/// `POST /v1/guilds/{guild}/channels` (needs `MANAGE_CHANNELS`; setting
/// overwrites also needs `MANAGE_ROLES`).
///
/// Without explicit overwrites, a channel created inside a category copies
/// the category's overwrites. Voice channels get default
/// [`VoiceSettings`] capped by the plan when `voice` is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChannelRequest {
    /// Channel kind.
    pub kind: ChannelKind,
    /// Display name.
    pub name: String,
    /// Parent category.
    #[serde(default)]
    pub parent_id: Option<Snowflake>,
    /// Sort order; default after the last sibling.
    #[serde(default)]
    pub position: Option<i32>,
    /// Topic (text and announcement channels).
    #[serde(default)]
    pub topic: Option<String>,
    /// Voice options (voice channels only).
    #[serde(default)]
    pub voice: Option<VoiceSettings>,
    /// Initial overwrites.
    #[serde(default)]
    pub permission_overwrites: Option<Vec<PermissionOverwrite>>,
}

/// `PATCH /v1/channels/{channel}` (needs `MANAGE_CHANNELS` in the channel;
/// changing overwrites also needs `MANAGE_ROLES`). Absent fields stay; an
/// explicit `null` clears `parentId` or `topic`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChannelRequest {
    /// New name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New parent category, or `null` for top level.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub parent_id: Option<Option<Snowflake>>,
    /// New sort order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    /// New topic, or `null` to clear it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub topic: Option<Option<String>>,
    /// New voice options (voice channels only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<VoiceSettings>,
    /// Replaces all overwrites.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_overwrites: Option<Vec<PermissionOverwrite>>,
}

/// One entry of `PATCH /v1/guilds/{guild}/channels` (needs
/// `MANAGE_CHANNELS` in the guild and in every channel named): the bulk
/// reorder behind dragging channels and categories in the channel list
/// (§21.2, ADR-088). The list is applied in one transaction or not at all;
/// every channel that changed is sent as a `channelUpdate`.
///
/// `lockPermissions` (ADR-089, Discord's `lock_permissions`): a channel
/// moved into another category takes that category's overwrites, which
/// also needs `MANAGE_ROLES` in the channel and passes the checks of
/// `PATCH /v1/channels/{channel}`'s overwrites. The client decides: it
/// sends it for a channel that was synced with its old category (same
/// overwrites); a channel with its own overwrites, or one moved to the top
/// level, keeps them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPositionUpdate {
    /// A channel of this guild.
    pub id: Snowflake,
    /// New sort order among its siblings.
    pub position: i32,
    /// New parent category, or `null` for top level; absent keeps it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub parent_id: Option<Option<Snowflake>>,
    /// Copy the new parent category's overwrites; only applies when the
    /// entry moves the channel into another category.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lock_permissions: bool,
}
