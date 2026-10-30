use serde::{Deserialize, Serialize};

use crate::gateway::PresenceUpdate;
use crate::locale::Locale;
use crate::model::{Channel, Guild, GuildAccess, GuildCategory, Member, Role, User, VoiceState};
use crate::permissions::Permissions;
use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// `POST /v1/guilds`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGuildRequest {
    /// Guild name.
    pub name: String,
}

/// `PATCH /v1/guilds/{guild}` (needs `MANAGE_GUILD`). Absent fields stay;
/// `null` clears the Keşfet fields.
///
/// A listed guild (`public`, `request`) must keep a description and a
/// category (`400` on `description` or `category`); its `locale` defaults
/// to the owner's UI language. Switching away from `request` approves
/// the pending join requests when the new mode is `public` and closes them
/// (`joinRequestUpdate` `closed` to the requesters) otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGuildRequest {
    /// New name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New access mode (ADR-095).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<GuildAccess>,
    /// New Keşfet description (trimmed, at most
    /// [`MAX_GUILD_DESCRIPTION_LEN`](super::MAX_GUILD_DESCRIPTION_LEN)
    /// characters); `null` or empty removes it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::double_option"
    )]
    pub description: Option<Option<String>>,
    /// New Keşfet category; `null` removes it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::double_option"
    )]
    pub category: Option<Option<GuildCategory>>,
    /// New Keşfet language; `null` removes it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::serde_util::double_option"
    )]
    pub locale: Option<Option<Locale>>,
}

/// A member together with its user, so clients can render member lists
/// without further requests.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMember {
    /// The membership.
    pub member: Member,
    /// The user.
    pub user: User,
}

/// The caller's effective permissions in one channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPermissions {
    /// The channel.
    pub channel_id: Snowflake,
    /// Effective permissions after overwrites and implicit rules.
    pub permissions: Permissions,
}

/// Everything a client needs to render one guild, as seen by the caller.
///
/// Returned by `GET /v1/guilds/{guild}` and sent as the `guildCreate` event
/// after `ready`, after joining a guild and whenever the caller's view
/// changed in a way plain events cannot express (role or overwrite edits).
/// A `guildCreate` replaces all cached state of that guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildSnapshot {
    /// The guild.
    pub guild: Guild,
    /// Channels the caller can see (`VIEW_CHANNEL`), sorted by position.
    pub channels: Vec<Channel>,
    /// All roles, `@everyone` included.
    pub roles: Vec<Role>,
    /// All members with their users.
    pub members: Vec<GuildMember>,
    /// Who is in voice, limited to visible channels. Nobody is ever in voice
    /// because they opened the app (§6.6).
    pub voice_states: Vec<VoiceState>,
    /// The caller's guild-wide permissions.
    pub permissions: Permissions,
    /// The caller's permissions in each visible channel.
    pub channel_permissions: Vec<ChannelPermissions>,
    /// Members who are not offline. Everyone else is offline as of
    /// `presence_version` (§8.4).
    #[serde(default)]
    pub presences: Vec<PresenceUpdate>,
    /// The server's presence version when this snapshot was taken. A client
    /// keeps a newer status it already has for a user.
    #[serde(default)]
    pub presence_version: u64,
    /// The caller's read state in every visible text and announcement
    /// channel (§8.1), replacing what the client had for them. Empty for
    /// bots and from servers without read states.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_states: Vec<super::ReadState>,
    /// Open message reports in the channels where the caller has
    /// `MANAGE_MESSAGES` (ADR-090); 0 for everyone else and from older
    /// servers. Fresh in `GET /v1/guilds/{guild}` (read it when the server
    /// settings open); no event changes it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub open_reports: u32,
    /// Pending join requests (ADR-095) for callers with `KICK_MEMBERS`;
    /// 0 for everyone else and from older servers. `joinRequestCreate`
    /// keeps it current.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pending_join_requests: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// `POST /v1/guilds/{guild}/roles` (needs `MANAGE_ROLES`). A new role is
/// placed just above `@everyone`. Only permissions the caller has can be
/// granted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRoleRequest {
    /// Role name.
    pub name: String,
    /// Guild-wide permissions; default none.
    #[serde(default)]
    pub permissions: Option<Permissions>,
    /// Color as `0xRRGGBB`; default 0 (none).
    #[serde(default)]
    pub color: Option<u32>,
    /// Show separately in the member list; default `false`.
    #[serde(default)]
    pub hoist: Option<bool>,
    /// Anyone may mention it; default `false`.
    #[serde(default)]
    pub mentionable: Option<bool>,
}

/// `PATCH /v1/guilds/{guild}/roles/{role}` (needs `MANAGE_ROLES` and a higher
/// role than the edited one). Absent fields stay. `@everyone` can only change
/// its permissions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleRequest {
    /// New name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New guild-wide permissions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// New color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    /// New hoist flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hoist: Option<bool>,
    /// New mentionable flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mentionable: Option<bool>,
    /// New position (≥ 1, below the caller's highest role).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
}

/// `PUT /v1/guilds/{guild}/bans/{user}` (needs `BAN_MEMBERS` and a higher
/// rank than the user; nobody bans the owner or themselves, and bots are
/// deleted instead). The body is optional. Banning removes the member
/// (voice included) and keeps them out: no invite lets a banned user back in
/// until `DELETE /v1/guilds/{guild}/bans/{user}`. Someone who is not a
/// member (any more) can be banned too. Banning again only changes the
/// reason.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBanRequest {
    /// Why, for the ban list (at most
    /// [`MAX_BAN_REASON_LEN`](super::limits::MAX_BAN_REASON_LEN)
    /// characters). Only those with `BAN_MEMBERS` see it; the banned user
    /// never does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A banned user, as `GET /v1/guilds/{guild}/bans` lists them (needs
/// `BAN_MEMBERS`; newest first).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildBan {
    /// The banned user.
    pub user: User,
    /// The reason given, if any.
    #[serde(default)]
    pub reason: Option<String>,
    /// Who banned them (`None` once that account is gone).
    #[serde(default)]
    pub banned_by: Option<Snowflake>,
    /// When.
    pub created_at: Timestamp,
}
