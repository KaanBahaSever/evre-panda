//! The now-playing panel: the "🐼 Şimdi çalıyor" message with buttons
//! (architecture.md §4.5, §8.1 components).
//!
//! Each song gets a new panel where it was asked for; the previous one loses
//! its buttons. The player edits the panel when what it shows changes
//! (pause, volume, repeat, the queue), debounced. The buttons do what the
//! commands do, under the same rules.

use era_bot_sdk::components::{ActionRow, Button, ButtonStyle, row};

use crate::lang::{self, Locale, fill, percent, strings};
use crate::queue::{LoopMode, Track};
use crate::text;

/// `customId`s of the panel's buttons.
pub mod id {
    /// ⏯ pause or resume.
    pub const PAUSE: &str = "panda:pause";
    /// ⏭ next song.
    pub const SKIP: &str = "panda:skip";
    /// ⏹ stop, clear the queue and leave.
    pub const STOP: &str = "panda:stop";
    /// 🔀 shuffle the queue.
    pub const SHUFFLE: &str = "panda:shuffle";
    /// 🔁 cycle the repeat mode.
    pub const LOOP: &str = "panda:loop";
    /// 🔉 volume down.
    pub const VOLUME_DOWN: &str = "panda:vol-";
    /// 🔊 volume up.
    pub const VOLUME_UP: &str = "panda:vol+";
    /// 📜 show the queue (only to the clicker).
    pub const QUEUE: &str = "panda:queue";
}

/// How much a volume button changes the volume, in percent.
pub const VOLUME_STEP: u32 = 10;

/// The highest volume (`!ses` takes 0–200).
pub const MAX_VOLUME: u32 = 200;

/// A panel button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Pause, or resume when paused.
    Pause,
    /// Next song.
    Skip,
    /// Stop and leave.
    Stop,
    /// Shuffle the queue.
    Shuffle,
    /// Next repeat mode (off → song → all → off).
    Loop,
    /// Volume − [`VOLUME_STEP`].
    VolumeDown,
    /// Volume + [`VOLUME_STEP`].
    VolumeUp,
    /// Show the queue.
    Queue,
}

impl Action {
    /// The button a `customId` names.
    #[must_use]
    pub fn parse(custom_id: &str) -> Option<Self> {
        Some(match custom_id {
            id::PAUSE => Self::Pause,
            id::SKIP => Self::Skip,
            id::STOP => Self::Stop,
            id::SHUFFLE => Self::Shuffle,
            id::LOOP => Self::Loop,
            id::VOLUME_DOWN => Self::VolumeDown,
            id::VOLUME_UP => Self::VolumeUp,
            id::QUEUE => Self::Queue,
            _ => return None,
        })
    }

    /// Whether it changes playback (and so needs the clicker in Panda's
    /// voice channel, like the commands); showing the queue does not.
    #[must_use]
    pub const fn controls_playback(self) -> bool {
        !matches!(self, Self::Queue)
    }
}

/// The new volume after a volume button.
#[must_use]
pub fn stepped_volume(volume: u32, up: bool) -> u32 {
    if up {
        (volume + VOLUME_STEP).min(MAX_VOLUME)
    } else {
        volume.saturating_sub(VOLUME_STEP)
    }
}

/// What a panel shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// The language it is written in.
    pub lang: Locale,
    /// The song playing.
    pub track: Track,
    /// Paused right now.
    pub paused: bool,
    /// Volume in percent.
    pub volume: u32,
    /// Repeat mode.
    pub loop_mode: LoopMode,
    /// Songs waiting after it.
    pub queued: usize,
}

/// The panel's text: the song, then its state.
#[must_use]
pub fn content(view: &View) -> String {
    let s = strings(view.lang);
    let mut state = Vec::new();
    if view.paused {
        state.push(s.panel_paused.to_owned());
    }
    state.push(format!("🔊 {}", percent(view.lang, view.volume)));
    match view.loop_mode {
        LoopMode::Off => {}
        LoopMode::One => state.push(s.panel_loop_one.to_owned()),
        LoopMode::All => state.push(s.panel_loop_all.to_owned()),
    }
    state.push(match view.queued {
        0 => s.panel_queue_empty.to_owned(),
        n => fill(s.panel_queued, &[("songs", &lang::songs(view.lang, n))]),
    });
    format!(
        "{}\n{}",
        text::now_playing(view.lang, &view.track),
        state.join(" · ")
    )
}

