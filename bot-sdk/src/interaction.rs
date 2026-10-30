//! A click on one of the bot's buttons, and how to answer it
//! (architecture.md §8.1 interactions, ADR-059).

use std::ops::Deref;

use protocol::model::ActionRow;
use protocol::rest::{InteractionCallback, InteractionResponse};
use protocol::Message;

use crate::error::{Error, Result};
use crate::rest::Rest;

/// Someone clicked a button under one of the bot's messages: a
/// [`BotEvent::Interaction`](crate::BotEvent::Interaction). Derefs to the
/// wire [`protocol::model::Interaction`] (`custom_id`, `user`, `message`,
/// `channel_id`, `guild_id`).
///
/// Answer exactly once, within
/// [`INTERACTION_DEADLINE_MS`](protocol::rest::limits::INTERACTION_DEADLINE_MS)
/// (3 s), or the person sees "the bot did not answer":
/// - [`Interaction::update_message`] / [`Interaction::update_components`]:
///   edit the message the button is on (everyone sees it);
/// - [`Interaction::reply_ephemeral`]: a message only the clicker sees;
/// - [`Interaction::defer`]: acknowledge now, act later (edit the message
///   with [`Rest::edit_message_with`], or [`Interaction::followup`]).
///
/// After the answer, [`Interaction::followup`] sends more ephemeral
/// messages for up to
/// [`INTERACTION_FOLLOWUP_SECS`](protocol::rest::limits::INTERACTION_FOLLOWUP_SECS).
///
/// ```no_run
/// use era_bot_sdk::{Bot, BotEvent};
///
/// # async fn run(bot: Bot, mut events: era_bot_sdk::Events) -> era_bot_sdk::Result<()> {
/// while let Some(event) = events.recv().await {
///     if let BotEvent::Interaction(click) = event {
///         match click.custom_id.as_str() {
///             "count" => {
///                 let shown = click.message.content.clone() + "+";
///                 click.update_message(shown, click.message.components.clone()).await?;
///             }
///             _ => drop(click.reply_ephemeral("Bunu tanımıyorum 🤔").await?),
///         }
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Interaction {
    data: protocol::model::Interaction,
    rest: Rest,
}

impl Deref for Interaction {
    type Target = protocol::model::Interaction;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Interaction {
    pub(crate) const fn new(data: protocol::model::Interaction, rest: Rest) -> Self {
        Self { data, rest }
    }

    /// The wire interaction.
    #[must_use]
    pub const fn data(&self) -> &protocol::model::Interaction {
        &self.data
    }

    /// Takes the wire interaction out.
    #[must_use]
    pub fn into_data(self) -> protocol::model::Interaction {
        self.data
    }

    async fn answer(&self, callback: &InteractionCallback) -> Result<InteractionResponse> {
        self.rest.interaction_callback(self.data.id, callback).await
    }

    async fn answer_with_message(&self, callback: &InteractionCallback) -> Result<Message> {
        self.answer(callback)
            .await?
            .message
            .ok_or_else(|| Error::Decode("the answer carries no message".into()))
    }

    /// Replaces the text and the buttons of the message the button is on.
    pub async fn update_message(
        &self,
        content: impl Into<String>,
        components: Vec<ActionRow>,
    ) -> Result<Message> {
        self.answer_with_message(&InteractionCallback::UpdateMessage {
            content: Some(content.into()),
            components: Some(components),
        })
        .await
    }

    /// Replaces only the buttons (`[]` removes them); the text and its
    /// "edited" mark stay.
    pub async fn update_components(&self, components: Vec<ActionRow>) -> Result<Message> {
        self.answer_with_message(&InteractionCallback::UpdateMessage {
            content: None,
            components: Some(components),
        })
        .await
    }

    /// Answers with a message only the person who clicked sees ("Sadece
    /// sen görüyorsun"); it is not stored and has no buttons.
    pub async fn reply_ephemeral(&self, content: impl Into<String>) -> Result<Message> {
        self.answer_with_message(&InteractionCallback::Ephemeral {
            content: content.into(),
        })
        .await
    }

    /// Acknowledges the click now; the bot acts later.
    pub async fn defer(&self) -> Result<()> {
        self.answer(&InteractionCallback::Defer).await.map(drop)
    }

    /// One more ephemeral message to the person who clicked, after the
    /// answer (at most 5 per interaction).
    pub async fn followup(&self, content: impl Into<String>) -> Result<Message> {
        self.rest
            .interaction_followup(self.data.id, content.into())
            .await
    }
}
