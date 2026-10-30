use serde::{Deserialize, Serialize};

use crate::snowflake::Snowflake;

/// How far the caller has read in one guild text channel or DM, and what is
/// still unread there (§8.1). Read states are the server's and follow the
/// user across devices.
///
/// Sent in [`GuildSnapshot::read_states`](super::GuildSnapshot::read_states)
/// (the guild's visible text channels), in `ready` (the open DMs), as the
/// answer of `POST /v1/channels/{channel}/messages/{message}/ack` and as the
/// `readStateUpdate` event to the user's other devices.
///
/// The counts cover the channel up to [`Self::last_message_id`]: a client
/// adds only `messageCreate`s with a newer ID (one with an older ID, e.g. a
/// replayed or caught-up message, is already counted). Both counts stop at
/// [`MAX_UNREAD_COUNT`](super::limits::MAX_UNREAD_COUNT), which clients
/// show as "99+".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadState {
    /// The channel (guild text or announcement channel, or DM).
    pub channel_id: Snowflake,
    /// The newest message marked read. `None` when the user never marked
    /// anything here: then everything since they joined the guild (or were
    /// added to the DM) is unread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_message_id: Option<Snowflake>,
    /// The channel's newest message when the counts were taken; `None` while
    /// it had none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message_id: Option<Snowflake>,
    /// Messages of others after the read position, at most
    /// [`MAX_UNREAD_COUNT`](super::limits::MAX_UNREAD_COUNT).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unread_count: u32,
    /// Messages of others after the read position that mention the user
    /// (`<@id>`), with the same cap.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mention_count: u32,
}

impl ReadState {
    /// Nothing read and nothing unread in `channel_id`.
    #[must_use]
    pub const fn empty(channel_id: Snowflake) -> Self {
        Self {
            channel_id,
            last_read_message_id: None,
            last_message_id: None,
            unread_count: 0,
            mention_count: 0,
        }
    }

    /// Whether anything is unread.
    #[must_use]
    pub const fn is_unread(&self) -> bool {
        self.unread_count > 0 || self.mention_count > 0
    }
}

// serde's `skip_serializing_if` passes a reference.
const fn is_zero(value: &u32) -> bool {
    *value == 0
}
