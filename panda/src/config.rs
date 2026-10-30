//! Configuration: environment variables, optionally from a `KEY=VALUE`
//! file (`--config <file>` or `PANDA_CONFIG`). Variables set in the
//! environment win over the file, so a systemd `EnvironmentFile` and a
//! hand-written file behave the same.
//!
//! | Key | Default | Meaning |
//! |---|---|---|
//! | `ERA_URL` | (required) | Evre's address, e.g. `https://evre.chat` |
//! | `ERA_BOT_TOKEN` | (required) | The bot's `erabot_…` token (Server settings → Bots) |
//! | `PANDA_DATA_DIR` | `./panda-data` | Settings per guild, yt-dlp's work folder |
//! | `PANDA_PREFIX` | `!` | Command prefix |
//! | `PANDA_LANG` | `tr` | Language of the replies and the panel: `tr`, `en`, `fr`, `de`, `es` (commands work in all of them) |
//! | `PANDA_YTDLP` | `yt-dlp` | yt-dlp program |
//! | `PANDA_FFMPEG` | `ffmpeg` | ffmpeg program |
//! | `PANDA_YTDLP_COOKIES` | – | Cookies file for YouTube's "confirm you're not a bot" |
//! | `PANDA_YTDLP_AUTO_UPDATE` | `false` | Run `yt-dlp -U` at start and daily |
//! | `PANDA_LOUDNORM` | `true` | EBU R128 loudness normalization (ffmpeg `loudnorm`) |
//! | `PANDA_VOLUME` | `100` | Starting volume (0–200 %) |
//! | `PANDA_BITRATE_KBPS` | plan limit | Music bitrate, capped by the guild's plan |
//! | `PANDA_ALONE_MINUTES` | `2` | Leave after this long without listeners |
//! | `PANDA_IDLE_MINUTES` | `5` | Leave after this long with nothing to play |
//! | `PANDA_MAX_QUEUE` | `200` | Songs in the queue at most |
//! | `PANDA_ALLOW_LOCAL_URLS` | `false` | Play links to this machine or the LAN (tests only) |

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::lang::Locale;

/// The variable holding the bot token. Child processes (yt-dlp, ffmpeg)
/// are started without it.
pub const TOKEN_VAR: &str = "ERA_BOT_TOKEN";

/// Panda's settings.
#[derive(Clone)]
pub struct Config {
    /// Era's base URL.
    pub era_url: String,
    /// The bot token (never logged).
    pub token: String,
    /// Where Panda keeps its files.
    pub data_dir: PathBuf,
    /// Command prefix.
    pub prefix: String,
    /// The language Panda writes in.
    pub lang: Locale,
    /// yt-dlp program.
    pub ytdlp: PathBuf,
    /// ffmpeg program.
    pub ffmpeg: PathBuf,
    /// Cookies file passed to yt-dlp for YouTube.
    pub cookies: Option<PathBuf>,
    /// Keep yt-dlp current with `yt-dlp -U`.
    pub ytdlp_auto_update: bool,
    /// Loudness normalization on.
    pub loudnorm: bool,
    /// Starting volume in percent.
    pub default_volume: u32,
    /// Wanted music bitrate.
    pub bitrate_kbps: Option<u32>,
    /// Leave after this long without listeners.
    pub alone_timeout: Duration,
    /// Leave after this long with nothing to play.
    pub idle_timeout: Duration,
    /// Queue length limit.
    pub max_queue: usize,
    /// Allow links to loopback and private addresses.
    pub allow_local_urls: bool,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("era_url", &self.era_url)
            .field("token", &"<redacted>")
            .field("data_dir", &self.data_dir)
            .field("prefix", &self.prefix)
            .field("lang", &self.lang)
            .field("ytdlp", &self.ytdlp)
            .field("ffmpeg", &self.ffmpeg)
            .field("loudnorm", &self.loudnorm)
            .field("bitrate_kbps", &self.bitrate_kbps)
            .finish_non_exhaustive()
    }
}

