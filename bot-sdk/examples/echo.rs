//! A minimal chat bot: `!echo <text>` repeats the text, `!ping` answers
//! `pong`, `!kim` / `!whoami` says who asked.
//!
//! ```text
//! ERA_URL=https://era.example ERA_BOT_TOKEN=erabot_… cargo run -p era-bot-sdk --example echo
//! ```

use era_bot_sdk::command::Commands;
use era_bot_sdk::protocol::Event;
use era_bot_sdk::{Bot, BotEvent, mention};

#[derive(Debug, Clone, Copy)]
enum Cmd {
    Echo,
    Ping,
    WhoAmI,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let url = std::env::var("ERA_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let token = std::env::var("ERA_BOT_TOKEN")
        .map_err(|_| anyhow::anyhow!("set ERA_BOT_TOKEN to the bot's erabot_… token"))?;

    let commands = Commands::new("!")
        .add(Cmd::Echo, &["echo", "yankı", "yanki"])
        .add(Cmd::Ping, &["ping"])
        .add(Cmd::WhoAmI, &["kim", "whoami"]);
    let (bot, mut events) = Bot::connect(&url, &token).await?;
    log::info!("connected as {}", bot.user().username);

    while let Some(event) = events.recv().await {
        match event {
            BotEvent::Dispatch(Event::MessageCreate(message)) => {
                if bot.is_from_bot(&message) {
                    continue;
                }
                let Some((command, parsed)) = commands.parse(&message.content) else {
                    continue;
                };
                let answer = match command {
                    Cmd::Echo if parsed.rest.is_empty() => "Ne diyeyim? `!echo merhaba`".to_owned(),
                    Cmd::Echo => parsed.rest.to_owned(),
                    Cmd::Ping => "pong 🏓".to_owned(),
                    Cmd::WhoAmI => {
                        let guild = bot.cache().guild_of_channel(message.channel_id);
                        let name = guild.map_or_else(
                            || message.author_id.to_string(),
                            |guild| bot.cache().display_name(guild, message.author_id),
                        );
                        format!("Sen {} ({name}) 🙂", mention(message.author_id))
                    }
                };
                if let Err(error) = bot.reply(&message, answer).await {
                    log::warn!("reply failed: {error}");
                }
            }
            BotEvent::Disconnected => log::info!("connection lost, reconnecting"),
            BotEvent::Resumed => log::info!("resumed"),
            BotEvent::Closed(reason) => {
                log::warn!("stopped: {reason:?}");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
