use serde::{Deserialize, Serialize};

use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// An invite into a guild. During the invite-only trial a valid guild invite
/// also allows registering a new account (architecture.md §11).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    /// The code shared in links. Random, case-sensitive.
    pub code: String,
    /// The guild the invite joins.
    pub guild_id: Snowflake,
    /// The channel the client opens after joining, if any. Never a voice
    /// channel to join: joining voice is always explicit (§6.6).
    pub channel_id: Option<Snowflake>,
    /// Who created the invite; `None` once that account is gone.
    pub inviter_id: Option<Snowflake>,
    /// Maximum number of uses; `None` means unlimited.
    pub max_uses: Option<u32>,
    /// How often the invite has been used.
    pub uses: u32,
    /// When the invite stops working; `None` means never.
    pub expires_at: Option<Timestamp>,
    /// When the invite was created.
    pub created_at: Timestamp,
}
