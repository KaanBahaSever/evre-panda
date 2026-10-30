//! The event loop: chat commands and the panel's buttons to the guilds'
//! players.
//!
//! Playback controls (skip, stop, pause, volume, shuffle, repeat, the
//! queue's edits, leaving) need the caller in Panda's voice channel while
//! Panda is in one, from a command or a button alike; reading (the queue,
//! now playing, help) is for everyone.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use era_bot_sdk::command::Commands;
use era_bot_sdk::protocol::{Event, Message, Snowflake};
use era_bot_sdk::{Bot, BotEvent, Interaction};

use crate::config::Config;
use crate::lang::{self, Locale, fill, strings};
use crate::media;
use crate::panel::{self, Action};
use crate::player::{Player, Queued, Refusal};
use crate::queue::{LoopMode, Track, next_id};
use crate::store::Store;
use crate::text;

/// Panda's commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// `!çal`
    Play,
    /// `!geç`
    Skip,
    /// `!dur`
    Stop,
    /// `!duraklat`
    Pause,
    /// `!devam`
    Resume,
    /// `!sıra`
    Queue,
    /// `!şimdi`
    NowPlaying,
    /// `!ses`
    Volume,
    /// `!karıştır`
    Shuffle,
    /// `!tekrar`
    Loop,
    /// `!gel`
    Join,
    /// `!çık`
    Leave,
    /// `!sil`
    Remove,
    /// `!temizle`
    Clear,
    /// `!yardım`
    Help,
}

impl Cmd {
    /// Whether it changes playback (and so needs the caller in Panda's
    /// voice channel). `!ses` and `!tekrar` read without an argument.
    #[must_use]
    pub const fn controls_playback(self, has_args: bool) -> bool {
        match self {
            Self::Skip
            | Self::Stop
            | Self::Pause
            | Self::Resume
            | Self::Shuffle
            | Self::Leave
            | Self::Remove
            | Self::Clear => true,
            Self::Volume | Self::Loop => has_args,
            Self::Play | Self::Queue | Self::NowPlaying | Self::Join | Self::Help => false,
        }
    }
}

/// The command names, all active whatever `PANDA_LANG` says: Turkish
/// first, the same without Turkish letters (for keyboards without them),
/// then English, French, German and Spanish (also without accents). No name
/// may appear twice (a test checks).
pub const ALIASES: &[(Cmd, &[&str])] = &[
    (
        Cmd::Play,
        &[
            "çal",
            "cal",
            "play",
            "p",
            "jouer",
            "joue",
            "spielen",
            "spiel",
            "reproducir",
            "pon",
        ],
    ),
    (
        Cmd::Skip,
        &[
            "geç",
            "gec",
            "skip",
            "next",
            "passer",
            "suivant",
            "überspringen",
            "ueberspringen",
            "nächster",
            "naechster",
            "saltar",
            "siguiente",
        ],
    ),
    (
        Cmd::Stop,
        &[
            "dur", "stop", "arrêter", "arreter", "arrête", "arrete", "stopp", "parar", "detener",
        ],
    ),
    (
        Cmd::Pause,
        &["duraklat", "pause", "pausieren", "pausa", "pausar"],
    ),
    (
        Cmd::Resume,
        &[
            "devam",
            "resume",
            "unpause",
            "reprendre",
            "continuer",
            "fortsetzen",
            "weiter",
            "reanudar",
            "continuar",
        ],
    ),
    (
        Cmd::Queue,
        &[
            "sıra",
            "sira",
            "queue",
            "q",
            "file",
            "liste",
            "warteschlange",
            "cola",
            "lista",
        ],
    ),
    (
        Cmd::NowPlaying,
        &[
            "şimdi",
            "simdi",
            "np",
            "nowplaying",
            "maintenant",
            "encours",
            "jetzt",
            "ahora",
        ],
    ),
    (
        Cmd::Volume,
        &[
            "ses",
            "volume",
            "vol",
            "lautstärke",
            "lautstaerke",
            "volumen",
        ],
    ),
    (
        Cmd::Shuffle,
        &[
            "karıştır",
            "karistir",
            "shuffle",
            "mélanger",
            "melanger",
            "mischen",
            "mezclar",
        ],
    ),
    (
        Cmd::Loop,
        &[
            "tekrar",
            "loop",
            "repeat",
            "boucle",
            "répéter",
            "repeter",
            "wiederholen",
            "repetir",
            "bucle",
        ],
    ),
    (
        Cmd::Join,
        &[
            "gel",
            "join",
            "rejoindre",
            "viens",
            "komm",
            "beitreten",
            "ven",
        ],
    ),
    (
        Cmd::Leave,
        &[
            "çık",
            "cik",
            "leave",
            "disconnect",
            "quitter",
            "partir",
            "verlassen",
            "raus",
            "salir",
        ],
    ),
    (
        Cmd::Remove,
        &[
            "sil",
            "remove",
            "retirer",
            "supprimer",
            "entfernen",
            "quitar",
            "eliminar",
        ],
    ),
    (
        Cmd::Clear,
        &["temizle", "clear", "vider", "leeren", "vaciar", "limpiar"],
    ),
    (
        Cmd::Help,
        &[
            "yardım",
            "yardim",
            "help",
            "komutlar",
            "aide",
            "commandes",
            "hilfe",
            "befehle",
            "ayuda",
            "comandos",
        ],
    ),
];

