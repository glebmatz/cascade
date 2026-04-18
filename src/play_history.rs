use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// One completed (non-practice) play.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayRecord {
    pub ts: u64,
    pub slug: String,
    pub title: String,
    pub difficulty: String,
    pub mods: String,
    pub score: u64,
    pub accuracy: f64,
    pub max_combo: u32,
    pub total_notes: u32,
    pub judgements: [u32; 4],
    pub duration_played_ms: u64,
    pub song_duration_ms: u64,
    pub grade: String,
    pub died: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayHistory {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub plays: Vec<PlayRecord>,
}

fn default_version() -> u32 {
    1
}

impl Default for PlayHistory {
    fn default() -> Self {
        Self {
            version: 1,
            plays: Vec::new(),
        }
    }
}

impl PlayHistory {
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn append(&mut self, rec: PlayRecord) {
        self.plays.push(rec);
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
