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

    /// Look up a record for a specific (difficulty, mods) combo.
    pub fn get_with_mods(
        &self,
        slug: &str,
        difficulty: &str,
        mods_key: &str,
    ) -> Option<&BestScore> {
        let key = compose_key(difficulty, mods_key);
        self.entries.get(slug).and_then(|d| d.get(&key))
    }

    #[allow(dead_code)]
    pub fn update_if_best(&mut self, slug: &str, difficulty: &str, new: BestScore) -> bool {
        self.update_if_best_with_mods(slug, difficulty, "", new)
    }

    pub fn update_if_best_with_mods(
        &mut self,
        slug: &str,
        difficulty: &str,
        mods_key: &str,
        new: BestScore,
    ) -> bool {
        let entry = self.entries.entry(slug.to_string()).or_default();
        let key = compose_key(difficulty, mods_key);
        let is_new_best = entry
            .get(&key)
            .map(|prev| new.score > prev.score)
            .unwrap_or(true);
        if is_new_best {
            entry.insert(key, new);
        }
        is_new_best
    }

    /// Iterate all (key, score) pairs for a song. Key format: `"diff"` or
    /// `"diff|mods_key"`.
    pub fn all_for_song(&self, slug: &str) -> impl Iterator<Item = (&String, &BestScore)> {
        self.entries.get(slug).into_iter().flat_map(|m| m.iter())
    }
}

fn compose_key(difficulty: &str, mods_key: &str) -> String {
    if mods_key.is_empty() {
        difficulty.to_string()
    } else {
        format!("{}|{}", difficulty, mods_key)
    }
}

/// Split a composed key back into (difficulty, mods_key).
pub fn decompose_key(key: &str) -> (&str, &str) {
    match key.split_once('|') {
        Some((d, m)) => (d, m),
        None => (key, ""),
    }
}
