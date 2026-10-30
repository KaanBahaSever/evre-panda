use serde::{Deserialize, Serialize};

use crate::model::ActionRow;
use crate::snowflake::Snowflake;

/// `POST /v1/channels/{channel}/messages` (needs `SEND_MESSAGES`).
///
/// The message is persisted before it is acknowledged or dispatched (§8.1).
/// Sends are idempotent per `(author, nonce)`: retrying with the same nonce
/// returns the already stored message instead of creating a second one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageRequest {
    /// Markdown text.
    pub content: String,
    /// Client-generated ULID from the outbox.
    #[serde(default)]
    pub nonce: Option<String>,
    /// Uploads to attach (`POST /v1/channels/{channel}/attachments`, at most
    /// [`MAX_MESSAGE_ATTACHMENTS`]); with attachments the text may be empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachment_ids: Vec<crate::Snowflake>,
    /// The message this one answers (§8.1 replies): a message of the same
    /// channel (`400` on `replyTo` otherwise), which needs
    /// `READ_MESSAGE_HISTORY`. Older servers ignore it and store a plain
    /// message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Snowflake>,
    /// With `reply_to`: ping the original's author (the composer's "@"
    /// toggle). They are added to the reply's mentions, so it counts as a
    /// mention for them, unless it is the sender or they are no longer in
    /// the channel. Ignored without `reply_to`; default `false`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mention_replied: bool,
    /// Rows of buttons under the message (§8.1 components): bots only (a
    /// person sending any is a `403`), at most
    /// [`MAX_COMPONENT_ROWS`](super::limits::MAX_COMPONENT_ROWS) rows; with
    /// components the text may be empty. Older servers ignore them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ActionRow>,
}

/// Attachments per message at most.
pub const MAX_MESSAGE_ATTACHMENTS: usize = 10;

/// `PATCH /v1/channels/{channel}/messages/{message}` (author only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMessageRequest {
    /// New Markdown text.
    pub content: String,
    /// New components (bots only): absent keeps them, `[]` removes them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<ActionRow>>,
}

impl UpdateMessageRequest {
    /// New text, components unchanged.
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            components: None,
        }
    }
}

/// Query of `GET /v1/channels/{channel}/messages` (needs `VIEW_CHANNEL` and
/// `READ_MESSAGE_HISTORY`).
///
/// The response is always sorted by ID ascending (oldest first):
///
/// - `after` only: the first `limit` messages after `after` (catch-up sync
///   after a long disconnect, §8.1; page on with the last ID);
/// - `before` only: the `limit` messages right before `before` (scrolling up;
///   page on with the first ID);
/// - both: messages strictly between them, oldest first, up to `limit`;
/// - neither: the newest `limit` messages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageQuery {
    /// Only messages with a smaller ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Snowflake>,
    /// Only messages with a larger ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Snowflake>,
    /// Page size, 1 to [`MAX_MESSAGE_PAGE`](super::limits::MAX_MESSAGE_PAGE);
    /// default [`DEFAULT_MESSAGE_PAGE`](super::limits::DEFAULT_MESSAGE_PAGE).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}