/// The panel's buttons for its state.
#[must_use]
pub fn components(view: &View) -> Vec<ActionRow> {
    let s = strings(view.lang);
    let pause = if view.paused {
        s.button_resume
    } else {
        s.button_pause
    };
    let (loop_label, loop_emoji) = match view.loop_mode {
        LoopMode::Off => (s.button_loop_off, "🔁"),
        LoopMode::One => (s.button_loop_one, "🔂"),
        LoopMode::All => (s.button_loop_all, "🔁"),
    };
    let loop_style = if view.loop_mode == LoopMode::Off {
        ButtonStyle::Secondary
    } else {
        ButtonStyle::Primary
    };
    vec![
        row([
            Button::new(
                if view.paused {
                    ButtonStyle::Primary
                } else {
                    ButtonStyle::Secondary
                },
                id::PAUSE,
                pause,
            )
            .emoji("⏯"),
            Button::secondary(id::SKIP, s.button_skip).emoji("⏭"),
            Button::danger(id::STOP, s.button_stop).emoji("⏹"),
            Button::secondary(id::SHUFFLE, s.button_shuffle)
                .emoji("🔀")
                .disabled(view.queued < 2),
            Button::new(loop_style, id::LOOP, loop_label).emoji(loop_emoji),
        ]),
        row([
            Button::secondary(id::VOLUME_DOWN, s.button_volume_down)
                .emoji("🔉")
                .disabled(view.volume == 0),
            Button::secondary(id::VOLUME_UP, s.button_volume_up)
                .emoji("🔊")
                .disabled(view.volume >= MAX_VOLUME),
            Button::secondary(id::QUEUE, s.button_queue).emoji("📜"),
        ]),
    ]
}

/// A panel's text once its song is over: its first line, as "🎵 Çaldı"
/// (in the language the panel was written in, which may be another than
/// now's after a restart with another `PANDA_LANG`).
#[must_use]
pub fn ended(content: &str) -> String {
    let first = content.lines().next().unwrap_or_default();
    Locale::ALL
        .into_iter()
        .map(strings)
        .find_map(|s| {
            first
                .strip_prefix(s.now_label)
                .map(|rest| format!("{}{rest}", s.played_label))
        })
        .unwrap_or_else(|| first.to_owned())
}

#[cfg(test)]
mod tests {
    use era_bot_sdk::components::custom_ids;

    use super::*;
    use crate::queue::tests::track;

    fn view() -> View {
        View {
            lang: Locale::Tr,
            track: track("Barış Manço - Gülpembe"),
            paused: false,
            volume: 100,
            loop_mode: LoopMode::Off,
            queued: 0,
        }
    }

    #[test]
    fn texts_follow_the_state() {
        let playing = content(&view());
        assert_eq!(
            playing,
            "🐼 Şimdi çalıyor: **Barış Manço - Gülpembe** (1:00) · isteyen: kaan\n🔊 %100 · Sırada şarkı yok"
        );
        let busy = content(&View {
            paused: true,
            volume: 80,
            loop_mode: LoopMode::All,
            queued: 3,
            ..view()
        });
        assert!(
            busy.ends_with("⏸ Duraklatıldı · 🔊 %80 · 🔁 Sıra tekrar · Sırada 3 şarkı"),
            "{busy}"
        );
        assert_eq!(
            ended(&playing),
            "🎵 Çaldı: **Barış Manço - Gülpembe** (1:00) · isteyen: kaan"
        );
        assert_eq!(ended("başka"), "başka");
    }

