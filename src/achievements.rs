use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::game::modifiers::{Modifier, Mods};
use crate::game::state::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AchievementId {
    FirstSteps,
    Centurion,
    Millennium,
    FullCombo,
    SRank,
    SsRank,
    UntouchableHard,
    PerfectStorm,
    InTheDark,
    Searchlight,
    OneShot,
    PixelPerfect,
}

impl AchievementId {
    pub const ALL: &'static [AchievementId] = &[
        AchievementId::FirstSteps,
        AchievementId::Centurion,
        AchievementId::Millennium,
        AchievementId::FullCombo,
        AchievementId::SRank,
        AchievementId::SsRank,
        AchievementId::UntouchableHard,
        AchievementId::PerfectStorm,
        AchievementId::InTheDark,
        AchievementId::Searchlight,
        AchievementId::OneShot,
        AchievementId::PixelPerfect,
    ];

    pub fn key(self) -> &'static str {
        match self {
            AchievementId::FirstSteps => "first_steps",
            AchievementId::Centurion => "centurion",
            AchievementId::Millennium => "millennium",
            AchievementId::FullCombo => "full_combo",
            AchievementId::SRank => "s_rank",
            AchievementId::SsRank => "ss_rank",
            AchievementId::UntouchableHard => "untouchable_hard",
            AchievementId::PerfectStorm => "perfect_storm",
            AchievementId::InTheDark => "in_the_dark",
            AchievementId::Searchlight => "searchlight",
            AchievementId::OneShot => "one_shot",
            AchievementId::PixelPerfect => "pixel_perfect",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            AchievementId::FirstSteps => "First Steps",
            AchievementId::Centurion => "Centurion",
            AchievementId::Millennium => "Millennium",
            AchievementId::FullCombo => "Full Combo",
            AchievementId::SRank => "S Rank",
            AchievementId::SsRank => "Untouchable",
            AchievementId::UntouchableHard => "Hard Mode Hero",
            AchievementId::PerfectStorm => "Perfect Storm",
            AchievementId::InTheDark => "In the Dark",
            AchievementId::Searchlight => "Searchlight",
            AchievementId::OneShot => "One Shot",
            AchievementId::PixelPerfect => "Pixel Perfect",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            AchievementId::FirstSteps => "Complete your first song.",
            AchievementId::Centurion => "Hit a 100-note combo.",
            AchievementId::Millennium => "Hit a 1000-note combo.",
            AchievementId::FullCombo => "Finish a song with no Misses.",
            AchievementId::SRank => "Earn an S grade on any song.",
            AchievementId::SsRank => "Earn an SS grade (100% accuracy).",
            AchievementId::UntouchableHard => "Full Combo on Hard difficulty.",
            AchievementId::PerfectStorm => "Full Combo on Expert difficulty.",
            AchievementId::InTheDark => "Beat a song with the Hidden mod.",
            AchievementId::Searchlight => "Beat a song with the Flashlight mod.",
            AchievementId::OneShot => "Survive a song with Sudden Death.",
            AchievementId::PixelPerfect => "Beat a song with Perfect Only.",
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AchievementStore {
    pub unlocked: HashMap<String, String>,
}

impl AchievementStore {
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn is_unlocked(&self, id: AchievementId) -> bool {
        self.unlocked.contains_key(id.key())
    }

    /// Check predicates against the result of one song; returns the list of
    /// achievements that were newly unlocked by this play.
    pub fn check(
        &mut self,
        state: &GameState,
        difficulty: &str,
        mods: &Mods,
    ) -> Vec<AchievementId> {
        let now = chrono_iso8601();
        let full_combo = state.judgement_counts[3] == 0 && state.total_notes > 0;
        let mut newly = Vec::new();

        let unlock =
            |store: &mut AchievementStore, id: AchievementId, list: &mut Vec<AchievementId>| {
                if !store.is_unlocked(id) {
                    store.unlocked.insert(id.key().to_string(), now.clone());
                    list.push(id);
                }
            };

        if state.total_notes > 0 {
            unlock(self, AchievementId::FirstSteps, &mut newly);
        }
        if state.max_combo >= 100 {
            unlock(self, AchievementId::Centurion, &mut newly);
        }
        if state.max_combo >= 1000 {
            unlock(self, AchievementId::Millennium, &mut newly);
        }
        if full_combo {
            unlock(self, AchievementId::FullCombo, &mut newly);
        }
        match state.grade() {
            "S" | "SS" => unlock(self, AchievementId::SRank, &mut newly),
            _ => {}
        }
        if state.grade() == "SS" {
            unlock(self, AchievementId::SsRank, &mut newly);
        }
        if full_combo && difficulty.eq_ignore_ascii_case("hard") {
            unlock(self, AchievementId::UntouchableHard, &mut newly);
        }
        if full_combo && difficulty.eq_ignore_ascii_case("expert") {
            unlock(self, AchievementId::PerfectStorm, &mut newly);
        }
        if state.total_notes > 0 && mods.contains(Modifier::Hidden) {
            unlock(self, AchievementId::InTheDark, &mut newly);
        }
        if state.total_notes > 0 && mods.contains(Modifier::Flashlight) {
            unlock(self, AchievementId::Searchlight, &mut newly);
        }
        if mods.contains(Modifier::SuddenDeath)
            && state.judgement_counts[3] == 0
            && state.total_notes > 0
        {
            unlock(self, AchievementId::OneShot, &mut newly);
        }
        if mods.contains(Modifier::PerfectOnly) && state.total_notes > 0 && full_combo {
            unlock(self, AchievementId::PixelPerfect, &mut newly);
        }

        newly
    }
}

/// Approximate ISO-8601 timestamp without pulling chrono — uses SystemTime.
fn chrono_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}
