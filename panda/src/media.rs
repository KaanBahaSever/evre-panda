//! Finding songs (yt-dlp) and decoding them (ffmpeg) to 48 kHz stereo
//! `s16le` PCM:
//!
//! ```text
//! yt-dlp -o - <url> ─pipe─▶ ffmpeg -i pipe:0 -af loudnorm … -f s16le -ar 48000 -ac 2 pipe:1 ─▶ Panda
//! ffmpeg -i <audio link> …                                                                 ─▶ Panda
//! ```
//!
//! yt-dlp handles YouTube's chunked downloads and throttling, ffmpeg only
//! decodes (as in Panda v1). A [`Decoder`] starts both processes and reads
//! up to [`BUFFER`] of PCM ahead, so starting it before the previous song
//! ends gives gapless playback.

use std::net::IpAddr;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::queue::Source;

/// PCM per chunk: 20 ms of interleaved stereo.
pub const CHUNK_SAMPLES: usize = 1920;
/// How much PCM a decoder reads ahead: 10 s.
pub const BUFFER: Duration = Duration::from_secs(10);
const BUFFER_CHUNKS: usize = 500;
/// yt-dlp metadata lookups give up after this long.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(60);
/// Most yt-dlp JSON read for one lookup (a video's metadata is well under
/// a megabyte).
const MAX_LOOKUP_JSON: u64 = 16 * 1024 * 1024;
/// Longest title kept for a song (chat shows at most 120 characters).
const MAX_TITLE_CHARS: usize = 300;
/// Loudness target: -14 LUFS like YouTube and Spotify (Panda v1 too),
/// true peak -1.5 dBTP.
const LOUDNORM: &str = "loudnorm=I=-14:TP=-1.5:LRA=11";
/// Protocols ffmpeg (and ffprobe) may use for a direct link: an `http(s)`
/// fetch over TLS and the HLS crypto layer, never `file`, `concat`,
/// `subfile` or `data`.
const DIRECT_PROTOCOLS: &str = "http,https,tls,tcp,crypto";
/// Protocols ffmpeg may use while decoding yt-dlp's piped output: the pipe
/// it reads and the crypto layer only, so a crafted stream cannot make it
/// open local files or fetch URLs of its own.
const PIPE_PROTOCOLS: [&str; 2] = ["-protocol_whitelist", "pipe,crypto"];
/// Audio file endings ffmpeg reads directly.
const AUDIO_EXTENSIONS: [&str; 11] = [
    ".mp3", ".ogg", ".oga", ".opus", ".wav", ".flac", ".m4a", ".aac", ".mka", ".weba", ".m3u8",
];

/// What someone typed after `!çal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// A link.
    Url(String),
    /// Words to search for.
    Search(String),
}

/// Tells links from search words. Links without a scheme (`youtu.be/…`,
/// `www.…`) get `https://`; `<…>` around a link (Markdown) is removed.
#[must_use]
pub fn classify(input: &str) -> Query {
    let text = input.trim();
    let text = text
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .unwrap_or(text);
    let lower = text.to_ascii_lowercase();
    if !text.contains(char::is_whitespace) {
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Query::Url(text.to_owned());
        }
        for bare in [
            "www.",
            "youtu.be/",
            "youtube.com/",
            "m.youtube.com/",
            "music.youtube.com/",
            "soundcloud.com/",
            "on.soundcloud.com/",
        ] {
            if lower.starts_with(bare) {
                return Query::Url(format!("https://{text}"));
            }
        }
    }
    Query::Search(text.to_owned())
}

/// How a link is read: audio files and HLS playlists by ffmpeg, everything
/// else through yt-dlp.
#[must_use]
pub fn source_for(url: &str) -> Source {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    if AUDIO_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
        Source::Direct(url.to_owned())
    } else {
        Source::YtDlp(url.to_owned())
    }
}

