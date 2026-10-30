//! Songs and the queue.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use era_bot_sdk::protocol::Snowflake;
use rand::seq::SliceRandom;

/// How a song's audio is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Through yt-dlp (YouTube, SoundCloud and every other site it knows).
    YtDlp(String),
    /// An audio file or stream link, read by ffmpeg itself.
    Direct(String),
}

/// One song in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// Unique per process, so a prefetched decoder matches its song.
    pub id: u64,
    /// Title to show.
    pub title: String,
    /// What to decode.
    pub source: Source,
    /// The page to link, if any.
    pub link: Option<String>,
    /// Length; `None` when unknown or live.
    pub duration: Option<Duration>,
    /// A live stream (radio, live video).
    pub live: bool,
    /// Who asked for it.
    pub requester: Snowflake,
    /// Their name at the time.
    pub requester_name: String,
    /// Where they asked (announcements go there).
    pub text_channel: Snowflake,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A fresh track ID.
#[must_use]
pub fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Repeat mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoopMode {
    /// Play each song once.
    #[default]
    Off,
    /// Repeat the current song.
    One,
    /// Repeat the whole queue.
    All,
}

impl LoopMode {
    /// The next mode for a bare `!tekrar`: off → one → all → off.
    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::Off => Self::One,
            Self::One => Self::All,
            Self::All => Self::Off,
        }
    }

    /// Parses `kapalı/şarkı/hepsi`, `off/one/all`, `non/titre/tout`,
    /// `aus/titel/alle` and `no/una/todo` (and a few spellings).
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_lowercase().as_str() {
            "kapalı" | "kapali" | "kapat" | "off" | "none" | "0" | "yok" | "non" | "aucun"
            | "désactivé" | "desactive" | "aus" | "no" | "apagado" => Some(Self::Off),
            "şarkı" | "sarki" | "bu" | "tek" | "one" | "song" | "track" | "1" | "titre"
            | "morceau" | "chanson" | "titel" | "lied" | "una" | "canción" | "cancion" => {
                Some(Self::One)
            }
            "hepsi" | "sıra" | "sira" | "tümü" | "tumu" | "all" | "queue" | "tout" | "tous"
            | "file" | "alle" | "alles" | "todo" | "todas" | "cola" => Some(Self::All),
            _ => None,
        }
    }
}

/// Why a song was not queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueFull(pub usize);

/// The songs waiting to play (the current one is not in here).
#[derive(Debug, Clone)]
pub struct Queue {
    items: VecDeque<Track>,
    /// Repeat mode.
    pub loop_mode: LoopMode,
    max: usize,
}

impl Queue {
    /// An empty queue holding at most `max` songs.
    #[must_use]
    pub fn new(max: usize) -> Self {
        Self {
            items: VecDeque::new(),
            loop_mode: LoopMode::Off,
            max: max.max(1),
        }
    }

    /// Adds a song at the end; returns its 1-based position.
    pub fn push(&mut self, track: Track) -> Result<usize, QueueFull> {
        if self.items.len() >= self.max {
            return Err(QueueFull(self.max));
        }
        self.items.push_back(track);
        Ok(self.items.len())
    }

    /// Puts a song first (it plays next).
    pub fn push_front(&mut self, track: Track) {
        self.items.push_front(track);
    }

    /// The next song to play.
    pub fn pop_next(&mut self) -> Option<Track> {
        self.items.pop_front()
    }

    /// What happens to a song that played to its end: again (`One`), at the
    /// back (`All`) or gone (`Off`).
    pub fn finished(&mut self, track: Track) {
        match self.loop_mode {
            LoopMode::Off => {}
            LoopMode::One => self.items.push_front(track),
            LoopMode::All => self.items.push_back(track),
        }
    }

    /// The song that plays next.
    #[must_use]
    pub fn front(&self) -> Option<&Track> {
        self.items.front()
    }

    /// Shuffles the waiting songs.
    pub fn shuffle(&mut self) {
        self.items.make_contiguous().shuffle(&mut rand::rng());
    }

    /// Removes the song at a 1-based position.
    pub fn remove(&mut self, position: usize) -> Option<Track> {
        position
            .checked_sub(1)
            .and_then(|index| self.items.remove(index))
    }

    /// Removes every waiting song.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Songs waiting.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Nothing waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The waiting songs in order.
    pub fn iter(&self) -> impl Iterator<Item = &Track> {
        self.items.iter()
    }

    /// Total length of the songs with a known length.
    #[must_use]
    pub fn total_duration(&self) -> Duration {
        self.items.iter().filter_map(|track| track.duration).sum()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn track(title: &str) -> Track {
        Track {
            id: next_id(),
            title: title.into(),
            source: Source::YtDlp(format!("https://example.com/{title}")),
            link: None,
            duration: Some(Duration::from_secs(60)),
            live: false,
            requester: Snowflake::new(1),
            requester_name: "kaan".into(),
            text_channel: Snowflake::new(2),
        }
    }

    fn titles(queue: &Queue) -> Vec<&str> {
        queue.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn order_limit_and_removal() {
        let mut queue = Queue::new(3);
        assert_eq!(queue.push(track("a")), Ok(1));
        assert_eq!(queue.push(track("b")), Ok(2));
        assert_eq!(queue.push(track("c")), Ok(3));
        assert_eq!(queue.push(track("d")), Err(QueueFull(3)));
        assert_eq!(queue.total_duration(), Duration::from_secs(180));
        assert_eq!(queue.remove(2).map(|t| t.title), Some("b".into()));
        assert_eq!(queue.remove(0), None);
        assert_eq!(queue.remove(9), None);
        assert_eq!(titles(&queue), ["a", "c"]);
        assert_eq!(queue.pop_next().map(|t| t.title), Some("a".into()));
        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn loop_modes() {
        let mut queue = Queue::new(10);
        queue.push(track("b")).unwrap();
        queue.finished(track("a"));
        assert_eq!(titles(&queue), ["b"], "off: gone");
        queue.loop_mode = LoopMode::One;
        queue.finished(track("a"));
        assert_eq!(titles(&queue), ["a", "b"], "one: again first");
        queue.pop_next();
        queue.loop_mode = LoopMode::All;
        queue.finished(track("a"));
        assert_eq!(titles(&queue), ["b", "a"], "all: at the back");

        assert_eq!(LoopMode::Off.cycle(), LoopMode::One);
        assert_eq!(LoopMode::All.cycle(), LoopMode::Off);
        assert_eq!(LoopMode::parse("Şarkı"), Some(LoopMode::One));
        assert_eq!(LoopMode::parse("hepsi"), Some(LoopMode::All));
        assert_eq!(LoopMode::parse("off"), Some(LoopMode::Off));
        assert_eq!(LoopMode::parse("belki"), None);
        // The words each language's usage line offers.
        for (words, mode) in [
            (["kapalı", "off", "non", "aus", "no"], LoopMode::Off),
            (["şarkı", "one", "titre", "titel", "una"], LoopMode::One),
            (["hepsi", "all", "tout", "alle", "todo"], LoopMode::All),
        ] {
            for word in words {
                assert_eq!(LoopMode::parse(word), Some(mode), "{word}");
            }
        }
    }

    #[test]
    fn shuffle_keeps_every_song() {
        let mut queue = Queue::new(100);
        for n in 0..50 {
            queue.push(track(&n.to_string())).unwrap();
        }
        queue.shuffle();
        let mut seen: Vec<_> = queue
            .iter()
            .map(|t| t.title.parse::<u32>().unwrap())
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..50).collect::<Vec<_>>());
    }
}
