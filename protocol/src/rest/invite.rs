use serde::{Deserialize, Serialize};

use crate::model::{GuildAccess, GuildCategory};
use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// `POST /v1/guilds/{guild}/invites` (needs `CREATE_INVITE`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteRequest {
    /// Maximum uses; absent or 0 means unlimited.
    #[serde(default)]
    pub max_uses: Option<u32>,
    /// Lifetime in seconds; absent or 0 means it never expires.
    #[serde(default)]
    pub max_age_secs: Option<u64>,
    /// Channel to open after joining (a text channel the invite's guild owns).
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
}

/// `GET /v1/invites/{code}`: what an invite leads to, shown before joining.
/// Needs no authentication.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    /// The invite code.
    pub code: String,
    /// The guild.
    pub guild_id: Snowflake,
    /// The guild's name.
    pub guild_name: String,
    /// Number of members.
    pub member_count: u64,
    /// When the invite stops working; `None` means never.
    pub expires_at: Option<Timestamp>,
    /// The guild's access mode (ADR-095): `closed` means the invite does
    /// not work now ("Bu sunucu şu an yeni üye almıyor."). Older servers
    /// never send it.
    #[serde(default)]
    pub access: GuildAccess,
    /// The guild's Keşfet description, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The guild's Keşfet category, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<GuildCategory>,
}
