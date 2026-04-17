use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Modifier {
    Hidden,
    Flashlight,
    SuddenDeath,
    PerfectOnly,
}

impl Modifier {
    #[allow(dead_code)]
    pub const ALL: &'static [Modifier] = &[
        Modifier::Hidden,
        Modifier::Flashlight,
        Modifier::SuddenDeath,
        Modifier::PerfectOnly,
    ];

    pub fn short_code(self) -> &'static str {
        match self {
            Modifier::Hidden => "hd",
            Modifier::Flashlight => "fl",
            Modifier::SuddenDeath => "sd",
            Modifier::PerfectOnly => "po",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "hd" | "hidden" => Some(Modifier::Hidden),
            "fl" | "flashlight" => Some(Modifier::Flashlight),
            "sd" | "sudden-death" | "sudden_death" => Some(Modifier::SuddenDeath),
            "po" | "perfect-only" | "perfect_only" => Some(Modifier::PerfectOnly),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Modifier::Hidden => "Hidden",
            Modifier::Flashlight => "Flashlight",
            Modifier::SuddenDeath => "Sudden Death",
            Modifier::PerfectOnly => "Perfect Only",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Modifier::Hidden => "Notes only appear close to the hit zone.",
            Modifier::Flashlight => "Highway is dark except a narrow band around the hit zone.",
            Modifier::SuddenDeath => "First miss ends the run.",
            Modifier::PerfectOnly => "Anything below Perfect counts as a Miss.",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mods {
    set: BTreeSet<Modifier>,
}

impl Mods {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn collect_from<I: IntoIterator<Item = Modifier>>(it: I) -> Self {
        Self {
            set: it.into_iter().collect(),
        }
    }

    pub fn contains(&self, m: Modifier) -> bool {
        self.set.contains(&m)
    }

    pub fn toggle(&mut self, m: Modifier) {
        if !self.set.remove(&m) {
            self.set.insert(m);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = Modifier> + '_ {
        self.set.iter().copied()
    }

    pub fn from_codes(s: &str) -> Self {
        Self {
            set: s
                .split(',')
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
                .filter_map(Modifier::from_code)
                .collect(),
        }
    }

    /// Stable string used as part of score storage key. Empty when no mods.
    pub fn storage_key(&self) -> String {
        self.set
            .iter()
            .map(|m| m.short_code())
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Compact display like `[HD+SD]`. Empty when no mods.
    pub fn badge(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let codes: Vec<String> = self
            .set
            .iter()
            .map(|m| m.short_code().to_uppercase())
            .collect();
        format!("[{}]", codes.join("+"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codes() {
        let m = Mods::from_codes("hd,fl,unknown,sd");
        assert!(m.contains(Modifier::Hidden));
        assert!(m.contains(Modifier::Flashlight));
        assert!(m.contains(Modifier::SuddenDeath));
        assert!(!m.contains(Modifier::PerfectOnly));
    }

    #[test]
    fn storage_key_is_stable_alphabetical() {
        let a = Mods::from_codes("sd,hd");
        let b = Mods::from_codes("hd,sd");
        assert_eq!(a.storage_key(), b.storage_key());
        assert_eq!(a.storage_key(), "hd+sd");
    }

    #[test]
    fn empty_storage_key() {
        assert_eq!(Mods::new().storage_key(), "");
    }

    #[test]
    fn toggle_adds_then_removes() {
        let mut m = Mods::new();
        m.toggle(Modifier::Hidden);
        assert!(m.contains(Modifier::Hidden));
        m.toggle(Modifier::Hidden);
        assert!(!m.contains(Modifier::Hidden));
    }
}