/// The host and port of an `http(s)` link.
fn host_port(url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let default_port = match scheme.to_ascii_lowercase().as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    if let Some(v6) = authority.strip_prefix('[') {
        let (host, after) = v6.split_once(']')?;
        let port = after
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(default_port);
        return Some((host.to_owned(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => {
            Some((host.to_owned(), port.parse().ok()?))
        }
        _ => Some((authority.to_owned(), default_port)),
    }
    .filter(|(host, _)| !host.is_empty())
}

/// Loopback, private, link-local, unspecified or otherwise not public.
#[must_use]
pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || a == 0 // "this network"; 0.0.0.0 reaches this host
                || a == 100 && (b & 0xc0) == 64 // CGNAT
                || a == 192 && b == 0 && c == 0 // IETF protocol assignments
                || a == 198 && (b & 0xfe) == 18 // benchmarking
                || a >= 240 // reserved
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            // An IPv4 address in two segments (`a.b` and `c.d`).
            let v4_in = |high: u16, low: u16| {
                let [a, b] = high.to_be_bytes();
                let [c, d] = low.to_be_bytes();
                is_local(IpAddr::V4(std::net::Ipv4Addr::new(a, b, c, d)))
            };
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00 // unique local
                || (segments[0] & 0xffc0) == 0xfe80 // link local
                || v6.to_ipv4_mapped().is_some_and(|v4| is_local(IpAddr::V4(v4)))
                // IPv4-compatible (`::a.b.c.d`) and NAT64 (`64:ff9b::a.b.c.d`).
                || (segments[..6] == [0; 6] || segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0])
                    && v4_in(segments[6], segments[7])
                // 6to4 (`2002:a.b.c.d::`).
                || segments[0] == 0x2002 && v4_in(segments[1], segments[2])
        }
    }
}

/// Parses a host that is an IP literal, including the obfuscated IPv4 forms
/// a permissive C resolver (`inet_aton`) accepts: a bare 32-bit number
/// (`2130706433`), hex (`0x7f000001`), octal (`017700000001`) and the
/// 1-to-3-part shorthands (`127.1`, `127.0.0x1`). These let a link smuggle
/// an internal address past a naive dotted-quad check, so they are resolved
/// here rather than handed to the system resolver.
#[must_use]
fn parse_ip_literal(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    parse_ipv4_literal(host).map(IpAddr::V4)
}

/// The `inet_aton` reading of an IPv4 host that [`str::parse`] rejects: 1 to
/// 4 parts, each decimal, hexadecimal (`0x…`) or octal (leading `0`). Only
/// digits of the part's radix are accepted, so real hostnames (which carry
/// letters or too many parts) return `None`.
#[must_use]
fn parse_ipv4_literal(host: &str) -> Option<std::net::Ipv4Addr> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let n = parts.len();
    let mut values = Vec::with_capacity(n);
    for part in &parts {
        values.push(parse_ip_part(part)?);
    }
    // The last part fills the remaining low bytes; each earlier part is one
    // byte in the high bytes. So `a.b.c.d`, `a.b.c` (c is 16-bit), `a.b` (b
    // 24-bit), `a` (32-bit). u64 arithmetic avoids a 32-bit shift overflow.
    let last = *values.last()?;
    let max_last: u64 = (1u64 << (8 * (5 - n))) - 1;
    if last > max_last {
        return None;
    }
    let mut addr: u64 = 0;
    for (i, value) in values[..n - 1].iter().enumerate() {
        if *value > 0xff {
            return None;
        }
        addr |= *value << (8 * (3 - i));
    }
    addr |= last;
    Some(std::net::Ipv4Addr::from(u32::try_from(addr).ok()?))
}

/// One part of an `inet_aton` address, as an unsigned number up to 2^32.
#[must_use]
fn parse_ip_part(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }
    let (radix, digits) = if let Some(hex) = part
        .strip_prefix("0x")
        .or_else(|| part.strip_prefix("0X"))
    {
        (16, hex)
    } else if let Some(octal) = part.strip_prefix('0').filter(|rest| !rest.is_empty()) {
        (8, octal)
    } else {
        (10, part)
    };
    if digits.is_empty() {
        // A bare "0".
        return (part == "0").then_some(0);
    }
    let value = u64::from_str_radix(digits, radix).ok()?;
    (value <= u64::from(u32::MAX)).then_some(value)
}

/// Refuses links to this machine or the local network (people in chat
/// must not make Panda fetch internal pages), unless allowed.
pub async fn check_link(url: &str, allow_local: bool) -> Result<(), String> {
    let Some((host, port)) = host_port(url) else {
        return Err("unsupported".into());
    };
    if allow_local {
        return Ok(());
    }
    if host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".localhost") {
        return Err("local".into());
    }
    // A numeric host is an IP literal (standard or obfuscated), not a name:
    // decide it here so no system resolver turns an internal address into a
    // "public" lookup, and so every obfuscated form is caught.
    if let Some(ip) = parse_ip_literal(&host) {
        return if is_local(ip) {
            Err("local".into())
        } else {
            Ok(())
        };
    }
    let addresses: Vec<IpAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| "not-found".to_owned())?
        .map(|addr| addr.ip())
        .collect();
    if addresses.is_empty() || addresses.into_iter().any(is_local) {
        return Err("local".into());
    }
    Ok(())
}