/// Parses `KEY=VALUE` lines: `#` comments, blank lines, optional `export `,
/// optional single or double quotes around the value.
#[must_use]
pub fn parse_env_file(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        values.insert(key.trim().to_owned(), value.to_owned());
    }
    values
}

fn flag(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "evet" | "açık" => Ok(true),
        "0" | "false" | "no" | "off" | "hayır" | "kapalı" => Ok(false),
        other => Err(format!("not a yes/no value: {other}")),
    }
}

fn minutes(value: &str) -> Result<Duration, String> {
    let minutes: f64 = value
        .trim()
        .parse()
        .map_err(|_| format!("not a number of minutes: {value}"))?;
    if !(0.0..=24.0 * 60.0).contains(&minutes) {
        return Err(format!("minutes out of range: {value}"));
    }
    Ok(Duration::from_secs_f64(minutes * 60.0))
}

impl Config {
    /// Reads the settings from `lookup` (the environment, then the file).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let get = |key: &str| lookup(key).filter(|value| !value.trim().is_empty());
        let era_url =
            get("ERA_URL").ok_or("ERA_URL is not set (Evre's address, e.g. https://evre.chat)")?;
        let token = get(TOKEN_VAR)
            .ok_or("ERA_BOT_TOKEN is not set (the bot's token from Server settings → Bots)")?
            .trim()
            .to_owned();
        let number = |key: &str, default: u64| -> Result<u64, String> {
            get(key).map_or(Ok(default), |value| {
                value
                    .trim()
                    .parse()
                    .map_err(|_| format!("{key} is not a number: {value}"))
            })
        };
        let volume = number("PANDA_VOLUME", 100)?;
        if volume > 200 {
            return Err("PANDA_VOLUME must be 0 to 200".into());
        }
        let bitrate = get("PANDA_BITRATE_KBPS")
            .map(|value| {
                value
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| format!("PANDA_BITRATE_KBPS is not a number: {value}"))
            })
            .transpose()?;
        Ok(Self {
            era_url,
            token,
            data_dir: get("PANDA_DATA_DIR")
                .map_or_else(|| PathBuf::from("panda-data"), PathBuf::from),
            prefix: get("PANDA_PREFIX").unwrap_or_else(|| "!".into()),
            lang: get("PANDA_LANG")
                .map(|value| crate::lang::parse(&value))
                .transpose()
                .map_err(|e| format!("PANDA_LANG: {e}"))?
                .unwrap_or(Locale::Tr),
            ytdlp: get("PANDA_YTDLP").map_or_else(|| PathBuf::from("yt-dlp"), PathBuf::from),
            ffmpeg: get("PANDA_FFMPEG").map_or_else(|| PathBuf::from("ffmpeg"), PathBuf::from),
            cookies: get("PANDA_YTDLP_COOKIES").map(PathBuf::from),
            ytdlp_auto_update: get("PANDA_YTDLP_AUTO_UPDATE")
                .map(|v| flag(&v))
                .transpose()
                .map_err(|e| format!("PANDA_YTDLP_AUTO_UPDATE: {e}"))?
                .unwrap_or(false),
            loudnorm: get("PANDA_LOUDNORM")
                .map(|v| flag(&v))
                .transpose()
                .map_err(|e| format!("PANDA_LOUDNORM: {e}"))?
                .unwrap_or(true),
            default_volume: u32::try_from(volume).unwrap_or(100),
            bitrate_kbps: bitrate,
            alone_timeout: get("PANDA_ALONE_MINUTES")
                .map(|v| minutes(&v))
                .transpose()
                .map_err(|e| format!("PANDA_ALONE_MINUTES: {e}"))?
                .unwrap_or(Duration::from_secs(120)),
            idle_timeout: get("PANDA_IDLE_MINUTES")
                .map(|v| minutes(&v))
                .transpose()
                .map_err(|e| format!("PANDA_IDLE_MINUTES: {e}"))?
                .unwrap_or(Duration::from_secs(300)),
            max_queue: usize::try_from(number("PANDA_MAX_QUEUE", 200)?.max(1)).unwrap_or(200),
            allow_local_urls: get("PANDA_ALLOW_LOCAL_URLS")
                .map(|v| flag(&v))
                .transpose()
                .map_err(|e| format!("PANDA_ALLOW_LOCAL_URLS: {e}"))?
                .unwrap_or(false),
        })
    }

    /// The environment, over the optional file.
    pub fn load(file: Option<&Path>) -> Result<Self, String> {
        let file_values = match file {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                parse_env_file(&text)
            }
            None => HashMap::new(),
        };
        Self::from_lookup(|key| {
            std::env::var(key)
                .ok()
                .or_else(|| file_values.get(key).cloned())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(pairs: &[(&str, &str)]) -> Result<Config, String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_lookup(|key| map.get(key).cloned())
    }

    #[test]
    fn env_files() {
        let values = parse_env_file(
            "# Panda\nERA_URL=https://era.example\nexport ERA_BOT_TOKEN=\"erabot_x\"\n\nPANDA_PREFIX='?'\nnot a line\n",
        );
        assert_eq!(values["ERA_URL"], "https://era.example");
        assert_eq!(values["ERA_BOT_TOKEN"], "erabot_x");
        assert_eq!(values["PANDA_PREFIX"], "?");
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn defaults_and_required_keys() {
        assert!(config(&[]).unwrap_err().contains("ERA_URL"));
        assert!(
            config(&[("ERA_URL", "https://era.example")])
                .unwrap_err()
                .contains("ERA_BOT_TOKEN")
        );
        let c = config(&[
            ("ERA_URL", "https://era.example"),
            ("ERA_BOT_TOKEN", " erabot_x "),
        ])
        .unwrap();
        assert_eq!(c.token, "erabot_x");
        assert_eq!(c.prefix, "!");
        assert_eq!(c.lang, Locale::Tr);
        assert!(c.loudnorm);
        assert_eq!(c.default_volume, 100);
        assert_eq!(c.alone_timeout, Duration::from_secs(120));
        assert!(!c.allow_local_urls);
        // The token never shows in debug output.
        assert!(!format!("{c:?}").contains("erabot_x"));
    }

    #[test]
    fn values_are_checked() {
        let base = [
            ("ERA_URL", "https://era.example"),
            ("ERA_BOT_TOKEN", "erabot_x"),
        ];
        let with = |key: &'static str, value: &'static str| {
            let mut pairs = base.to_vec();
            pairs.push((key, value));
            config(&pairs)
        };
        assert!(with("PANDA_VOLUME", "250").is_err());
        assert!(with("PANDA_LOUDNORM", "maybe").is_err());
        assert!(!with("PANDA_LOUDNORM", "kapalı").unwrap().loudnorm);
        assert_eq!(
            with("PANDA_IDLE_MINUTES", "0.5").unwrap().idle_timeout,
            Duration::from_secs(30)
        );
        assert_eq!(
            with("PANDA_BITRATE_KBPS", "192").unwrap().bitrate_kbps,
            Some(192)
        );
        assert_eq!(with("PANDA_LANG", "fr").unwrap().lang, Locale::Fr);
        assert_eq!(with("PANDA_LANG", "en-GB").unwrap().lang, Locale::En);
        assert!(
            with("PANDA_LANG", "xx")
                .unwrap_err()
                .starts_with("PANDA_LANG:")
        );
        assert_eq!(
            with("PANDA_LANG", " ").unwrap().lang,
            Locale::Tr,
            "blank means the default"
        );
        assert_eq!(
            with("PANDA_BITRATE_KBPS", "192").unwrap().bitrate_kbps,
            Some(192)
        );
    }
}
