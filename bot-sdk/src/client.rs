//! The bot handle and its event stream.

use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use protocol::{Event, Guild, Message, Snowflake, StatusSetting, Token, User};
use tokio::sync::{mpsc, watch};

use protocol::model::ActionRow;
use protocol::rest::UpdateMessageRequest;

use crate::cache::Cache;
use crate::error::{Error, Result};
use crate::gateway::{Command, GatewayTask, Timing};
use crate::interaction::Interaction;
use crate::rest::{Rest, gateway_url};

/// Something that happened, in the order it happened.
///
/// The [`Cache`] already reflects an event when you receive it.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BotEvent {
    /// A new gateway session started (the first connection, or after the
    /// server could not resume the old one). The cache was reset; one
    /// [`Event::GuildCreate`] per guild follows.
    Ready {
        /// The bot's account.
        user: User,
        /// Its guilds (settings only; the snapshots follow).
        guilds: Vec<Guild>,
    },
    /// The connection came back and every missed event was replayed.
    Resumed,
    /// The connection dropped; the SDK reconnects on its own.
    Disconnected,
    /// A gateway dispatch: messages, voice states, members, channels, ...
    /// (never `interactionCreate`, which comes as [`BotEvent::Interaction`]).
    Dispatch(Event),
    /// Someone clicked a button under one of the bot's messages: answer it
    /// within 3 s ([`Interaction::update_message`],
    /// [`Interaction::reply_ephemeral`], [`Interaction::defer`]).
    Interaction(Interaction),
    /// The gateway stopped for good: [`Bot::shutdown`] (`None`) or an error
    /// such as a reset or deleted token (`Some`). Nothing follows.
    Closed(Option<Error>),
}

impl BotEvent {
    /// The new message, for [`Event::MessageCreate`].
    #[must_use]
    pub fn message_create(&self) -> Option<&Message> {
        match self {
            Self::Dispatch(Event::MessageCreate(message)) => Some(message),
            _ => None,
        }
    }
}

/// The events of one bot. Ends after [`BotEvent::Closed`].
#[derive(Debug)]
pub struct Events {
    rx: mpsc::UnboundedReceiver<BotEvent>,
}

impl Events {
    /// The next event; `None` once the bot has stopped.
    pub async fn recv(&mut self) -> Option<BotEvent> {
        self.rx.recv().await
    }

    /// The next event if one is waiting.
    pub fn try_recv(&mut self) -> Option<BotEvent> {
        self.rx.try_recv().ok()
    }
}

impl futures::Stream for Events {
    type Item = BotEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<BotEvent>> {
        self.rx.poll_recv(cx)
    }
}

/// Connection settings for [`Bot::connect_with`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ConnectOptions {
    /// How long [`Bot::connect_with`] waits for `ready` and the guild
    /// snapshots.
    pub ready_timeout: Duration,
    /// Timeout of one REST request.
    pub request_timeout: Duration,
    /// Timeout of one WebSocket connection attempt.
    pub connect_timeout: Duration,
    /// Shortest reconnect delay (doubles per failed attempt, with jitter).
    pub reconnect_min: Duration,
    /// Longest reconnect delay.
    pub reconnect_max: Duration,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            ready_timeout: Duration::from_secs(20),
            request_timeout: Duration::from_secs(15),
            connect_timeout: Duration::from_secs(10),
            reconnect_min: Duration::from_millis(500),
            reconnect_max: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lifecycle {
    /// Waiting for `ready` or the guild snapshots it announced.
    Loading,
    /// The cache holds every guild.
    Loaded,
    /// Stopped.
    Closed(Option<Error>),
}

/// State shared by the handles and the gateway task.
pub(crate) struct Shared {
    pub(crate) cache: Cache,
    /// For the answers of [`Interaction`]s.
    rest: Rest,
    /// Dropped after [`BotEvent::Closed`], which ends the stream.
    events: Mutex<Option<mpsc::UnboundedSender<BotEvent>>>,
    /// Guilds announced by `ready` whose snapshot has not arrived.
    pending: Mutex<HashSet<Snowflake>>,
    lifecycle: watch::Sender<Lifecycle>,
}

impl Shared {
    pub(crate) fn emit(&self, event: BotEvent) {
        if let Some(events) = &*self.events.lock().unwrap_or_else(|p| p.into_inner()) {
            let _ = events.send(event);
        }
    }

