//! Practice mode: loop a section with optional slowdown for learning.
//!
//! Two shapes of the same data:
//!
//! * [`PracticeSettings`] — the in-menu editable state (start / end may be
//!   `None` while the user is dialling it in).
//! * [`PracticeConfig`] — the resolved, clamped, game-ready form handed to
//!   [`crate::screens::gameplay::GameplayScreen`].

pub const SPEED_MIN: f32 = 0.25;
pub const SPEED_MAX: f32 = 2.0;
pub const SPEED_STEP: f32 = 0.05;

/// Resolved practice configuration used by the gameplay loop.
#[derive(Debug, Clone)]
pub struct PracticeConfig {
    /// Inclusive start of the section, in track-time milliseconds.
    pub section_start_ms: u64,
    /// Exclusive end of the section, in track-time milliseconds.
    /// Already clamped against the song's `duration_ms`.
    pub section_end_ms: u64,
    /// Playback speed multiplier. `1.0` = normal.
    pub speed: f32,
}

impl PracticeConfig {
    /// Start position to pass to `rodio`'s seek — wall-clock milliseconds.
    /// The sink speaks wall-clock time, so we pre-divide by speed.
    pub fn section_start_wallclock_ms(&self) -> u64 {
        (self.section_start_ms as f64 / self.speed.max(0.01) as f64) as u64
    }

    /// Compact badge, e.g. `"1:30→2:00 ×0.7"` or `"full ×0.7"`.
    pub fn badge(&self) -> String {
        let speed_part = format_speed(self.speed);
        let section_part = format!(
            "{}→{}",
            format_mmss(self.section_start_ms),
            format_mmss(self.section_end_ms),
        );
        match (has_section_override(self), (self.speed - 1.0).abs() > 0.001) {
            (true, true) => format!("{} {}", section_part, speed_part),
            (true, false) => section_part,
            (false, true) => format!("full {}", speed_part),
            (false, false) => String::from("full 1.0×"),
        }
    }
}

/// In-menu editable state. Defaults to an inactive configuration.
#[derive(Debug, Clone)]
pub struct PracticeSettings {
    pub section_start_ms: Option<u64>,
    pub section_end_ms: Option<u64>,
    pub speed: f32,
}

impl Default for PracticeSettings {
    fn default() -> Self {
        Self {
            section_start_ms: None,
            section_end_ms: None,
            speed: 1.0,
        }
    }
}

impl PracticeSettings {
    pub fn new() -> Self {
        Self::default()
    }

    /// True if either a section or a non-default speed is set.
    pub fn is_active(&self) -> bool {
        self.section_start_ms.is_some()
            || self.section_end_ms.is_some()
            || (self.speed - 1.0).abs() > 0.001
    }

    /// Resolve into a ready-to-use [`PracticeConfig`] against a song's
    /// duration. Returns `None` if:
    /// * practice is inactive, or
    /// * start is out of range, or
    /// * the clamped section would be empty.
    pub fn to_config(&self, duration_ms: u64) -> Option<PracticeConfig> {
        if !self.is_active() {
            return None;
        }
        let start = self.section_start_ms.unwrap_or(0);
        let end_unclamped = self.section_end_ms.unwrap_or(duration_ms);
        if start >= duration_ms {
            return None;
        }
        let end = end_unclamped.min(duration_ms);
        if end <= start {
            return None;
        }
        Some(PracticeConfig {
            section_start_ms: start,
            section_end_ms: end,
            speed: clamp_speed(self.speed),
        })
    }

