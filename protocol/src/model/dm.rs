use serde::{Deserialize, Serialize};

use crate::model::User;
use crate::snowflake::Snowflake;

/// A direct message channel (§8.2): a 1:1 DM between two users or a group
/// DM of up to [`MAX_DM_RECIPIENTS`](crate::rest::limits::MAX_DM_RECIPIENTS)
/// people. Messages, typing and attachments use the usual
/// `/v1/channels/{id}/...` routes with this channel's ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmChannel {
    /// Channel ID.
    pub id: Snowflake,
    /// Every participant, the caller included, in the order they were
    /// added (by user ID among those added together).
    pub recipients: Vec<User>,
    /// Who manages a group DM (its creator, or the longest-standing member
    /// after the creator left); `None` for a 1:1 DM.
    pub owner_id: Option<Snowflake>,
    /// Name of a group DM; `None` shows the recipients' names.
    pub name: Option<String>,
    /// The newest message, `None` while there is none. Lists of DMs are
    /// sorted by it (or by the channel ID when empty), newest first.
    pub last_message_id: Option<Snowflake>,
}

impl DmChannel {
    /// Whether this is a group DM (it has an owner).
    #[must_use]
    pub const fn is_group(&self) -> bool {
        self.owner_id.is_some()
    }
}
