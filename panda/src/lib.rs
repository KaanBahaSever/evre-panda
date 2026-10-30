//! Panda v2: Era's music bot (architecture.md §4.5), a Rust rewrite of
//! Panda v1 (TeamSpeak, C#) built only on the public `era-bot-sdk`.
//!
//! Chat commands in Turkish, English, French, German and Spanish (`!çal` /
//! `!play` / `!jouer`, ...) queue songs from YouTube, SoundCloud and any
//! site yt-dlp knows, or direct audio links. Audio goes yt-dlp → ffmpeg
//! (EBU R128 `loudnorm`) → 48 kHz stereo PCM → the bot's music track in the
//! caller's voice channel, at the bitrate the guild's plan allows. Replies
//! and the panel speak `PANDA_LANG` ([`lang`]).
//!
//! Needs `yt-dlp` and `ffmpeg` (with `ffprobe`) on the PATH or configured
//! (see [`config`]).

pub mod bot;
pub mod check;
pub mod config;
pub mod lang;
pub mod media;
pub mod panel;
pub mod player;
pub mod queue;
pub mod store;
pub mod text;

pub use bot::{run, run_until};
pub use config::Config;