/// What a lookup found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Title.
    pub title: String,
    /// How to play it.
    pub source: Source,
    /// The page to show.
    pub link: Option<String>,
    /// Length, if known.
    pub duration: Option<Duration>,
    /// A live stream.
    pub live: bool,
}

/// Maps yt-dlp's error output to a short reason (see
/// [`crate::text::media_error`]).
#[must_use]
pub fn friendly_error(stderr: &str) -> String {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("confirm you") || lower.contains("not a bot") {
        return "bot-check".into();
    }
    if lower.contains("confirm your age") || lower.contains("age-restricted") {
        return "age".into();
    }
    if lower.contains("private video")
        || lower.contains("video unavailable")
        || lower.contains("not available")
    {
        return "unavailable".into();
    }
    if lower.contains("does not exist")
        || lower.contains("http error 404")
        || lower.contains("404 not found")
    {
        return "not-found".into();
    }
    if lower.contains("unsupported url") {
        return "unsupported".into();
    }
    // yt-dlp's last ERROR line, else ffmpeg's last line.
    let lines = || {
        stderr
            .lines()
            .rev()
            .map(str::trim)
            .filter(|l| !l.is_empty())
    };
    lines()
        .find(|line| line.contains("ERROR"))
        .or_else(|| lines().next())
        .map(|line| {
            line.replace("ERROR: ", "")
                .trim()
                .chars()
                .take(200)
                .collect()
        })
        .unwrap_or_else(|| "failed".into())
}

/// Whether another try may work: YouTube refusing or breaking off often
/// passes, a missing, private or unsupported song never does.
#[must_use]
pub fn retryable(reason: &str) -> bool {
    !matches!(
        reason,
        "not-found"
            | "unavailable"
            | "age"
            | "unsupported"
            | "local"
            | "missing-ytdlp"
            | "missing-ffmpeg"
    )
}

fn seconds(value: &Value) -> Option<Duration> {
    value
        .as_f64()
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(Duration::from_secs_f64)
}

/// A song from yt-dlp's JSON (`-J`): a video, or the first entry of a
/// search or playlist.
#[must_use]
pub fn parse_ytdlp_json(json: &Value) -> Option<Found> {
    let item = match json.get("entries").and_then(Value::as_array) {
        Some(entries) => entries.iter().find(|entry| !entry.is_null())?,
        None => json,
    };
    let link = ["webpage_url", "original_url", "url"]
        .iter()
        .filter_map(|key| item.get(*key).and_then(Value::as_str))
        .find(|url| url.starts_with("http"))
        .map(str::to_owned)
        .or_else(|| {
            // Flat search results of YouTube carry only the ID.
            item.get("id")
                .and_then(Value::as_str)
                .filter(|_| {
                    item.get("ie_key").and_then(Value::as_str) == Some("Youtube")
                        || json.get("extractor").and_then(Value::as_str) == Some("youtube:search")
                })
                .map(|id| format!("https://www.youtube.com/watch?v={id}"))
        })?;
    let live = item.get("is_live").and_then(Value::as_bool) == Some(true)
        || item.get("live_status").and_then(Value::as_str) == Some("is_live");
    let title = item
        .get("title")
        .and_then(Value::as_str)
        // No title: the chat shows "untitled" in Panda's language.
        .unwrap_or("")
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    Some(Found {
        title,
        source: Source::YtDlp(link.clone()),
        link: Some(link),
        duration: if live {
            None
        } else {
            item.get("duration").and_then(seconds)
        },
        live,
    })
}

/// A command for yt-dlp, ffmpeg or ffprobe. They handle what people post,
/// so they never get Panda's bot token in their environment (third
/// security review).
fn tool(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env_remove(crate::config::TOKEN_VAR);
    command
}

fn ytdlp_base(config: &Config, youtube: bool) -> Command {
    let mut command = tool(&config.ytdlp);
    command.args(["--ignore-config", "--no-warnings", "--no-progress"]);
    if youtube && let Some(cookies) = &config.cookies {
        command.arg("--cookies").arg(cookies);
    }
    let work = config.data_dir.join("ytdlp-work");
    if std::fs::create_dir_all(&work).is_ok() {
        // yt-dlp leaves HLS pieces in its working directory.
        command.current_dir(work);
    }
    command
}

fn is_youtube(url: &str) -> bool {
    let host = host_port(url).map(|(host, _)| host.to_ascii_lowercase());
    host.is_some_and(|host| {
        host == "youtu.be" || host == "youtube.com" || host.ends_with(".youtube.com")
    })
}

