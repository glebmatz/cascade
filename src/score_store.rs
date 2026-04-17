use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BestScore {
    pub score: u64,
    pub max_combo: u32,
    pub accuracy: f64,
    pub grade: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ScoreStore {
    pub entries: HashMap<String, HashMap<String, BestScore>>,
}

impl ScoreStore {
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        let Ok(s) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&s).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn get(&self, slug: &str, difficulty: &str) -> Option<&BestScore> {
        self.entries.get(slug).and_then(|d| d.get(difficulty))
    }

    pub fn update_if_best(&mut self, slug: &str, difficulty: &str, new: BestScore) -> bool {
        let entry = self.entries.entry(slug.to_string()).or_default();
        let is_new_best = entry
            .get(difficulty)
            .map(|prev| new.score > prev.score)
            .unwrap_or(true);
        if is_new_best {
            entry.insert(difficulty.to_string(), new);
        }
        is_new_best
    }
}
