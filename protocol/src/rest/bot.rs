use serde::{Deserialize, Serialize};

use crate::gateway::Token;
use crate::model::User;
use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// A guild's bot account (§4.4, ADR-046): a [`User`] with `bot = true` that
/// belongs to exactly one guild and is a member of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bot {
    /// The bot's account. `displayName` is the name given at creation; the
    /// bot may change it (and its avatar) itself through `/v1/users/@me`.
    pub user: User,
    /// The guild it belongs to.
    pub guild_id: Snowflake,
    /// Who created it; `None` once that account is gone.
    pub creator_id: Option<Snowflake>,
    /// When it was created.
    pub created_at: Timestamp,
}

/// `GET /v1/guilds/{guild}/bots` (needs `MANAGE_GUILD`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildBots {
    /// The guild's bots, oldest first.
    pub bots: Vec<Bot>,
    /// The plan's `max_bots`; `None` means unlimited.
    pub max_bots: Option<u64>,
}

/// `POST /v1/guilds/{guild}/bots` (needs `MANAGE_GUILD`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBotRequest {
    /// The bot's name (its display name).
    pub name: String,
}

/// A bot with its token: the answer of bot creation and token reset, the
/// only time the token is ever shown (only its hash is stored). REST calls
/// send it as `Authorization: Bot <token>`; the gateway takes it in
/// `identify`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotToken {
    /// The bot.
    pub bot: Bot,
    /// Its new token.
    pub token: Token,
}
