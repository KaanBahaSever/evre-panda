use serde::{Deserialize, Serialize};

use crate::permissions::Permissions;
use crate::snowflake::Snowflake;

/// Who a [`PermissionOverwrite`] applies to. Serialized as
/// `{"type": "role" | "member", "id": "<snowflake>"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "camelCase")]
pub enum OverwriteTarget {
    /// A role; the guild ID targets `@everyone`.
    Role(Snowflake),
    /// A single member, by user ID.
    Member(Snowflake),
}

/// Per-channel allow/deny adjustments for a role or member (§10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOverwrite {
    /// Role or member this applies to.
    pub target: OverwriteTarget,
    /// Permissions granted in the channel.
    pub allow: Permissions,
    /// Permissions revoked in the channel.
    pub deny: Permissions,
}