/// The commands under every name of [`ALIASES`].
#[must_use]
pub fn commands(prefix: &str) -> Commands<Cmd> {
    ALIASES
        .iter()
        .fold(Commands::new(prefix), |commands, (command, names)| {
            commands.add(*command, names)
        })
}

/// A volume from `150`, `%150` or `150%`.
#[must_use]
pub fn parse_volume(text: &str) -> Option<u32> {
    text.trim()
        .trim_start_matches('%')
        .trim_end_matches('%')
        .trim()
        .parse()
        .ok()
}

/// One guild's player plus a lock that keeps `!çal` lookups in order.
#[derive(Clone)]
struct Guild {
    player: Player,
    lookups: Arc<tokio::sync::Mutex<()>>,
}

/// Runs Panda until Ctrl+C (or SIGTERM on Unix).
pub async fn run(config: Config) -> Result<(), String> {
    run_until(config, shutdown_signal()).await
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

/// Runs Panda until `shutdown` completes (tests use their own signal).
pub async fn run_until(config: Config, shutdown: impl Future<Output = ()>) -> Result<(), String> {
    std::fs::create_dir_all(&config.data_dir)
        .map_err(|error| format!("cannot create {}: {error}", config.data_dir.display()))?;
    match media::version(&config.ytdlp, "--version").await {
        Some(version) => log::info!("yt-dlp {version}"),
        None => log::warn!(
            "yt-dlp not found ({}): only direct audio links will play",
            config.ytdlp.display()
        ),
    }
    match media::version(&config.ffmpeg, "-version").await {
        Some(version) => log::info!("{version}"),
        None => log::warn!(
            "ffmpeg not found ({}): nothing will play",
            config.ffmpeg.display()
        ),
    }
    if config.ytdlp_auto_update {
        let config = config.clone();
        tokio::spawn(async move {
            loop {
                media::update_ytdlp(&config).await;
                tokio::time::sleep(Duration::from_secs(24 * 3600)).await;
            }
        });
    }

    let (bot, mut events) = era_bot_sdk::Bot::connect(&config.era_url, &config.token)
        .await
        .map_err(|error| format!("cannot connect to Era: {error}"))?;
    log::info!(
        "Panda is online as {} in {} guild(s)",
        bot.user().username,
        bot.cache().guild_ids().len()
    );
    let store = Arc::new(Store::open(&config.data_dir, config.default_volume));
    let config = Arc::new(config);
    let commands = Arc::new(commands(&config.prefix));
    let mut guilds: HashMap<Snowflake, Guild> = HashMap::new();
    let mut guild_for = |guild_id: Snowflake| -> Guild {
        guilds
            .entry(guild_id)
            .or_insert_with(|| Guild {
                player: Player::new(
                    bot.clone(),
                    Arc::clone(&config),
                    Arc::clone(&store),
                    guild_id,
                ),
                lookups: Arc::new(tokio::sync::Mutex::new(())),
            })
            .clone()
    };
    let mut seen = Vec::new();

    tokio::pin!(shutdown);
    let result = loop {
        let event = tokio::select! {
            () = &mut shutdown => break Ok(()),
            event = events.recv() => event,
        };
        match event {
            None => break Err("the gateway stopped".to_owned()),
            Some(BotEvent::Closed(error)) => {
                break Err(format!(
                    "Era ended the session: {}",
                    error.map_or_else(|| "shut down".to_owned(), |e| e.to_string())
                ));
            }
            Some(BotEvent::Dispatch(Event::MessageCreate(message))) => {
                if bot.is_from_bot(&message) {
                    continue;
                }
                let Some((command, parsed)) = commands.parse(&message.content) else {
                    continue;
                };
                // DMs have no guild; bots cannot use them anyway.
                let Some(guild_id) = bot.cache().guild_of_channel(message.channel_id) else {
                    continue;
                };
                let rest = parsed.rest.to_owned();
                let args = parsed.args.clone();
                let guild = guild_for(guild_id);
                let bot = bot.clone();
                let config = Arc::clone(&config);
                tokio::spawn(async move {
                    let reply = handle(
                        &bot, &config, &guild, guild_id, &message, command, &rest, &args,
                    )
                    .await;
                    if let Some(reply) = reply
                        && let Err(error) = bot.reply(&message, reply).await
                    {
                        log::warn!("replying failed: {error}");
                    }
                });
            }
            Some(BotEvent::Interaction(interaction)) => {
                let guild = guild_for(interaction.guild_id);
                let bot = bot.clone();
                tokio::spawn(async move { on_button(&bot, &guild, interaction).await });
            }
            Some(BotEvent::Dispatch(Event::VoiceStateUpdate(state)))
                if state.user_id == bot.id() && state.moved_by.is_some() =>
            {
                if let Some(channel) = state.channel_id {
                    let player = guild_for(state.guild_id).player;
                    tokio::spawn(async move { player.moved_to(channel).await });
                }
            }
            Some(BotEvent::Dispatch(Event::GuildCreate(snapshot))) => {
                if !seen.contains(&snapshot.guild.id) {
                    seen.push(snapshot.guild.id);
                    log::info!("guild {} ({})", snapshot.guild.name, snapshot.guild.id);
                }
            }
            Some(BotEvent::Disconnected) => log::info!("gateway connection lost, reconnecting"),
            Some(BotEvent::Resumed) => log::info!("gateway resumed"),
            Some(_) => {}
        }
    };
    for guild in guilds.values() {
        guild.player.shutdown().await;
    }
    bot.shutdown().await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn handle(
    bot: &Bot,
    config: &Config,
    guild: &Guild,
    guild_id: Snowflake,
    message: &Message,
    command: Cmd,
    rest: &str,
    args: &[String],
) -> Option<String> {
    let player = &guild.player;
    let p = config.prefix.as_str();
    let lang = config.lang;
    let s = strings(lang);
    // A text with the prefix filled in.
    let with_p = |template: &str| fill(template, &[("p", &p)]);
    let caller_channel = || bot.cache().voice_channel_of(guild_id, message.author_id);
    if command.controls_playback(!args.is_empty())
        && let Err(refusal) = in_panda_channel(bot, lang, guild_id, player, message.author_id).await
    {
        return Some(refusal);
    }
    match command {
        Cmd::Play => {
            if rest.is_empty() {
                return Some(with_p(s.play_what));
            }
            let caller = caller_channel();
            if caller.is_none() && player.voice_channel().await.is_none() {
                return Some(s.voice_first_call.into());
            }
            let _in_order = guild.lookups.lock().await;
            let _ = bot.typing(message.channel_id).await;
            let query = media::classify(rest);
            let found = match media::lookup(config, &query).await {
                Ok(found) => found,
                Err(reason) => {
                    log::info!("lookup of {query:?} failed: {reason}");
                    return Some(text::media_error(lang, &reason));
                }
            };
            let track = Track {
                id: next_id(),
                title: found.title,
                source: found.source,
                link: found.link,
                duration: found.duration,
                live: found.live,
                requester: message.author_id,
                requester_name: bot.cache().display_name(guild_id, message.author_id),
                text_channel: message.channel_id,
            };
            let shown = track.clone();
            match player.play(track, caller).await {
                Ok(Queued::Now) => None,
                Ok(Queued::At(position)) => Some(text::queued(lang, &shown, position)),
                Err(Refusal(reason)) => Some(reason),
            }
        }
        Cmd::Skip => Some(match player.skip().await {
            Some(_) => s.skipped.into(),
            None => s.nothing_playing.into(),
        }),
        Cmd::Stop => Some(if player.stop().await {
            s.stopped.into()
        } else {
            s.stop_nothing.into()
        }),
        Cmd::Pause => Some(if player.pause().await {
            with_p(s.paused)
        } else {
            s.pause_nothing.into()
        }),
        Cmd::Resume => Some(if player.resume().await {
            s.resumed.into()
        } else {
            s.resume_nothing.into()
        }),
        Cmd::Queue => Some(player.queue_text().await),
        Cmd::NowPlaying => Some(
            player
                .now_text()
                .await
                .unwrap_or_else(|| s.nothing_playing.into()),
        ),
        Cmd::Volume => Some(match args.first() {
            None => fill(
                s.volume_is,
                &[("volume", &lang::percent(lang, player.volume(None)))],
            ),
            Some(value) => match parse_volume(value) {
                Some(percent) if percent <= 200 => fill(
                    s.volume_set,
                    &[("volume", &lang::percent(lang, player.volume(Some(percent))))],
                ),
                _ => with_p(s.volume_range),
            },
        }),
        Cmd::Shuffle => Some(match player.shuffle().await {
            0 | 1 => s.shuffle_few.into(),
            count => fill(s.shuffled, &[("songs", &lang::songs(lang, count))]),
        }),
        Cmd::Loop => {
            let mode = match args.first() {
                None => None,
                Some(word) => match LoopMode::parse(word) {
                    Some(mode) => Some(mode),
                    None => return Some(with_p(s.loop_usage)),
                },
            };
            Some(text::loop_mode(lang, player.set_loop(mode).await).into())
        }
        Cmd::Join => {
            let Some(channel) = caller_channel() else {
                return Some(s.voice_first.into());
            };
            Some(match player.join(channel, message.channel_id).await {
                Ok(true) => s.coming.into(),
                Ok(false) => s.already_here.into(),
                Err(Refusal(reason)) => reason,
            })
        }
        Cmd::Leave => Some(if player.leave().await {
            with_p(s.left)
        } else {
            s.not_in_voice.into()
        }),
        Cmd::Remove => {
            let Some(position) = args
                .first()
                .and_then(|n| n.trim_start_matches('#').parse::<usize>().ok())
            else {
                return Some(with_p(s.remove_usage));
            };
            Some(match player.remove(position).await {
                Some(track) => fill(
                    s.removed,
                    &[("title", &text::clean_title(lang, &track.title))],
                ),
                None => fill(s.no_such, &[("n", &position)]),
            })
        }
        Cmd::Clear => Some(match player.clear().await {
            0 => s.clear_empty.into(),
            count => fill(s.cleared, &[("songs", &lang::songs(lang, count))]),
        }),
        Cmd::Help => Some(text::help(lang, p)),
    }
}

/// Playback controls need `user_id` in Panda's voice channel while Panda is
/// in one (otherwise there is nothing to control, and the command says so).
async fn in_panda_channel(
    bot: &Bot,
    lang: Locale,
    guild_id: Snowflake,
    player: &Player,
    user_id: Snowflake,
) -> Result<(), String> {
    match player.voice_channel().await {
        Some(channel) if bot.cache().voice_channel_of(guild_id, user_id) != Some(channel) => {
            Err(text::not_in_channel(lang).to_owned())
        }
        _ => Ok(()),
    }
}

/// A click on a panel button: the same action as the command, under the
/// same rule, answered within the interaction deadline. The panel updates
/// in place; the queue and refusals go to the clicker alone.
async fn on_button(bot: &Bot, guild: &Guild, interaction: Interaction) {
    if let Err(error) = answer_button(bot, guild, &interaction).await {
        log::warn!("answering a button failed: {error}");
    }
}

async fn answer_button(
    bot: &Bot,
    guild: &Guild,
    interaction: &Interaction,
) -> era_bot_sdk::Result<()> {
    let player = &guild.player;
    let lang = player.lang();
    let Some(action) = Action::parse(&interaction.custom_id) else {
        interaction
            .reply_ephemeral(strings(lang).unknown_button)
            .await?;
        return Ok(());
    };
    if action == Action::Queue {
        interaction
            .reply_ephemeral(player.queue_text().await)
            .await?;
        return Ok(());
    }
    let message_id = interaction.message.id;
    if !player.is_panel(message_id) {
        // An older panel (another song, or from before a restart).
        interaction
            .update_message(panel::ended(&interaction.message.content), Vec::new())
            .await?;
        return Ok(());
    }
    if let Err(refusal) =
        in_panda_channel(bot, lang, interaction.guild_id, player, interaction.user.id).await
    {
        interaction.reply_ephemeral(refusal).await?;
        return Ok(());
    }
    match action {
        Action::Pause => {
            if player.is_paused() {
                player.resume().await;
            } else {
                player.pause().await;
            }
        }
        Action::Skip => {
            player.skip().await;
        }
        Action::Stop => {
            // Leaving the room can take a moment: answer first.
            interaction.defer().await?;
            player.stop().await;
            return Ok(());
        }
        Action::Shuffle => {
            player.shuffle().await;
        }
        Action::Loop => {
            player.set_loop(None).await;
        }
        Action::VolumeDown | Action::VolumeUp => {
            let volume = panel::stepped_volume(player.volume(None), action == Action::VolumeUp);
            player.volume(Some(volume));
        }
        Action::Queue => {}
    }
    match player.panel_now(message_id).await {
        Some((content, components)) => {
            interaction
                .update_message(content.clone(), components.clone())
                .await?;
            player.panel_shown(message_id, content, components);
        }
        // The song is over (skipped to the next, or the queue ended): the
        // panel task closes this panel and posts the next one.
        None => interaction.defer().await?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turkish_and_english_names() {
        let commands = commands("!");
        for (input, expected) in [
            ("!çal barış manço", Cmd::Play),
            ("!cal x", Cmd::Play),
            ("!play x", Cmd::Play),
            ("!p x", Cmd::Play),
            ("!GEÇ", Cmd::Skip),
            ("!skip", Cmd::Skip),
            ("!dur", Cmd::Stop),
            ("!stop", Cmd::Stop),
            ("!duraklat", Cmd::Pause),
            ("!pause", Cmd::Pause),
            ("!devam", Cmd::Resume),
            ("!resume", Cmd::Resume),
            ("!sıra", Cmd::Queue),
            ("!queue", Cmd::Queue),
            ("!şimdi", Cmd::NowPlaying),
            ("!np", Cmd::NowPlaying),
            ("!ses 150", Cmd::Volume),
            ("!volume 150", Cmd::Volume),
            ("!karıştır", Cmd::Shuffle),
            ("!shuffle", Cmd::Shuffle),
            ("!tekrar hepsi", Cmd::Loop),
            ("!loop all", Cmd::Loop),
            ("!gel", Cmd::Join),
            ("!join", Cmd::Join),
            ("!çık", Cmd::Leave),
            ("!leave", Cmd::Leave),
            ("!yardım", Cmd::Help),
            ("!help", Cmd::Help),
        ] {
            assert_eq!(
                commands.parse(input).map(|(c, _)| c),
                Some(expected),
                "{input}"
            );
        }
        assert!(commands.parse("çal x").is_none());
        assert!(commands.parse("!dans").is_none());
        let (_, parsed) = commands.parse("!çal https://youtu.be/x?t=3").unwrap();
        assert_eq!(parsed.rest, "https://youtu.be/x?t=3");
    }

    #[test]
    fn english_french_german_and_spanish_names() {
        let commands = commands("!");
        for (input, expected) in [
            // English.
            ("!play x", Cmd::Play),
            ("!skip", Cmd::Skip),
            ("!next", Cmd::Skip),
            ("!stop", Cmd::Stop),
            ("!pause", Cmd::Pause),
            ("!resume", Cmd::Resume),
            ("!queue", Cmd::Queue),
            ("!q", Cmd::Queue),
            ("!volume 80", Cmd::Volume),
            ("!vol 80", Cmd::Volume),
            ("!loop one", Cmd::Loop),
            ("!repeat", Cmd::Loop),
            ("!shuffle", Cmd::Shuffle),
            ("!join", Cmd::Join),
            ("!leave", Cmd::Leave),
            ("!np", Cmd::NowPlaying),
            ("!remove 2", Cmd::Remove),
            ("!clear", Cmd::Clear),
            ("!help", Cmd::Help),
            // French, with and without accents.
            ("!jouer daft punk", Cmd::Play),
            ("!passer", Cmd::Skip),
            ("!suivant", Cmd::Skip),
            ("!arrêter", Cmd::Stop),
            ("!ARRETER", Cmd::Stop),
            ("!reprendre", Cmd::Resume),
            ("!file", Cmd::Queue),
            ("!maintenant", Cmd::NowPlaying),
            ("!mélanger", Cmd::Shuffle),
            ("!melanger", Cmd::Shuffle),
            ("!boucle tout", Cmd::Loop),
            ("!rejoindre", Cmd::Join),
            ("!quitter", Cmd::Leave),
            ("!retirer 3", Cmd::Remove),
            ("!vider", Cmd::Clear),
            ("!aide", Cmd::Help),
            // German and Spanish.
            ("!spielen x", Cmd::Play),
            ("!überspringen", Cmd::Skip),
            ("!stopp", Cmd::Stop),
            ("!fortsetzen", Cmd::Resume),
            ("!lautstärke 50", Cmd::Volume),
            ("!hilfe", Cmd::Help),
            ("!reproducir x", Cmd::Play),
            ("!saltar", Cmd::Skip),
            ("!parar", Cmd::Stop),
            ("!pausa", Cmd::Pause),
            ("!reanudar", Cmd::Resume),
            ("!cola", Cmd::Queue),
            ("!volumen 50", Cmd::Volume),
            ("!ayuda", Cmd::Help),
        ] {
            assert_eq!(
                commands.parse(input).map(|(c, _)| c),
                Some(expected),
                "{input}"
            );
        }
    }

    /// One name, one command: a later language must never take over a name.
    #[test]
    fn no_name_is_used_twice() {
        let mut seen = std::collections::HashMap::new();
        for (command, names) in ALIASES {
            for name in *names {
                let lower = name.to_lowercase();
                if let Some(other) = seen.insert(lower.clone(), *command) {
                    panic!("{lower} is both {other:?} and {command:?}");
                }
            }
        }
    }

    /// Every command a text tells people to type (`` `!çal …` ``) exists,
    /// in every language and with another prefix.
    #[test]
    fn every_command_in_a_text_exists() {
        for prefix in ["!", "?"] {
            let commands = commands(prefix);
            for lang in Locale::ALL {
                for (field, template) in strings(lang).fields() {
                    let text = fill(template, &[("p", &prefix)]);
                    for quoted in text.split('`').skip(1).step_by(2) {
                        if quoted.starts_with(prefix) {
                            assert!(
                                commands.parse(quoted).is_some(),
                                "{lang:?}.{field}: {quoted} is not a command"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn controls_need_the_voice_channel() {
        for command in [
            Cmd::Skip,
            Cmd::Stop,
            Cmd::Pause,
            Cmd::Resume,
            Cmd::Shuffle,
            Cmd::Leave,
            Cmd::Remove,
            Cmd::Clear,
        ] {
            assert!(command.controls_playback(false), "{command:?}");
        }
        assert!(Cmd::Volume.controls_playback(true));
        assert!(!Cmd::Volume.controls_playback(false), "reading the volume");
        assert!(Cmd::Loop.controls_playback(true));
        for command in [Cmd::Play, Cmd::Queue, Cmd::NowPlaying, Cmd::Join, Cmd::Help] {
            assert!(!command.controls_playback(true), "{command:?}");
        }
    }

    #[test]
    fn volumes() {
        assert_eq!(parse_volume("150"), Some(150));
        assert_eq!(parse_volume("%80"), Some(80));
        assert_eq!(parse_volume("80%"), Some(80));
        assert_eq!(parse_volume("çok"), None);
    }
}
