use serde::{Deserialize, Serialize};

use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// A user's membership in a guild.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// The member's user ID.
    pub user_id: Snowflake,
    /// The guild.
    pub guild_id: Snowflake,
    /// Guild-specific nickname.
    pub nick: Option<String>,
    /// Assigned roles. Never contains `@everyone`, which every member has
    /// implicitly.
    #[serde(default)]
    pub role_ids: Vec<Snowflake>,
    /// When the user joined the guild.
    pub joined_at: Timestamp,
}
