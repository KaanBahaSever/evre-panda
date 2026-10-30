use serde::{Deserialize, Serialize};

use crate::permissions::Permissions;
use crate::snowflake::Snowflake;

/// A guild role. Every guild has an `@everyone` role whose ID equals the
/// guild ID; it applies to all members implicitly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    /// Role ID.
    pub id: Snowflake,
    /// Owning guild.
    pub guild_id: Snowflake,
    /// Display name.
    pub name: String,
    /// Color as `0xRRGGBB`; 0 means no color.
    pub color: u32,
    /// Hierarchy position; higher is more powerful. `@everyone` is 0.
    pub position: i32,
    /// Guild-wide permissions granted by this role.
    pub permissions: Permissions,
    /// Show members with this role separately in the member list.
    pub hoist: bool,
    /// Anyone may `@mention` this role.
    pub mentionable: bool,
}

impl Role {
    /// Whether this is the guild's `@everyone` role.
    #[must_use]
    pub const fn is_everyone(&self) -> bool {
        self.id.get() == self.guild_id.get()
    }
}
