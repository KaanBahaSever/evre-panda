//! `panda --check`: is this machine ready to play? ffmpeg (with loudnorm),
//! yt-dlp and the voice stack (libwebrtc), without connecting anywhere.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::media;

/// Decodes one second of a generated tone through Panda's ffmpeg filter
/// chain; returns its level in dBFS.
async fn ffmpeg_tone(ffmpeg: &Path) -> Result<f32, String> {
    let mut child = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-af",
            "loudnorm=I=-14:TP=-1.5:LRA=11",
            "-f",
            "s16le",
            "-ar",
            "48000",
            "-ac",
            "2",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut pcm = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        tokio::time::timeout(Duration::from_secs(20), stdout.read_to_end(&mut pcm))
            .await
            .map_err(|_| "timed out".to_owned())?
            .map_err(|error| error.to_string())?;
    }
    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(media::friendly_error(&String::from_utf8_lossy(
            &output.stderr,
        )));
    }
    let samples: Vec<f32> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0)
        .collect();
    if samples.len() < 48_000 {
        return Err(format!("only {} samples", samples.len()));
    }
    let power = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    Ok(10.0 * power.max(1e-12).log10())
}

/// Prints one line per check; `true` when everything needed works.
pub async fn run(ffmpeg: &Path, ytdlp: &Path) -> bool {
    let mut ok = true;
    match media::version(ffmpeg, "-version").await {
        Some(version) => println!("ok    ffmpeg: {version}"),
        None => {
            println!("FAIL  ffmpeg not found ({})", ffmpeg.display());
            ok = false;
        }
    }
    if ok {
        match ffmpeg_tone(ffmpeg).await {
            Ok(db) => println!("ok    ffmpeg decodes with loudnorm ({db:.1} dBFS)"),
            Err(error) => {
                println!("FAIL  ffmpeg with loudnorm: {error}");
                ok = false;
            }
        }
    }
    match media::version(ytdlp, "--version").await {
        Some(version) => println!("ok    yt-dlp {version}"),
        None => println!(
            "warn  yt-dlp not found ({}): only direct audio links will play",
            ytdlp.display()
        ),
    }
    match era_bot_sdk::voice::self_check().await {
        Ok(db) => println!("ok    voice (libwebrtc) passes audio ({db:.1} dBFS)"),
        Err(error) => {
            println!("FAIL  voice: {error}");
            ok = false;
        }
    }
    ok
}
