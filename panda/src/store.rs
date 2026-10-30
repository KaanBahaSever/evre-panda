//! Settings kept across restarts: each guild's volume and repeat mode, in
//! `<data dir>/settings.json`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use era_bot_sdk::protocol::Snowflake;
use serde::{Deserialize, Serialize};

use crate::queue::LoopMode;

/// One guild's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildSettings {
    /// Volume in percent (0–200).
    pub volume: u32,
    /// Repeat mode.
    #[serde(default)]
    pub loop_mode: LoopMode,
}

/// The settings file.
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    default_volume: u32,
    guilds: Mutex<HashMap<String, GuildSettings>>,
}

impl Store {
    /// Loads `<data_dir>/settings.json` (a missing or broken file means
    /// defaults).
    #[must_use]
    pub fn open(data_dir: &std::path::Path, default_volume: u32) -> Self {
        let path = data_dir.join("settings.json");
        let guilds = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            default_volume,
            guilds: Mutex::new(guilds),
        }
    }

    /// A guild's settings.
    #[must_use]
    pub fn get(&self, guild_id: Snowflake) -> GuildSettings {
        self.guilds
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&guild_id.to_string())
            .copied()
            .unwrap_or(GuildSettings {
                volume: self.default_volume,
                loop_mode: LoopMode::Off,
            })
    }

    /// Changes a guild's settings and writes the file (atomically: a
    /// temporary file renamed over the old one).
    pub fn update(&self, guild_id: Snowflake, change: impl FnOnce(&mut GuildSettings)) {
        let mut settings = self.get(guild_id);
        change(&mut settings);
        let snapshot = {
            let mut guilds = self.guilds.lock().unwrap_or_else(|p| p.into_inner());
            guilds.insert(guild_id.to_string(), settings);
            guilds.clone()
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&snapshot) else {
            return;
        };
        let temporary = self.path.with_extension("json.tmp");
        let written = std::fs::write(&temporary, bytes)
            .and_then(|()| std::fs::rename(&temporary, &self.path));
        if let Err(error) = written {
            log::warn!("cannot save {}: {error}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("panda-store-{:x}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let guild = Snowflake::new(7);
        let store = Store::open(&dir, 80);
        assert_eq!(store.get(guild).volume, 80);
        store.update(guild, |s| {
            s.volume = 150;
            s.loop_mode = LoopMode::All;
        });
        let again = Store::open(&dir, 80);
        assert_eq!(
            again.get(guild),
            GuildSettings {
                volume: 150,
                loop_mode: LoopMode::All
            }
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
