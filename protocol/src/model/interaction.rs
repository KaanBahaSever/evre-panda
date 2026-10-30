//! Interactions: a person clicked a button under a bot's message
//! (architecture.md §8.1 components and interactions, ADR-059).

use serde::{Deserialize, Serialize};

use crate::model::{Message, User};
use crate::snowflake::Snowflake;

/// What kind of interaction it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InteractionKind {
    /// A [`Button`](crate::model::Button) was clicked.
    #[default]
    Button,
    /// A kind introduced after this build (select menus, modals, ...).
    #[serde(other)]
    Unknown,
}

/// Payload of `interactionCreate`: sent only to the bot that owns the
/// message, which must answer within
/// [`INTERACTION_DEADLINE_MS`](crate::rest::limits::INTERACTION_DEADLINE_MS)
/// with `POST /v1/interactions/{id}/callback`
/// ([`InteractionCallback`](crate::rest::InteractionCallback)).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interaction {
    /// The interaction's ID (for the callback and followups).
    pub id: Snowflake,
    /// What happened.
    #[serde(default)]
    pub kind: InteractionKind,
    /// The guild of the channel.
    pub guild_id: Snowflake,
    /// The channel of the message.
    pub channel_id: Snowflake,
    /// The message whose component was used, as it is now.
    pub message: Message,
    /// The `customId` of the component.
    pub custom_id: String,
    /// Who clicked.
    pub user: User,
}