    #[test]
    fn buttons_follow_the_state() {
        let rows = components(&view());
        assert_eq!(
            custom_ids(&rows),
            [
                id::PAUSE,
                id::SKIP,
                id::STOP,
                id::SHUFFLE,
                id::LOOP,
                id::VOLUME_DOWN,
                id::VOLUME_UP,
                id::QUEUE
            ]
        );
        let button = |rows: &[ActionRow], id: &str| {
            rows.iter()
                .flat_map(|row| &row.components)
                .filter_map(|c| c.as_button())
                .find(|b| b.custom_id.as_deref() == Some(id))
                .cloned()
                .unwrap()
        };
        assert_eq!(button(&rows, id::PAUSE).label, "Duraklat");
        assert!(button(&rows, id::SHUFFLE).disabled, "nothing to shuffle");
        assert_eq!(button(&rows, id::LOOP).label, "Tekrar: kapalı");
        let paused = components(&View {
            paused: true,
            loop_mode: LoopMode::One,
            queued: 2,
            volume: MAX_VOLUME,
            ..view()
        });
        assert_eq!(button(&paused, id::PAUSE).label, "Devam");
        assert_eq!(button(&paused, id::PAUSE).style, ButtonStyle::Primary);
        assert!(!button(&paused, id::SHUFFLE).disabled);
        assert_eq!(button(&paused, id::LOOP).label, "Tekrar: şarkı");
        assert_eq!(button(&paused, id::LOOP).emoji.as_deref(), Some("🔂"));
        assert!(button(&paused, id::VOLUME_UP).disabled);
        let quiet = components(&View {
            volume: 0,
            ..view()
        });
        assert!(button(&quiet, id::VOLUME_DOWN).disabled);
        // Within the server's limits.
        assert!(rows.len() <= era_bot_sdk::protocol::rest::limits::MAX_COMPONENT_ROWS);
        assert!(
            rows.iter()
                .all(|r| r.components.len()
                    <= era_bot_sdk::protocol::rest::limits::MAX_ROW_COMPONENTS)
        );
    }

    #[test]
    fn panels_in_other_languages() {
        let label = |rows: &[ActionRow], id: &str| {
            rows.iter()
                .flat_map(|row| &row.components)
                .filter_map(|c| c.as_button())
                .find(|b| b.custom_id.as_deref() == Some(id))
                .map(|b| b.label.clone())
                .unwrap()
        };
        let english = View {
            lang: Locale::En,
            paused: true,
            loop_mode: LoopMode::One,
            queued: 1,
            ..view()
        };
        assert_eq!(
            content(&english),
            "🐼 Now playing: **Barış Manço - Gülpembe** (1:00) · requested by kaan\n⏸ Paused · 🔊 100% · 🔂 Song on repeat · 1 song queued"
        );
        let rows = components(&english);
        assert_eq!(label(&rows, id::PAUSE), "Resume");
        assert_eq!(label(&rows, id::LOOP), "Repeat: song");
        assert_eq!(label(&rows, id::QUEUE), "Queue");
        let french = View {
            lang: Locale::Fr,
            queued: 3,
            ..view()
        };
        assert!(
            content(&french).ends_with("🔊 100 % · 3 titres en attente"),
            "{}",
            content(&french)
        );
        let rows = components(&french);
        assert_eq!(label(&rows, id::PAUSE), "Pause");
        assert_eq!(label(&rows, id::SKIP), "Passer");
        assert_eq!(label(&rows, id::STOP), "Arrêter");
        assert_eq!(label(&rows, id::LOOP), "Boucle : non");
        // Every language's panel ends as "played", whatever the language now.
        for lang in Locale::ALL {
            let shown = content(&View { lang, ..view() });
            let over = ended(&shown);
            assert!(over.starts_with(strings(lang).played_label), "{over}");
            assert!(!over.contains('\n'), "{over}");
        }
    }

    #[test]
    fn actions_and_volume_steps() {
        for (custom_id, action) in [
            (id::PAUSE, Action::Pause),
            (id::SKIP, Action::Skip),
            (id::STOP, Action::Stop),
            (id::SHUFFLE, Action::Shuffle),
            (id::LOOP, Action::Loop),
            (id::VOLUME_DOWN, Action::VolumeDown),
            (id::VOLUME_UP, Action::VolumeUp),
            (id::QUEUE, Action::Queue),
        ] {
            assert_eq!(Action::parse(custom_id), Some(action));
        }
        assert_eq!(Action::parse("panda:dance"), None);
        assert!(!Action::Queue.controls_playback());
        assert!(Action::Skip.controls_playback());
        assert_eq!(stepped_volume(100, true), 110);
        assert_eq!(stepped_volume(195, true), 200);
        assert_eq!(stepped_volume(5, false), 0);
        assert_eq!(stepped_volume(100, false), 90);
    }
}