fn missing(program: &Path, error: &std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        let name = program
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().to_lowercase());
        if name.contains("ffmpeg") || name.contains("ffprobe") {
            "missing-ffmpeg".into()
        } else {
            "missing-ytdlp".into()
        }
    } else {
        format!("could not start {}: {error}", program.display())
    }
}

/// Runs a command, returning stdout or a short reason. At most
/// [`MAX_LOOKUP_JSON`] of output is read: a page whose metadata makes yt-dlp
/// print hundreds of megabytes is refused instead of filling the host's
/// memory (third security review).
async fn run_json(mut command: Command, program: &Path) -> Result<Value, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| missing(program, &error))?;
    let stdout = child.stdout.take().ok_or("yt-dlp has no output")?;
    let tail = Tail::default();
    let stderr = child.stderr.take().map(|stderr| tail.spawn_reader(stderr));
    let run = async {
        let mut out = Vec::new();
        stdout
            .take(MAX_LOOKUP_JSON + 1)
            .read_to_end(&mut out)
            .await
            .map_err(|error| error.to_string())?;
        if out.len() as u64 > MAX_LOOKUP_JSON {
            let _ = child.kill().await;
            return Err("unsupported".to_owned());
        }
        let status = child.wait().await.map_err(|error| error.to_string())?;
        if let Some(stderr) = stderr {
            let _ = tokio::time::timeout(Duration::from_secs(2), stderr).await;
        }
        Ok((status, out))
    };
    let (status, out) = tokio::time::timeout(LOOKUP_TIMEOUT, run)
        .await
        .map_err(|_| "timeout".to_owned())??;
    if !status.success() || out.is_empty() {
        return Err(friendly_error(&tail.text()));
    }
    serde_json::from_slice(&out).map_err(|error| format!("yt-dlp: {error}"))
}

/// The title of a link from its last path segment (`my%20song.mp3` →
/// `my song`).
fn title_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(path);
    let decoded = percent_decode(last);
    match decoded.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 5 => stem.to_owned(),
        _ => decoded,
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && text.is_char_boundary(i + 1)
            && text.is_char_boundary(i + 3)
            && let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The length of a file or stream by ffprobe (next to ffmpeg), if it
/// tells.
async fn probe_duration(config: &Config, url: &str) -> Option<Duration> {
    let ffprobe = {
        let name = config
            .ffmpeg
            .file_name()?
            .to_string_lossy()
            .replace("ffmpeg", "ffprobe");
        config.ffmpeg.with_file_name(name)
    };
    let mut command = tool(&ffprobe);
    command.args([
        "-v",
        "error",
        "-protocol_whitelist",
        DIRECT_PROTOCOLS,
        "-show_entries",
        "format=duration",
        "-of",
        "csv=p=0",
        url,
    ]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(15), command.output())
        .await
        .ok()?
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(Duration::from_secs_f64)
}

/// Finds a song: a search on YouTube, or the song behind a link.
pub async fn lookup(config: &Config, query: &Query) -> Result<Found, String> {
    match query {
        Query::Search(words) => {
            let mut command = ytdlp_base(config, true);
            command
                .args(["-J", "--flat-playlist"])
                .arg(format!("ytsearch1:{words}"));
            let json = run_json(command, &config.ytdlp).await?;
            let found = parse_ytdlp_json(&json).ok_or_else(|| "not-found".to_owned())?;
            check_found(config, found).await
        }
        Query::Url(url) => {
            check_link(url, config.allow_local_urls).await?;
            match source_for(url) {
                Source::Direct(url) => Ok(Found {
                    title: title_from_url(&url),
                    duration: probe_duration(config, &url).await,
                    live: url.to_ascii_lowercase().contains(".m3u8"),
                    link: Some(url.clone()),
                    source: Source::Direct(url),
                }),
                Source::YtDlp(url) => {
                    let mut command = ytdlp_base(config, is_youtube(&url));
                    command
                        .args([
                            "-J",
                            "--no-playlist",
                            "--playlist-items",
                            "1",
                            "--skip-download",
                        ])
                        .arg(&url);
                    let json = run_json(command, &config.ytdlp).await?;
                    let found = parse_ytdlp_json(&json).ok_or_else(|| "not-found".to_owned())?;
                    check_found(config, found).await
                }
            }
        }
    }
}

