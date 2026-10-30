//! What Panda says (casual Markdown, in the language of `PANDA_LANG`, see
//! [`crate::lang`]) and how it formats times.

use std::fmt::Write as _;
use std::time::Duration;

use crate::lang::{Locale, fill, percent, songs, strings};
use crate::queue::{LoopMode, Queue, Track};

/// `3:07`, `1:02:03`.
#[must_use]
pub fn duration(value: Duration) -> String {
    let total = value.as_secs();
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// A track's length for chat: `3:07`, `canlı` (`live`) or `?`.
#[must_use]
pub fn length(lang: Locale, track: &Track) -> String {
    if track.live {
        strings(lang).live.into()
    } else {
        track.duration.map_or_else(|| "?".into(), duration)
    }
}

/// `▬▬▬▬🔘▬▬▬▬▬▬` for a position in a song.
#[must_use]
pub fn progress_bar(position: Duration, total: Duration) -> String {
    const WIDTH: usize = 12;
    let fraction = if total.is_zero() {
        0.0
    } else {
        (position.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let knob = ((fraction * (WIDTH - 1) as f64).round() as usize).min(WIDTH - 1);
    (0..WIDTH)
        .map(|i| if i == knob { "🔘" } else { "▬" })
        .collect()
}

/// Markdown-safe title: no line breaks, no stray `*`, `_`, `` ` `` or `~`,
/// no mentions, at most 120 characters ("Adsız şarkı" in the language when
/// nothing is left).
#[must_use]
pub fn clean_title(lang: Locale, title: &str) -> String {
    let mut out: String = no_mentions(title)
        .chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            c => c,
        })
        .filter(|c| !matches!(c, '*' | '_' | '`' | '~' | '|'))
        .collect();
    out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > 120 {
        out = out.chars().take(117).collect::<String>() + "…";
    }
    if out.is_empty() {
        strings(lang).untitled.into()
    } else {
        out
    }
}

/// Text from others (a song's title, a display name) without anything the
/// server reads as a mention (`<@id>`): the message is Panda's, so Panda
/// would ping whoever a requester names, also people who blocked the
/// requester (third security review). `<` becomes the look-alike `‹`.
#[must_use]
pub fn no_mentions(text: &str) -> String {
    text.replace('<', "‹")
}

/// A requester's name as Panda writes it: no mentions, no Markdown, no
/// control characters.
#[must_use]
pub fn requester(track: &Track) -> String {
    let name: String = no_mentions(&track.requester_name)
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '*' | '_' | '`' | '~' | '|'))
        .collect();
    if name.trim().is_empty() {
        "?".into()
    } else {
        name
    }
}

/// The announcement when a song starts: "🐼 Şimdi çalıyor: **title**
/// (3:07) · isteyen: name".
#[must_use]
pub fn now_playing(lang: Locale, track: &Track) -> String {
    let s = strings(lang);
    format!(
        "{} **{}** ({}) · {}",
        s.now_label,
        clean_title(lang, &track.title),
        length(lang, track),
        fill(s.requested_by, &[("name", &requester(track))])
    )
}

/// The answer when a song is queued behind others.
#[must_use]
pub fn queued(lang: Locale, track: &Track, position: usize) -> String {
    fill(
        strings(lang).queued,
        &[
            ("n", &position),
            ("title", &clean_title(lang, &track.title)),
            ("length", &length(lang, track)),
        ],
    )
}

/// `!şimdi`: the current song with its progress.
#[must_use]
pub fn now(
    lang: Locale,
    track: &Track,
    position: Duration,
    paused: bool,
    volume: u32,
    loop_mode: LoopMode,
) -> String {
    let s = strings(lang);
    let mut text = format!("🎶 **{}**", clean_title(lang, &track.title));
    if let Some(link) = &track.link {
        let _ = write!(text, "\n<{link}>");
    }
    match (track.live, track.duration) {
        (false, Some(total)) => {
            let _ = write!(
                text,
                "\n{} {} / {}",
                progress_bar(position, total),
                duration(position),
                duration(total)
            );
        }
        _ => {
            let _ = write!(
                text,
                "\n{}",
                fill(s.listening, &[("time", &duration(position))])
            );
        }
    }
    text.push('\n');
    text.push_str(&fill(
        s.now_details,
        &[
            ("name", &requester(track)),
            ("volume", &percent(lang, volume)),
        ],
    ));
    if paused {
        text.push_str(s.now_paused);
    }
    match loop_mode {
        LoopMode::Off => {}
        LoopMode::One => text.push_str(s.now_loop_one),
        LoopMode::All => text.push_str(s.now_loop_all),
    }
    text
}

/// How many songs `!sıra` lists.
const QUEUE_LINES: usize = 10;

