//! `panda [--config <file>]`: runs Panda v2. Settings come from the
//! environment and the optional `KEY=VALUE` file (see `panda::config`).

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "Panda v2, the music bot for Evre

Usage: panda [--config <file>] [--check]

  --check   test ffmpeg, yt-dlp and the voice stack here, then exit

Settings come from environment variables, or KEY=VALUE lines in the file
given with --config (or PANDA_CONFIG). Required:
  ERA_URL        Evre's address: https://evre.chat
  ERA_BOT_TOKEN  the bot's token (Server settings -> Bots)
Optional: PANDA_LANG (tr, en, fr, de, es), PANDA_DATA_DIR, PANDA_PREFIX,
PANDA_YTDLP, PANDA_FFMPEG, PANDA_YTDLP_COOKIES, PANDA_YTDLP_AUTO_UPDATE,
PANDA_LOUDNORM, PANDA_VOLUME, PANDA_BITRATE_KBPS, PANDA_ALONE_MINUTES,
PANDA_IDLE_MINUTES, PANDA_MAX_QUEUE.
Needs yt-dlp and ffmpeg (with ffprobe).";

fn main() -> ExitCode {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,livekit=warn,libwebrtc=warn"),
    )
    .init();
    let mut file = std::env::var_os("PANDA_CONFIG").map(PathBuf::from);
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--config" | "-c" => match args.next() {
                Some(path) => file = Some(PathBuf::from(path)),
                None => {
                    eprintln!("--config needs a file\n\n{USAGE}");
                    return ExitCode::from(2);
                }
            },
            "--version" | "-V" => {
                println!("panda {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument {other}\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    if check {
        return check_here(file.as_deref());
    }
    let config = match panda::Config::load(file.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("panda: {error}");
            return ExitCode::from(2);
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("panda: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(panda::run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// `--check`: the programs from the configuration (or the defaults).
fn check_here(file: Option<&std::path::Path>) -> ExitCode {
    let values = file
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| panda::config::parse_env_file(&text))
        .unwrap_or_default();
    let program = |key: &str, default: &str| {
        PathBuf::from(
            std::env::var(key)
                .ok()
                .or_else(|| values.get(key).cloned())
                .unwrap_or_else(|| default.to_owned()),
        )
    };
    let (ffmpeg, ytdlp) = (
        program("PANDA_FFMPEG", "ffmpeg"),
        program("PANDA_YTDLP", "yt-dlp"),
    );
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("panda: {error}");
            return ExitCode::FAILURE;
        }
    };
    if runtime.block_on(panda::check::run(&ffmpeg, &ytdlp)) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