/// The link yt-dlp resolved is played later without Panda's own lookup
/// (a playlist entry or a page's embedded media can point anywhere), so it
/// passes the same check as a typed one (third security review).
async fn check_found(config: &Config, found: Found) -> Result<Found, String> {
    let (Source::YtDlp(url) | Source::Direct(url)) = &found.source;
    check_link(url, config.allow_local_urls).await?;
    Ok(found)
}

/// What a decoder delivers.
#[derive(Debug)]
pub enum Pcm {
    /// 20 ms (or less at the end) of interleaved stereo 48 kHz PCM.
    Chunk(Vec<i16>),
    /// The song ended: `Ok` when the processes finished cleanly.
    End(Result<(), String>),
}

/// The last bytes a process wrote to stderr.
#[derive(Clone, Default)]
struct Tail(Arc<Mutex<String>>);

impl Tail {
    fn spawn_reader(
        &self,
        stream: impl AsyncRead + Unpin + Send + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let tail = self.clone();
        tokio::spawn(async move {
            let mut stream = stream;
            let mut buffer = [0_u8; 1024];
            while let Ok(n) = stream.read(&mut buffer).await {
                if n == 0 {
                    break;
                }
                let mut text = tail.0.lock().unwrap_or_else(|p| p.into_inner());
                text.push_str(&String::from_utf8_lossy(&buffer[..n]));
                if text.len() > 4096 {
                    let cut = text.len() - 4096;
                    let cut = (cut..text.len())
                        .find(|i| text.is_char_boundary(*i))
                        .unwrap_or(0);
                    text.drain(..cut);
                }
            }
        })
    }