/// `!sıra`.
#[must_use]
pub fn queue(lang: Locale, prefix: &str, current: Option<&Track>, queue: &Queue) -> String {
    let s = strings(lang);
    if current.is_none() && queue.is_empty() {
        return fill(s.queue_empty, &[("p", &prefix)]);
    }
    let mut text = String::new();
    if let Some(track) = current {
        let _ = writeln!(
            text,
            "▶️ **{}** ({}) · {}",
            clean_title(lang, &track.title),
            length(lang, track),
            requester(track)
        );
    }
    if queue.is_empty() {
        text.push_str(s.queue_no_more);
    } else {
        let header = fill(
            s.queue_header,
            &[
                ("songs", &songs(lang, queue.len())),
                ("time", &duration(queue.total_duration())),
            ],
        );
        let _ = writeln!(text, "{header}");
        for (index, track) in queue.iter().take(QUEUE_LINES).enumerate() {
            let _ = writeln!(
                text,
                "`{}.` {} ({}) · {}",
                index + 1,
                clean_title(lang, &track.title),
                length(lang, track),
                requester(track)
            );
        }
        if queue.len() > QUEUE_LINES {
            let more = fill(
                s.queue_more,
                &[("songs", &songs(lang, queue.len() - QUEUE_LINES))],
            );
            let _ = writeln!(text, "{more}");
        }
    }
    match queue.loop_mode {
        LoopMode::Off => {}
        LoopMode::One => text.push_str(s.queue_loop_one),
        LoopMode::All => text.push_str(s.queue_loop_all),
    }
    text.trim_end().to_owned()
}

/// A playback control from someone outside Panda's voice channel.
#[must_use]
pub const fn not_in_channel(lang: Locale) -> &'static str {
    strings(lang).not_in_channel
}

/// `!tekrar`.
#[must_use]
pub const fn loop_mode(lang: Locale, mode: LoopMode) -> &'static str {
    let s = strings(lang);
    match mode {
        LoopMode::Off => s.loop_off,
        LoopMode::One => s.loop_one,
        LoopMode::All => s.loop_all,
    }
}

/// `!yardım`.
#[must_use]
pub fn help(lang: Locale, prefix: &str) -> String {
    fill(strings(lang).help, &[("p", &prefix)])
}

