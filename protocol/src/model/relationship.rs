use serde::{Deserialize, Serialize};

use crate::gateway::PresenceUpdate;
use crate::model::User;
use crate::timestamp::Timestamp;

/// How another user relates to us (§8.5).
///
/// `Friend`, `Incoming` and `Outgoing` are mirrored: both users have an
/// entry. `Blocked` is one-sided: only the blocker has it, the blocked user
/// is never told (§8.2 blocking).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationshipKind {
    /// We are friends.
    Friend,
    /// They sent us a friend request.
    Incoming,
    /// We sent them a friend request.
    Outgoing,
    /// We blocked them: they cannot send us friend requests or DMs, their
    /// messages in shared guilds and group DMs are collapsed, and their
    /// mentions do not count for us.
    Blocked,
    /// A kind introduced after this build. Decoded only, never sent; clients
    /// drop such entries (and the earlier entry of that user).
    #[serde(other)]
    Unknown,
}

impl RelationshipKind {
    /// Whether both users have this entry (friends and pending requests),
    /// as opposed to a block, which only the blocker has.
    #[must_use]
    pub const fn is_mutual(self) -> bool {
        matches!(self, Self::Friend | Self::Incoming | Self::Outgoing)
    }
}

/// One entry of our friends list (§8.5), or a user we blocked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    /// The other user.
    pub user: User,
    /// Friend, pending request or blocked.
    pub kind: RelationshipKind,
    /// When the request was sent or accepted, or the user blocked.
    pub since: Timestamp,
    /// A friend's presence when this was sent, versioned like
    /// `presenceUpdate`; later changes arrive as `presenceUpdate`. `None`
    /// for pending requests and blocks (their presence is not shared).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<PresenceUpdate>,
}