    fn text(&self) -> String {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

/// A song being decoded. Dropping it stops the processes.
pub struct Decoder {
    rx: mpsc::Receiver<Pcm>,
    _children: Vec<Arc<tokio::sync::Mutex<Child>>>,
    reader: tokio::task::JoinHandle<()>,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl std::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decoder").finish_non_exhaustive()
    }
}

impl Decoder {
    /// Starts decoding `source`, from `start` into the song (a retry after
    /// a cut-off).
    pub fn start(config: &Config, source: &Source, start: Duration) -> Result<Self, String> {
        let seek = (!start.is_zero()).then(|| format!("{:.3}", start.as_secs_f64()));
        let mut ffmpeg = tool(&config.ffmpeg);
        ffmpeg.args(["-hide_banner", "-loglevel", "error"]);
        let mut children = Vec::new();
        let mut tails = Vec::new();
        let ytdlp_tail = Tail::default();
        match source {
            Source::YtDlp(url) => {
                let mut ytdlp = ytdlp_base(config, is_youtube(url));
                ytdlp
                    .args([
                        "--quiet",
                        "--no-playlist",
                        "-f",
                        "bestaudio/best",
                        "-o",
                        "-",
                    ])
                    .arg(url)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true);
                let mut child = ytdlp
                    .spawn()
                    .map_err(|error| missing(&config.ytdlp, &error))?;
                let stdout: Stdio = child
                    .stdout
                    .take()
                    .ok_or("yt-dlp has no output")?
                    .try_into()
                    .map_err(|error: std::io::Error| error.to_string())?;
                if let Some(stderr) = child.stderr.take() {
                    tails.push(ytdlp_tail.spawn_reader(stderr));
                }
                children.push(Arc::new(tokio::sync::Mutex::new(child)));
                // yt-dlp already fetched the media; ffmpeg only decodes what
                // comes down the pipe. Pin the protocols so a crafted stream
                // (a playlist or concat script yt-dlp happened to emit) cannot
                // make ffmpeg open `file:`, `concat:`, `subfile:` or fetch
                // anything of its own.
                ffmpeg.args([PIPE_PROTOCOLS[0], PIPE_PROTOCOLS[1], "-i", "pipe:0"]);
                ffmpeg.stdin(stdout);
                // A pipe cannot seek: decode up to there and drop it.
                if let Some(seek) = &seek {
                    ffmpeg.args(["-ss", seek]);
                }
            }
            Source::Direct(url) => {
                ffmpeg.args([
                    "-nostdin",
                    "-protocol_whitelist",
                    DIRECT_PROTOCOLS,
                    "-reconnect",
                    "1",
                    "-reconnect_streamed",
                    "1",
                    "-reconnect_delay_max",
                    "5",
                ]);
                if let Some(seek) = &seek {
                    ffmpeg.args(["-ss", seek]);
                }
                ffmpeg.arg("-i").arg(url).stdin(Stdio::null());
            }
        }
        ffmpeg.arg("-vn");
        if config.loudnorm {
            ffmpeg.args(["-af", LOUDNORM]);
        }
        ffmpeg
            .args(["-f", "s16le", "-ar", "48000", "-ac", "2", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = ffmpeg
            .spawn()
            .map_err(|error| missing(&config.ffmpeg, &error))?;
        let stdout = child.stdout.take().ok_or("ffmpeg has no output")?;
        let ffmpeg_tail = Tail::default();
        if let Some(stderr) = child.stderr.take() {
            tails.push(ffmpeg_tail.spawn_reader(stderr));
        }
        children.push(Arc::new(tokio::sync::Mutex::new(child)));

        let (tx, rx) = mpsc::channel(BUFFER_CHUNKS);
        let waited = children.clone();
        let reader = tokio::spawn(async move {
            let outcome = read_pcm(stdout, &tx).await;
            // Both processes must have ended, and their last words be read,
            // to judge the song.
            let mut failure = outcome.err();
            for tail in tails {
                let _ = tokio::time::timeout(Duration::from_secs(2), tail).await;
            }
            for (index, child) in waited.iter().enumerate() {
                let status = child.lock().await.wait().await;
                let ok = status.as_ref().is_ok_and(std::process::ExitStatus::success);
                if !ok && failure.is_none() {
                    let tail = if index + 1 == waited.len() {
                        ffmpeg_tail.text()
                    } else {
                        ytdlp_tail.text()
                    };
                    failure = Some(if tail.trim().is_empty() {
                        "failed".into()
                    } else {
                        friendly_error(&tail)
                    });
                }
            }
            let _ = tx.send(Pcm::End(failure.map_or(Ok(()), Err))).await;
        });
        Ok(Self {
            rx,
            _children: children,
            reader,
        })
    }

    /// The next chunk, or the end.
    pub async fn next(&mut self) -> Pcm {
        self.rx
            .recv()
            .await
            .unwrap_or_else(|| Pcm::End(Err("decoder stopped".into())))
    }
}

/// Reads s16le PCM in 20 ms chunks until the end of the stream.
async fn read_pcm(
    mut stdout: impl AsyncRead + Unpin,
    tx: &mpsc::Sender<Pcm>,
) -> Result<(), String> {
    let mut bytes = vec![0_u8; CHUNK_SAMPLES * 2];
    let mut filled = 0;
    loop {
        let n = stdout
            .read(&mut bytes[filled..])
            .await
            .map_err(|error| error.to_string())?;
        if n == 0 {
            // Whole stereo pairs only.
            let usable = filled - filled % 4;
            if usable > 0 {
                let chunk = samples(&bytes[..usable]);
                let _ = tx.send(Pcm::Chunk(chunk)).await;
            }
            return Ok(());
        }
        filled += n;
        if filled == bytes.len() {
            if tx.send(Pcm::Chunk(samples(&bytes))).await.is_err() {
                return Ok(()); // nobody listens any more
            }
            filled = 0;
        }
    }
}

fn samples(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect()
}

/// Scales PCM by `percent` (100 = unchanged) with hard clipping.
pub fn apply_volume(pcm: &mut [i16], percent: u32) {
    if percent == 100 {
        return;
    }
    let gain = percent as f32 / 100.0;
    for sample in pcm {
        #[allow(clippy::cast_possible_truncation)]
        let scaled = (f32::from(*sample) * gain).clamp(-32768.0, 32767.0) as i16;
        *sample = scaled;
    }
}

/// `<program> --version` (or `-version`): the first line, if it runs.
pub async fn version(program: &Path, flag: &str) -> Option<String> {
    let output = tool(program)
        .arg(flag)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::to_owned)
}

/// `yt-dlp -U`: updates a standalone yt-dlp in place.
pub async fn update_ytdlp(config: &Config) {
    let output = tool(&config.ytdlp)
        .arg("-U")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await;
    match output {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout);
            log::info!("yt-dlp -U: {}", text.lines().last().unwrap_or("").trim());
        }
        Err(error) => log::warn!("yt-dlp -U failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use serde_json::json;

    use super::*;

    #[test]
    fn links_and_searches() {
        assert_eq!(
            classify(" https://youtu.be/abc "),
            Query::Url("https://youtu.be/abc".into())
        );
        assert_eq!(
            classify("<https://soundcloud.com/x/y>"),
            Query::Url("https://soundcloud.com/x/y".into())
        );
        assert_eq!(
            classify("youtu.be/abc"),
            Query::Url("https://youtu.be/abc".into())
        );
        assert_eq!(
            classify("barış manço gülpembe"),
            Query::Search("barış manço gülpembe".into())
        );
        assert_eq!(classify("lofi"), Query::Search("lofi".into()));
    }

    #[test]
    fn sources() {
        assert_eq!(
            source_for("https://cdn.example/a/song.MP3?sig=1"),
            Source::Direct("https://cdn.example/a/song.MP3?sig=1".into())
        );
        assert_eq!(
            source_for("https://www.youtube.com/watch?v=x"),
            Source::YtDlp("https://www.youtube.com/watch?v=x".into())
        );
        assert!(is_youtube("https://music.youtube.com/watch?v=x"));
        assert!(is_youtube("https://youtu.be/x"));
        assert!(!is_youtube("https://notyoutube.com/x"));
    }

    #[test]
    fn hosts() {
        assert_eq!(
            host_port("https://a.example/x"),
            Some(("a.example".into(), 443))
        );
        assert_eq!(
            host_port("http://a.example:8080"),
            Some(("a.example".into(), 8080))
        );
        assert_eq!(
            host_port("http://u:p@a.example/"),
            Some(("a.example".into(), 80))
        );
        assert_eq!(host_port("http://[::1]:9/x"), Some(("::1".into(), 9)));
        assert_eq!(host_port("file:///etc/passwd"), None);
        assert_eq!(host_port("https:///x"), None);
    }

    #[test]
    fn local_addresses() {
        for ip in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 22)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            "fd00::1".parse().unwrap(),
            "fe80::1".parse().unwrap(),
            "::ffff:127.0.0.1".parse().unwrap(),
        ] {
            assert!(is_local(ip), "{ip}");
        }
        assert!(!is_local(IpAddr::V4(Ipv4Addr::new(156, 233, 71, 241))));
        assert!(!is_local("2a00:1450::1".parse().unwrap()));
        // Third security review: the rest of "this network", reserved and
        // multicast ranges, and IPv4 inside IPv6 (compatible, NAT64, 6to4).
        for ip in [
            "0.0.0.1",
            "0.255.255.255",
            "192.0.0.8",
            "198.18.0.1",
            "198.19.255.255",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::127.0.0.1",
            "::10.0.0.1",
            "64:ff9b::127.0.0.1",
            "64:ff9b::a9fe:a9fe",
            "2002:7f00:1::",
            "2002:c0a8:101::1",
            "ff02::1",
        ] {
            assert!(is_local(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "8.8.8.8",
            "198.20.0.1",
            "64:ff9b::808:808",
            "2002:808:808::1",
        ] {
            assert!(!is_local(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn obfuscated_ipv4_literals() {
        let v4 = parse_ipv4_literal;
        // Decimal, hex and octal of 127.0.0.1.
        assert_eq!(v4("2130706433"), Some(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(v4("0x7f000001"), Some(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(v4("017700000001"), Some(Ipv4Addr::new(127, 0, 0, 1)));
        // inet_aton shorthands.
        assert_eq!(v4("127.1"), Some(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(v4("127.0.1"), Some(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(v4("0x7f.0.0.1"), Some(Ipv4Addr::new(127, 0, 0, 1)));
        // A public one is still parsed (and judged non-local elsewhere).
        assert_eq!(v4("0x08080808"), Some(Ipv4Addr::new(8, 8, 8, 8)));
        // Real hostnames and out-of-range parts are not IPv4 literals.
        assert_eq!(v4("youtube.com"), None);
        assert_eq!(v4("256.1.1.1"), None);
        assert_eq!(v4("1.2.3.4.5"), None);
        assert_eq!(v4("0x1g"), None);
        assert_eq!(v4("08"), None); // 8 is not an octal digit
    }

    #[tokio::test]
    async fn local_links_are_refused() {
        for url in [
            "http://127.0.0.1:8080/a.mp3",
            "http://localhost/a.mp3",
            "http://[::1]/a.mp3",
            // Obfuscated forms of 127.0.0.1 and an internal service.
            "http://2130706433/a.mp3",
            "http://0x7f000001:2019/config",
            "http://017700000001/a.mp3",
            "http://127.1/a.mp3",
            "http://[::ffff:7f00:0001]/a.mp3",
            // CGNAT as a bare integer (100.64.0.1).
            "http://1681915905/a.mp3",
            // "This network" in shorthand (0.0.0.1), and NAT64 of loopback.
            "http://0.1/a.mp3",
            "http://[64:ff9b::7f00:1]/a.mp3",
        ] {
            assert_eq!(check_link(url, false).await, Err("local".into()), "{url}");
        }
        assert_eq!(
            check_link("file:///etc/passwd", true).await,
            Err("unsupported".into())
        );
        assert_eq!(check_link("http://127.0.0.1/a.mp3", true).await, Ok(()));
        // A public IP literal is accepted without a DNS lookup.
        assert_eq!(check_link("http://0x08080808/a.mp3", false).await, Ok(()));
    }

    #[test]
    fn ffmpeg_protocols_never_reach_local_files() {
        for list in [DIRECT_PROTOCOLS, PIPE_PROTOCOLS[1]] {
            let protocols: Vec<&str> = list.split(',').collect();
            for banned in ["file", "concat", "subfile", "data", "unix", "fd"] {
                assert!(!protocols.contains(&banned), "{list} allows {banned}");
            }
        }
        // The pipe path still reads the pipe it is handed.
        assert_eq!(PIPE_PROTOCOLS[0], "-protocol_whitelist");
        assert!(PIPE_PROTOCOLS[1].split(',').any(|p| p == "pipe"));
    }

    #[test]
    fn ytdlp_json() {
        let video = json!({
            "id": "dQw4w9WgXcQ",
            "title": "A video",
            "duration": 212.0,
            "webpage_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "is_live": false,
        });
        let found = parse_ytdlp_json(&video).unwrap();
        assert_eq!(found.title, "A video");
        assert_eq!(found.duration, Some(Duration::from_secs(212)));
        assert!(!found.live);

        // A flat search result carries only the ID.
        let search = json!({
            "extractor": "youtube:search",
            "entries": [{"id": "abc", "title": "Found", "duration": 61, "ie_key": "Youtube", "url": "abc"}],
        });
        let found = parse_ytdlp_json(&search).unwrap();
        assert_eq!(
            found.link.as_deref(),
            Some("https://www.youtube.com/watch?v=abc")
        );
        assert_eq!(
            found.source,
            Source::YtDlp("https://www.youtube.com/watch?v=abc".into())
        );

        let live = json!({"title": "Radyo", "webpage_url": "https://x.example/live", "live_status": "is_live", "duration": 5});
        let found = parse_ytdlp_json(&live).unwrap();
        assert!(found.live);
        assert_eq!(found.duration, None);

        assert_eq!(parse_ytdlp_json(&json!({"entries": []})), None);
    }

    #[test]
    fn errors_are_short() {
        assert_eq!(
            friendly_error("ERROR: [youtube] x: Sign in to confirm you're not a bot"),
            "bot-check"
        );
        assert_eq!(
            friendly_error("ERROR: [youtube] x: Private video"),
            "unavailable"
        );
        assert_eq!(friendly_error("WARNING: a\nERROR: boom\n"), "boom");
        assert_eq!(
            friendly_error("Error opening input files: Server returned 404 Not Found\n"),
            "not-found"
        );
        assert_eq!(friendly_error("something odd\n\n"), "something odd");
        assert_eq!(friendly_error(""), "failed");
    }

    #[test]
    fn titles_from_links() {
        assert_eq!(
            title_from_url("https://cdn.example/m/My%20Song.mp3?x=1"),
            "My Song"
        );
        assert_eq!(title_from_url("https://cdn.example/stream/"), "stream");
    }

    #[test]
    fn volume() {
        let mut pcm = [1000, -1000, 30_000, -30_000];
        apply_volume(&mut pcm, 50);
        assert_eq!(pcm, [500, -500, 15_000, -15_000]);
        apply_volume(&mut pcm, 200);
        assert_eq!(pcm, [1000, -1000, 30_000, -30_000]);
        apply_volume(&mut pcm, 200);
        assert_eq!(pcm, [2000, -2000, 32_767, -32_768]);
        apply_volume(&mut pcm, 0);
        assert_eq!(pcm, [0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn pcm_is_cut_into_chunks() {
        let bytes: Vec<u8> = (0..(CHUNK_SAMPLES * 2 + 6))
            .map(|i| (i % 7) as u8)
            .collect();
        let (tx, mut rx) = mpsc::channel(10);
        read_pcm(&bytes[..], &tx).await.unwrap();
        drop(tx);
        let Some(Pcm::Chunk(first)) = rx.recv().await else {
            panic!()
        };
        assert_eq!(first.len(), CHUNK_SAMPLES);
        // 6 bytes left: one whole stereo pair (4 bytes) survives.
        let Some(Pcm::Chunk(rest)) = rx.recv().await else {
            panic!()
        };
        assert_eq!(rest.len(), 2);
        assert!(rx.recv().await.is_none());
    }
}