/// Why a song could not be found or played, from yt-dlp's or ffmpeg's
/// words.
#[must_use]
pub fn media_error(lang: Locale, reason: &str) -> String {
    let s = strings(lang);
    match reason {
        "bot-check" => s.err_bot_check,
        "age" => s.err_age,
        "unavailable" => s.err_unavailable,
        "not-found" => s.err_not_found,
        "missing-ytdlp" => s.err_missing_ytdlp,
        "missing-ffmpeg" => s.err_missing_ffmpeg,
        "timeout" => s.err_timeout,
        "local" => s.err_local,
        "unsupported" => s.err_unsupported,
        // yt-dlp's and ffmpeg's own words stay in the log: they would tell
        // whoever posted the link whether a host or port answered, and
        // repeat whatever the page said (third security review).
        _ => s.err_other,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::tests::track;

    const TR: Locale = Locale::Tr;

    #[test]
    fn durations() {
        assert_eq!(duration(Duration::from_secs(5)), "0:05");
        assert_eq!(duration(Duration::from_secs(187)), "3:07");
        assert_eq!(duration(Duration::from_secs(3723)), "1:02:03");
    }

    #[test]
    fn progress_bars() {
        let total = Duration::from_secs(100);
        assert!(progress_bar(Duration::ZERO, total).starts_with("🔘"));
        assert!(progress_bar(total, total).ends_with("🔘"));
        assert_eq!(
            progress_bar(Duration::from_secs(50), total)
                .matches('🔘')
                .count(),
            1
        );
        assert!(progress_bar(Duration::from_secs(5), Duration::ZERO).starts_with("🔘"));
    }

    /// Third security review: Panda never pings for a requester.
    #[test]
    fn titles_and_names_carry_no_mentions() {
        assert_eq!(clean_title(TR, "hey <@42> dinle"), "hey ‹@42> dinle");
        let mut song = track("<@7> şarkısı");
        song.requester_name = "**<@9>**".into();
        for lang in Locale::ALL {
            let shown = now_playing(lang, &song);
            assert!(!shown.contains("<@"), "{shown}");
            assert!(shown.ends_with("‹@9>"), "{shown}");
        }
        assert!(now_playing(TR, &song).ends_with("isteyen: ‹@9>"));
        song.requester_name = "\u{7}".into();
        assert_eq!(requester(&song), "?");
        // yt-dlp's own words never reach the chat.
        assert_eq!(
            media_error(TR, "HTTP Error 403: 10.0.0.5 refused"),
            "Olmadı, bunu açamadım 😕"
        );
        assert_eq!(
            media_error(Locale::Fr, "HTTP Error 403: 10.0.0.5 refused"),
            "Ça n'a pas marché, je n'ai pas pu l'ouvrir 😕"
        );
    }

    #[test]
    fn titles_are_cleaned() {
        assert_eq!(clean_title(TR, "**Bold**\nnew_line"), "Bold newline");
        assert_eq!(clean_title(TR, "  "), "Adsız şarkı");
        assert_eq!(clean_title(Locale::En, "  "), "Untitled song");
        let long = "a".repeat(200);
        assert_eq!(clean_title(TR, &long).chars().count(), 118);
    }

    #[test]
    fn messages() {
        let song = track("Barış Manço - Gülpembe");
        assert_eq!(
            now_playing(TR, &song),
            "🐼 Şimdi çalıyor: **Barış Manço - Gülpembe** (1:00) · isteyen: kaan"
        );
        assert!(queued(TR, &song, 3).starts_with("Sıraya ekledim (#3)"));
        let mut queue = Queue::new(50);
        assert_eq!(
            super::queue(TR, "!", None, &queue),
            "Sıra boş. `!çal <şarkı adı ya da link>` ile bir şey ekle 🎵"
        );
        for n in 0..12 {
            queue.push(track(&format!("şarkı {n}"))).unwrap();
        }
        let text = super::queue(TR, "!", Some(&song), &queue);
        assert!(text.contains("Sırada 12 şarkı var (12:00)"), "{text}");
        assert!(text.contains("…ve 2 şarkı daha"), "{text}");
        let shown = now(TR, &song, Duration::from_secs(30), true, 150, LoopMode::One);
        assert!(
            shown.contains("0:30 / 1:00") && shown.contains("%150") && shown.contains("⏸"),
            "{shown}"
        );
        assert!(shown.contains("İsteyen: kaan · Ses: %150"), "{shown}");
        assert!(help(TR, "!").contains("`!çal"));
    }

    #[test]
    fn english_and_french_messages() {
        let song = track("Lofi");
        assert_eq!(
            now_playing(Locale::En, &song),
            "🐼 Now playing: **Lofi** (1:00) · requested by kaan"
        );
        assert_eq!(
            now_playing(Locale::Fr, &song),
            "🐼 En cours : **Lofi** (1:00) · demandé par kaan"
        );
        assert_eq!(
            queued(Locale::En, &song, 2),
            "Added to the queue (#2): **Lofi** (1:00)"
        );
        let mut queue = Queue::new(50);
        assert_eq!(
            super::queue(Locale::Fr, "?", None, &queue),
            "La file est vide. Ajoute quelque chose avec `?jouer <titre ou lien>` 🎵"
        );
        queue.push(track("un")).unwrap();
        let text = super::queue(Locale::En, "!", Some(&song), &queue);
        assert!(text.contains("1 song in the queue (1:00):"), "{text}");
        queue.push(track("deux")).unwrap();
        queue.loop_mode = LoopMode::All;
        let text = super::queue(Locale::Fr, "!", Some(&song), &queue);
        assert!(text.contains("2 titres dans la file (2:00) :"), "{text}");
        assert!(text.ends_with("🔁 La file tourne en boucle."), "{text}");
        let mut live = track("Radio");
        live.live = true;
        let shown = now(
            Locale::En,
            &live,
            Duration::from_secs(90),
            false,
            80,
            LoopMode::Off,
        );
        assert!(shown.contains("playing for 1:30"), "{shown}");
        assert!(shown.contains("Requested by kaan · Volume: 80%"), "{shown}");
        assert!(now_playing(Locale::En, &live).contains("(live)"));
        assert_eq!(
            loop_mode(Locale::Fr, LoopMode::One),
            "🔂 Ce titre va se répéter."
        );
        assert!(help(Locale::Fr, "!").contains("`!jouer <titre ou lien>`"));
        assert!(help(Locale::En, "!").contains("`!play <song name or link>`"));
        assert_eq!(
            not_in_channel(Locale::En),
            "You need to be in my voice channel for that 🙂"
        );
    }

    #[test]
    fn every_media_error_has_its_own_text() {
        let reasons = [
            "bot-check",
            "age",
            "unavailable",
            "not-found",
            "missing-ytdlp",
            "missing-ffmpeg",
            "timeout",
            "local",
            "unsupported",
            "something else",
        ];
        for lang in Locale::ALL {
            let mut seen: Vec<String> = reasons.iter().map(|r| media_error(lang, r)).collect();
            seen.sort();
            seen.dedup();
            assert_eq!(seen.len(), reasons.len(), "{lang:?}");
        }
    }
}
