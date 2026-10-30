//! # era-bot-sdk
//!
//! The Rust SDK for Era bots (architecture.md §4.4). Bots are customer-run:
//! a guild manager creates one in *Server settings → Botlar*, copies its
//! `erabot_…` token, and the bot connects from any machine over the same
//! REST API and gateway as the apps.
//!
//! - [`Bot::connect`]: checks the token, opens the gateway (identify,
//!   heartbeats, reconnect with jittered backoff, resume) and waits until
//!   the guild snapshots are in the [`Cache`].
//! - [`Events`]: every [`BotEvent`] in order (ready, guild snapshots,
//!   messages, voice states, members, ...), as a stream.
//! - [`Rest`]: messages (send, reply, edit, delete, typing), guild,
//!   channels, members, voice states, own profile and avatar.
//! - [`components`]: rows of buttons under the bot's messages; clicks come
//!   as [`BotEvent::Interaction`], answered through [`Interaction`]
//!   (update the message, an ephemeral reply only the clicker sees, or
//!   defer).
//! - [`command`]: prefix commands with aliases (`!çal x` / `!play x`).
//! - `voice` (feature `voice`): join a voice channel, publish a stereo
//!   48 kHz music track at the bitrate the guild's plan allows, push PCM,
//!   listen to others, leave.
//!
//! The wire types are the `protocol` crate, re-exported as [`protocol`].
//!
//! ```no_run
//! use era_bot_sdk::command::Commands;
//! use era_bot_sdk::{Bot, BotEvent};
//!
//! #[derive(Clone, Copy)]
//! enum Cmd { Ping, Echo }
//!
//! # async fn run() -> era_bot_sdk::Result<()> {
//! let commands = Commands::new("!")
//!     .add(Cmd::Ping, &["ping"])
//!     .add(Cmd::Echo, &["echo", "yankı"]);
//! let (bot, mut events) = Bot::connect("https://era.example", "erabot_…").await?;
//! while let Some(event) = events.recv().await {
//!     let Some(message) = event.message_create() else { continue };
//!     if bot.is_from_bot(message) {
//!         continue;
//!     }
//!     match commands.parse(&message.content) {
//!         Some((Cmd::Ping, _)) => drop(bot.reply(message, "pong").await?),
//!         Some((Cmd::Echo, command)) => drop(bot.reply(message, command.rest).await?),
//!         None => {}
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Buttons and interactions
//!
//! Bots (only bots) can put up to 5 rows of 5 buttons under their messages.
//! A click reaches only the bot that owns the message, as
//! [`BotEvent::Interaction`]; answer it within 3 s by editing the message,
//! with a message only the clicker sees, or by deferring. The server checks
//! that the clicker can see the channel and rate-limits clicks; the bot
//! applies its own rules (Panda: be in its voice channel).
//!
//! ```no_run
//! use era_bot_sdk::components::{Button, row};
//! use era_bot_sdk::{Bot, BotEvent};
//!
//! # async fn run(bot: Bot, mut events: era_bot_sdk::Events, channel: era_bot_sdk::protocol::Snowflake) -> era_bot_sdk::Result<()> {
//! let buttons = |n: u32| vec![row([
//!     Button::primary("plus", "Bir daha").emoji("➕"),
//!     Button::secondary("show", "Kaç oldu?"),
//! ])];
//! bot.send_components(channel, "Sayaç: 0", buttons(0)).await?;
//! while let Some(event) = events.recv().await {
//!     let BotEvent::Interaction(click) = event else { continue };
//!     let n: u32 = click.message.content.trim_start_matches("Sayaç: ").parse().unwrap_or(0);
//!     match click.custom_id.as_str() {
//!         "plus" => drop(click.update_message(format!("Sayaç: {}", n + 1), buttons(n + 1)).await?),
//!         _ => drop(click.reply_ephemeral(format!("{n}, {}", click.user.shown_name())).await?),
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Permissions and limits
//!
//! A bot is a guild member: what it may do comes from its roles, like
//! anyone else (sending needs `SEND_MESSAGES`, voice needs `CONNECT` and
//! `SPEAK`). It cannot log in with a password, join other guilds, use DMs
//! or friends, or manage bots. Its voice slot counts toward the plan's
//! concurrent voice limit.
//!
//! ## TLS
//!
//! HTTPS and WSS use rustls with the ring provider, which the SDK installs
//! as the process default if none is installed yet.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cache;
mod client;
pub mod command;
pub mod components;
mod error;
mod gateway;
mod interaction;
pub mod rest;
#[cfg(feature = "voice")]
pub mod voice;

pub use cache::{Cache, shown_name};
pub use client::{Bot, BotEvent, ConnectOptions, Events, mention};
pub use error::{Error, Result};
pub use interaction::Interaction;
pub use protocol;
pub use rest::Rest;
