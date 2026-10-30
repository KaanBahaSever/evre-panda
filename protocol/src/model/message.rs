use serde::{Deserialize, Serialize};

use crate::model::{ActionRow, User};
use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// A chat message. Its creation time is encoded in its ID
/// ([`Message::created_at`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Message ID.
    pub id: Snowflake,
    /// Channel the message was sent in.
    pub channel_id: Snowflake,
    /// Author's user ID.
    pub author_id: Snowflake,
    /// Markdown text.
    pub content: String,
    /// Client-generated ULID from the outbox (§8.1). The server dedupes sends
    /// on `(author_id, nonce)` and echoes the nonce back so the client can
    /// match the acknowledgement to its pending send.
    pub nonce: Option<String>,
    /// When the message was last edited; `None` if never.
    pub edited_at: Option<Timestamp>,
    /// Uploaded files.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// IDs of mentioned users: the `<@id>` mentions of the text, plus the
    /// replied message's author when the reply pings them
    /// ([`MessageReply::mention`]). People who blocked the author are left
    /// out (as of the send or the last edit): for them it is no mention.
    #[serde(default)]
    pub mentions: Vec<Snowflake>,
    /// The message this one answers (§8.1 replies). Left out for plain
    /// messages and by older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<MessageReply>,
    /// Rows of buttons under the message (§8.1 components). Only bots send
    /// them; a click is an interaction for the bot. Left out when empty;
    /// older clients ignore them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ActionRow>,
    /// A bot's answer to an interaction that only the person who clicked
    /// sees: never stored, delivered as `ephemeralMessage` (not
    /// `messageCreate`) to that person's sessions only, gone after a
    /// reload. Its `reply` names the message whose button was clicked.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ephemeral: bool,
    /// The author, sent only when they are no longer in the place the
    /// message is in (they left the guild, were kicked or banned, left the
    /// group DM, or the account was purged: then `deleted`), so clients that
    /// look authors up among the members can still render them (ADR-090).
    /// Left out for current members and participants, and by older
    /// servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Box<User>>,
}

impl Message {
    /// When the message was created (derived from its ID).
    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.id.timestamp()
    }
}

/// What a reply shows of the message it answers: enough for the quote line
/// above the reply without loading the original (§8.1 replies). The server
/// fills it from the original's current state whenever it returns the
/// reply; clients update it when the original changes
/// (`messageUpdate` / `messageDelete` of `message_id`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageReply {
    /// The message answered, always in the same channel.
    pub message_id: Snowflake,
    /// Its author; `None` once it was deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_id: Option<Snowflake>,
    /// The start of its text on one line: whitespace runs collapsed to one
    /// space, at most [`REPLY_SNIPPET_LEN`](crate::rest::limits::REPLY_SNIPPET_LEN)
    /// characters, ending in `…` when cut, never inside a `<@id>`. Empty
    /// when it has only attachments or was deleted.
    #[serde(default)]
    pub snippet: String,
    /// How many files it has (clients show "attachment" for an empty
    /// snippet).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub attachment_count: u32,
    /// The original was deleted; only `message_id` is left.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The reply pings the original's author: they are in
    /// [`Message::mentions`] and it counts as a mention for them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mention: bool,
    /// The original's author, under the same rule as [`Message::author`]:
    /// only when they are no longer in the place the message is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Box<User>>,
}

impl MessageReply {
    /// A reply to a message that no longer exists.
    #[must_use]
    pub const fn deleted(message_id: Snowflake, mention: bool) -> Self {
        Self {
            message_id,
            author_id: None,
            snippet: String::new(),
            attachment_count: 0,
            deleted: true,
            mention,
            author: None,
        }
    }
}

// serde's `skip_serializing_if` passes a reference.
const fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// Metadata of an uploaded file (§8.3). The download URL is signed by the
/// server whenever it returns the attachment and is not stored.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Attachment ID.
    pub id: Snowflake,
    /// Original file name.
    pub filename: String,
    /// MIME type, if known.
    pub content_type: Option<String>,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Pixel width for images and videos.
    pub width: Option<u32>,
    /// Pixel height for images and videos.
    pub height: Option<u32>,
    /// Signed download URL (absolute, expires after a few days; the server
    /// signs a fresh one each time it returns the message).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}