    pub(crate) fn ready(&self, user: User, guilds: Vec<Guild>) {
        self.cache.reset(user.clone());
        let missing: HashSet<_> = guilds.iter().map(|guild| guild.id).collect();
        let loaded = missing.is_empty();
        *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = missing;
        self.emit(BotEvent::Ready { user, guilds });
        if loaded {
            self.lifecycle.send_replace(Lifecycle::Loaded);
        }
    }

    pub(crate) fn dispatch(&self, event: Event) {
        if let Event::InteractionCreate(interaction) = event {
            self.emit(BotEvent::Interaction(Interaction::new(
                interaction,
                self.rest.clone(),
            )));
            return;
        }
        self.cache.apply(&event);
        if let Event::GuildCreate(snapshot) = &event {
            let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            if pending.remove(&snapshot.guild.id) && pending.is_empty() {
                self.lifecycle.send_replace(Lifecycle::Loaded);
            }
        }
        self.emit(BotEvent::Dispatch(event));
    }

    pub(crate) fn closed(&self, error: Option<Error>) {
        self.emit(BotEvent::Closed(error.clone()));
        self.events.lock().unwrap_or_else(|p| p.into_inner()).take();
        self.lifecycle.send_replace(Lifecycle::Closed(error));
    }
}

struct Inner {
    rest: Rest,
    shared: Arc<Shared>,
    commands: mpsc::UnboundedSender<Command>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
    }
}

/// A connected bot: REST helpers, the [`Cache`] and gateway commands.
/// Cheap to clone; dropping the last clone shuts the gateway down.
///
/// ```no_run
/// use era_bot_sdk::{Bot, BotEvent};
///
/// # async fn run() -> era_bot_sdk::Result<()> {
/// let (bot, mut events) = Bot::connect("https://era.example", "erabot_…").await?;
/// while let Some(event) = events.recv().await {
///     if let Some(message) = event.message_create() {
///         if message.content == "!ping" {
///             bot.reply(message, "pong").await?;
///         }
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Bot {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Bot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bot")
            .field("base", &self.inner.rest.base_url())
            .field("user", &self.inner.shared.cache.me().map(|user| user.id))
            .finish_non_exhaustive()
    }
}

impl Bot {
    /// Connects with default [`ConnectOptions`]: checks the token over REST,
    /// opens the gateway and waits until `ready` and every guild snapshot
    /// arrived (so the cache is complete). Then it keeps the connection up,
    /// reconnecting and resuming on its own.
    pub async fn connect(base_url: &str, token: &str) -> Result<(Self, Events)> {
        Self::connect_with(base_url, token, ConnectOptions::default()).await
    }

    /// [`Bot::connect`] with explicit options.
    pub async fn connect_with(
        base_url: &str,
        token: &str,
        options: ConnectOptions,
    ) -> Result<(Self, Events)> {
        let rest = Rest::with_timeout(base_url, token, options.request_timeout)?;
        // Fails fast (401) on a wrong or reset token.
        let me = rest.me().await?;
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (commands_tx, commands_rx) = mpsc::unbounded_channel();
        let (lifecycle, mut watch) = watch::channel(Lifecycle::Loading);
        let shared = Arc::new(Shared {
            cache: Cache::default(),
            rest: rest.clone(),
            events: Mutex::new(Some(events_tx)),
            pending: Mutex::new(HashSet::new()),
            lifecycle,
        });
        shared.cache.reset(me);
        let task = GatewayTask::new(
            Arc::clone(&shared),
            gateway_url(rest.base_url()),
            Token::new(token.trim()),
            Timing {
                connect_timeout: options.connect_timeout,
                reconnect_min: options.reconnect_min,
                reconnect_max: options.reconnect_max,
            },
            commands_rx,
        );
        tokio::spawn(task.run());
        let bot = Self {
            inner: Arc::new(Inner {
                rest,
                shared,
                commands: commands_tx,
            }),
        };
        let waited = tokio::time::timeout(
            options.ready_timeout,
            watch.wait_for(|state| *state != Lifecycle::Loading),
        )
        .await;
        match waited {
            Ok(Ok(state)) => match &*state {
                Lifecycle::Closed(Some(error)) => Err(error.clone()),
                Lifecycle::Closed(None) => Err(Error::Closed),
                _ => Ok((bot, Events { rx: events_rx })),
            },
            Ok(Err(_)) => Err(Error::Closed),
            Err(_) => Err(Error::ReadyTimeout),
        }
    }