    /// One-line badge for SongSelect, or `None` when inactive.
    pub fn badge(&self, duration_ms: u64) -> Option<String> {
        self.to_config(duration_ms).map(|c| c.badge())
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn step_speed(&mut self, delta: f32) {
        self.speed = clamp_speed(snap_speed(self.speed + delta));
    }
}

pub fn clamp_speed(v: f32) -> f32 {
    v.clamp(SPEED_MIN, SPEED_MAX)
}

/// Snap to the nearest 0.05 step so repeated `±` stays predictable.
pub fn snap_speed(v: f32) -> f32 {
    (v / SPEED_STEP).round() * SPEED_STEP
}

/// Whether this config actually constrains the section (vs full song).
/// A section equal to `0..duration_ms` is treated as "no override" only if
/// the caller sets it that way — the config itself doesn't know duration, so
/// we conservatively assume an explicit start or an explicit non-zero start.
fn has_section_override(c: &PracticeConfig) -> bool {
    c.section_start_ms != 0
}

pub fn format_mmss(ms: u64) -> String {
    let total_secs = ms / 1000;
    format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

pub fn format_speed(speed: f32) -> String {
    // Show two decimals unless the value is effectively an integer.
    if (speed - speed.round()).abs() < 0.001 {
        format!("×{:.1}", speed)
    } else {
        format!("×{:.2}", speed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_by_default() {
        let s = PracticeSettings::default();
        assert!(!s.is_active());
        assert!(s.to_config(180_000).is_none());
    }

    #[test]
    fn active_if_section_set() {
        let s = PracticeSettings {
            section_start_ms: Some(90_000),
            section_end_ms: Some(120_000),
            speed: 1.0,
        };
        assert!(s.is_active());
        let c = s.to_config(180_000).unwrap();
        assert_eq!(c.section_start_ms, 90_000);
        assert_eq!(c.section_end_ms, 120_000);
        assert!((c.speed - 1.0).abs() < 0.001);
    }

    #[test]
    fn active_if_speed_nondefault() {
        let s = PracticeSettings {
            section_start_ms: None,
            section_end_ms: None,
            speed: 0.7,
        };
        assert!(s.is_active());
        let c = s.to_config(180_000).unwrap();
        assert_eq!(c.section_start_ms, 0);
        assert_eq!(c.section_end_ms, 180_000);
    }

    #[test]
    fn clamps_end_to_duration() {
        let s = PracticeSettings {
            section_start_ms: Some(90_000),
            section_end_ms: Some(999_999),
            speed: 1.0,
        };
        let c = s.to_config(180_000).unwrap();
        assert_eq!(c.section_end_ms, 180_000);
    }

    #[test]
    fn rejects_start_past_duration() {
        let s = PracticeSettings {
            section_start_ms: Some(200_000),
            section_end_ms: Some(220_000),
            speed: 1.0,
        };
        assert!(s.to_config(180_000).is_none());
    }

    #[test]
    fn rejects_empty_section() {
        let s = PracticeSettings {
            section_start_ms: Some(120_000),
            section_end_ms: Some(120_000),
            speed: 1.0,
        };
        assert!(s.to_config(180_000).is_none());
    }

    #[test]
    fn speed_is_clamped() {
        let s = PracticeSettings {
            section_start_ms: None,
            section_end_ms: None,
            speed: 10.0,
        };
        assert_eq!(s.to_config(180_000).unwrap().speed, SPEED_MAX);
        let s = PracticeSettings {
            section_start_ms: None,
            section_end_ms: None,
            speed: 0.01,
        };
        assert_eq!(s.to_config(180_000).unwrap().speed, SPEED_MIN);
    }

    #[test]
    fn step_speed_snaps_and_clamps() {
        let mut s = PracticeSettings::default();
        s.step_speed(-0.05);
        assert!((s.speed - 0.95).abs() < 0.001);
        for _ in 0..100 {
            s.step_speed(-0.05);
        }
        assert!((s.speed - SPEED_MIN).abs() < 0.001);
        for _ in 0..200 {
            s.step_speed(0.05);
        }
        assert!((s.speed - SPEED_MAX).abs() < 0.001);
    }

    #[test]
    fn badge_formats_cleanly() {
        let c = PracticeConfig {
            section_start_ms: 90_000,
            section_end_ms: 120_000,
            speed: 0.7,
        };
        assert_eq!(c.badge(), "1:30→2:00 ×0.70");
        let c = PracticeConfig {
            section_start_ms: 90_000,
            section_end_ms: 120_000,
            speed: 1.0,
        };
        assert_eq!(c.badge(), "1:30→2:00");
        let c = PracticeConfig {
            section_start_ms: 0,
            section_end_ms: 180_000,
            speed: 0.8,
        };
        assert_eq!(c.badge(), "full ×0.80");
    }

    #[test]
    fn wallclock_start_divides_by_speed() {
        let c = PracticeConfig {
            section_start_ms: 1_000,
            section_end_ms: 2_000,
            speed: 0.5,
        };
        assert_eq!(c.section_start_wallclock_ms(), 2_000);
        let c = PracticeConfig {
            section_start_ms: 1_000,
            section_end_ms: 2_000,
            speed: 1.0,
        };
        assert_eq!(c.section_start_wallclock_ms(), 1_000);
    }

    #[test]
    fn format_mmss_basic() {
        assert_eq!(format_mmss(0), "0:00");
        assert_eq!(format_mmss(90_000), "1:30");
        assert_eq!(format_mmss(3_599_000), "59:59");
    }
}
