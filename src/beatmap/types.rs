use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beatmap {
    pub version: u32,
    pub song: SongMeta,
    pub difficulty: Difficulty,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongMeta {
    pub title: String,
    pub artist: String,
    pub audio_file: String,
    pub bpm: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
}

impl Difficulty {
    pub fn filename(&self) -> String {
        format!(
            "{}.json",
            match self {
                Difficulty::Easy => "easy",
                Difficulty::Medium => "medium",
                Difficulty::Hard => "hard",
                Difficulty::Expert => "expert",
            }
        )
    }

    pub fn all() -> &'static [Difficulty] {
        &[
            Difficulty::Easy,
            Difficulty::Medium,
            Difficulty::Hard,
            Difficulty::Expert,
        ]
    }
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "Easy"),
            Difficulty::Medium => write!(f, "Medium"),
            Difficulty::Hard => write!(f, "Hard"),
            Difficulty::Expert => write!(f, "Expert"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub time_ms: u64,
    pub lane: u8,
    /// Duration in ms for hold notes. 0 = tap note.
    #[serde(default)]
    pub duration_ms: u64,
    /// For slide notes: the target lane the player must transition to before
    /// the hold ends. `None` for plain taps and regular holds. Old beatmaps
    /// without this field deserialise as `None` (backward-compatible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide_to: Option<u8>,
}
