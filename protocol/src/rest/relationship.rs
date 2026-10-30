use serde::{Deserialize, Serialize};

/// `POST /v1/users/@me/relationships`: sends a friend request by username
/// (accepts theirs if they already sent one).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequest {
    /// The other user's username (case-insensitive).
    pub username: String,
}
