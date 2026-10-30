use serde::{Deserialize, Serialize};

use crate::model::{ActionRow, Message};
use crate::snowflake::Snowflake;

/// `POST /v1/channels/{channel}/messages/{message}/interactions`: a person
/// clicked a button under a bot's message (§8.1 interactions).
///
/// Needs `VIEW_CHANNEL`; bots cannot click (`403`). The button must exist
/// and be enabled (`400` on `customId` otherwise; link buttons open their
/// URL instead). The server forwards it to the message's bot as
/// `interactionCreate` and answers once the bot did (at most
/// [`INTERACTION_DEADLINE_MS`](super::limits::INTERACTION_DEADLINE_MS)):
/// [`InteractionResponse`], `50300` when the bot is not connected, `50400`
/// when it did not answer in time, `429` for too many clicks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInteractionRequest {
    /// The clicked button's `customId`.
    pub custom_id: String,
}

/// How the bot answered an interaction, for the person who clicked (the
/// click's response) and for the bot (its callback's response).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionResponse {
    /// The interaction.
    pub id: Snowflake,
    /// What the bot did.
    pub kind: InteractionResponseKind,
    /// The updated message (`updated`; everyone in the channel also gets
    /// `messageUpdate`) or the ephemeral reply (`ephemeral`; the clicker's
    /// sessions also get `ephemeralMessage`). Absent for `deferred`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// What a bot did with an interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InteractionResponseKind {
    /// It edited the message the button is on.
    Updated,
    /// It answered the clicker alone.
    Ephemeral,
    /// It acknowledged and may answer later (an edit of the message, or
    /// followups).
    Deferred,
    /// An answer introduced after this build; treat it as `deferred`.
    #[serde(other)]
    Unknown,
}

/// `POST /v1/interactions/{interaction}/callback`: the bot's one answer to
/// an `interactionCreate`, within
/// [`INTERACTION_DEADLINE_MS`](super::limits::INTERACTION_DEADLINE_MS) (bots
/// only; another bot, an unknown or expired interaction: `404`; a second
/// answer: `400`). Answers with the [`InteractionResponse`] the clicker
/// gets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InteractionCallback {
    /// Edit the message the button is on, like
    /// `PATCH …/messages/{message}`: absent fields stay.
    UpdateMessage {
        /// New Markdown text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        /// New components; `[]` removes them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        components: Option<Vec<ActionRow>>,
    },
    /// Answer only the person who clicked ("Sadece sen görüyorsun"): a
    /// message that is never stored and has no components.
    Ephemeral {
        /// Markdown text (the usual message limits).
        content: String,
    },
    /// Acknowledge now (the client stops waiting) and answer later: edit
    /// the message, or send followups.
    Defer,
}

/// `POST /v1/interactions/{interaction}/followup`: one more ephemeral
/// message to the person who clicked, after the callback and for up to
/// [`INTERACTION_FOLLOWUP_SECS`](super::limits::INTERACTION_FOLLOWUP_SECS)
/// (bots only, the interaction's own bot) → the [`Message`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionFollowup {
    /// Markdown text (the usual message limits).
    pub content: String,
}