    /// The REST client (every route of the bot API).
    #[must_use]
    pub fn rest(&self) -> &Rest {
        &self.inner.rest
    }

    /// What the bot knows about its guilds.
    #[must_use]
    pub fn cache(&self) -> &Cache {
        &self.inner.shared.cache
    }

    /// The bot's own account.
    #[must_use]
    pub fn user(&self) -> User {
        self.cache()
            .me()
            .expect("the cache knows the bot from connect on")
    }

    /// The bot's user ID.
    #[must_use]
    pub fn id(&self) -> Snowflake {
        self.user().id
    }

    /// Sends a Markdown message to a channel.
    pub async fn send_message(
        &self,
        channel_id: Snowflake,
        content: impl Into<String>,
    ) -> Result<Message> {
        self.rest().send_message(channel_id, content).await
    }

    /// Answers a message with a reply in its channel: clients show the
    /// answer with a quote of `to` (§8.1). Its author is not pinged, which
    /// suits answers to commands; [`Bot::reply_mention`] pings them.
    pub async fn reply(&self, to: &Message, content: impl Into<String>) -> Result<Message> {
        self.rest()
            .send_reply(to.channel_id, to.id, content, false)
            .await
    }

    /// [`Bot::reply`] that pings the author of `to`: it counts as a mention
    /// for them (highlight, notification, mention badge).
    pub async fn reply_mention(&self, to: &Message, content: impl Into<String>) -> Result<Message> {
        self.rest()
            .send_reply(to.channel_id, to.id, content, true)
            .await
    }

    /// Edits one of the bot's messages.
    pub async fn edit_message(
        &self,
        message: &Message,
        content: impl Into<String>,
    ) -> Result<Message> {
        self.rest()
            .edit_message(message.channel_id, message.id, content)
            .await
    }

    /// Sends a message with rows of buttons under it
    /// ([`components`](crate::components)); clicks come as
    /// [`BotEvent::Interaction`].
    pub async fn send_components(
        &self,
        channel_id: Snowflake,
        content: impl Into<String>,
        components: Vec<ActionRow>,
    ) -> Result<Message> {
        self.rest()
            .send_components(channel_id, content, components)
            .await
    }

    /// Edits one of the bot's messages: new text and new buttons (`[]`
    /// removes them).
    pub async fn edit_components(
        &self,
        message: &Message,
        content: impl Into<String>,
        components: Vec<ActionRow>,
    ) -> Result<Message> {
        self.rest()
            .edit_message_with(
                message.channel_id,
                message.id,
                &UpdateMessageRequest {
                    content: content.into(),
                    components: Some(components),
                },
            )
            .await
    }

    /// Deletes a message (the bot's own, or any with `MANAGE_MESSAGES`).
    pub async fn delete_message(&self, message: &Message) -> Result<()> {
        self.rest()
            .delete_message(message.channel_id, message.id)
            .await
    }

    /// Shows "typing…" in a channel for a few seconds.
    pub async fn typing(&self, channel_id: Snowflake) -> Result<()> {
        self.rest().typing(channel_id).await
    }

    /// Sets the bot's status (kept across reconnects).
    pub fn set_status(&self, status: StatusSetting) {
        let _ = self.inner.commands.send(Command::Status(status));
    }

    /// Closes the gateway cleanly (others see the bot offline at once) and
    /// waits up to 5 s for it. The event stream ends with
    /// [`BotEvent::Closed`].
    pub async fn shutdown(&self) {
        let _ = self.inner.commands.send(Command::Shutdown);
        let mut watch = self.inner.shared.lifecycle.subscribe();
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            watch.wait_for(|state| matches!(state, Lifecycle::Closed(_))),
        )
        .await;
    }

    /// Whether a message was written by this bot.
    #[must_use]
    pub fn is_own(&self, message: &Message) -> bool {
        message.author_id == self.id()
    }

    /// Whether a message was written by any bot (this one included), as far
    /// as the cache knows.
    #[must_use]
    pub fn is_from_bot(&self, message: &Message) -> bool {
        self.cache()
            .user(message.author_id)
            .is_some_and(|user| user.bot)
            || self.is_own(message)
    }
}

/// The Markdown that mentions (pings) a user: `<@id>`.
#[must_use]
pub fn mention(user_id: Snowflake) -> String {
    format!("<@{user_id}>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions() {
        assert_eq!(mention(Snowflake::new(42)), "<@42>");
    }
}
