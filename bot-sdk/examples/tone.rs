//! Voice example: `!tone` joins the caller's voice channel and plays a
//! 440 Hz tone (left) and 660 Hz (right) for five seconds, then leaves.
//!
//! ```text
//! ERA_URL=… ERA_BOT_TOKEN=erabot_… cargo run -p era-bot-sdk --features voice --example tone
//! ```

use std::f32::consts::TAU;

use era_bot_sdk::voice::{FRAME_SAMPLES, SAMPLE_RATE, VoiceConnection, VoiceOptions};
use era_bot_sdk::{Bot, BotEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let url = std::env::var("ERA_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let token = std::env::var("ERA_BOT_TOKEN")
        .map_err(|_| anyhow::anyhow!("set ERA_BOT_TOKEN to the bot's erabot_… token"))?;
    let (bot, mut events) = Bot::connect(&url, &token).await?;

    while let Some(event) = events.recv().await {
        let Some(message) = event.message_create().cloned() else {
            if matches!(event, BotEvent::Closed(_)) {
                break;
            }
            continue;
        };
        if message.content.trim() != "!tone" || bot.is_from_bot(&message) {
            continue;
        }
        let guild = bot.cache().guild_of_channel(message.channel_id);
        let Some(channel) = guild.and_then(|g| bot.cache().voice_channel_of(g, message.author_id))
        else {
            bot.reply(&message, "Önce bir ses kanalına gir 🙂").await?;
            continue;
        };
        let voice = VoiceConnection::join(&bot, channel, VoiceOptions::default()).await?;
        bot.reply(&message, format!("🎵 {} kbit/s", voice.bitrate_kbps()))
            .await?;
        let mut phase = (0.0_f32, 0.0_f32);
        let mut frame = vec![0_i16; FRAME_SAMPLES];
        for _ in 0..500 {
            for [left, right] in frame.as_chunks_mut::<2>().0 {
                *left = (phase.0.sin() * 8000.0) as i16;
                *right = (phase.1.sin() * 8000.0) as i16;
                phase.0 = (phase.0 + TAU * 440.0 / SAMPLE_RATE as f32) % TAU;
                phase.1 = (phase.1 + TAU * 660.0 / SAMPLE_RATE as f32) % TAU;
            }
            voice.push(&frame).await?;
        }
        voice.leave().await;
    }
    Ok(())
}
