//! Buttons: `!sayaç` posts a counter with buttons under it. "➕" counts up
//! (the message updates for everyone), "Sıfırla" sets it back, "Kaç oldu?"
//! answers only the person who clicked.
//!
//! ```text
//! ERA_URL=https://era.example ERA_BOT_TOKEN=erabot_… cargo run -p era-bot-sdk --example buttons
//! ```

use era_bot_sdk::components::{ActionRow, Button, row};
use era_bot_sdk::protocol::Event;
use era_bot_sdk::{Bot, BotEvent};

/// The counter's text and buttons for `n`.
fn counter(n: u32) -> (String, Vec<ActionRow>) {
    (
        format!("Sayaç: **{n}**"),
        vec![row([
            Button::primary("count:plus", "Bir daha").emoji("➕"),
            Button::danger("count:zero", "Sıfırla").disabled(n == 0),
            Button::secondary("count:show", "Kaç oldu?"),
        ])],
    )
}

/// The number in a counter message.
fn value(content: &str) -> u32 {
    content
        .trim_start_matches("Sayaç: **")
        .trim_end_matches("**")
        .parse()
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let url = std::env::var("ERA_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let token = std::env::var("ERA_BOT_TOKEN")
        .map_err(|_| anyhow::anyhow!("set ERA_BOT_TOKEN to the bot's erabot_… token"))?;
    let (bot, mut events) = Bot::connect(&url, &token).await?;
    log::info!("connected as {}", bot.user().username);

    while let Some(event) = events.recv().await {
        match event {
            BotEvent::Dispatch(Event::MessageCreate(message))
                if !bot.is_from_bot(&message) && message.content.trim() == "!sayaç" =>
            {
                let (text, buttons) = counter(0);
                if let Err(error) = bot.send_components(message.channel_id, text, buttons).await {
                    log::warn!("sending failed: {error}");
                }
            }
            // Answer every click within 3 s, or the person sees an error.
            BotEvent::Interaction(click) => {
                let n = value(&click.message.content);
                let answered = match click.custom_id.as_str() {
                    "count:plus" | "count:zero" => {
                        let next = if click.custom_id == "count:plus" { n + 1 } else { 0 };
                        let (text, buttons) = counter(next);
                        click.update_message(text, buttons).await.map(drop)
                    }
                    "count:show" => click
                        .reply_ephemeral(format!("Şu an {n} 🙂 (bunu sadece sen görüyorsun)"))
                        .await
                        .map(drop),
                    _ => click.defer().await,
                };
                if let Err(error) = answered {
                    log::warn!("answering a click failed: {error}");
                }
            }
            BotEvent::Closed(reason) => {
                log::warn!("stopped: {reason:?}");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
