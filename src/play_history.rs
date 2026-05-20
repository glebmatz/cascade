use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// One timing event captured during gameplay. These events are intentionally
/// compact and JSON-friendly so old runs can double as replay/ghost data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEvent {
    pub note_idx: usize,
    pub note_time_ms: u64,
    pub lane: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_time_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_ms: Option<i64>,
    pub judgement: String,
    pub kind: String,
}

/// One completed (non-practice) play.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayRecord {
    #[serde(default)]
    pub run_id: String,
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
    #[serde(default)]
    pub events: Vec<ReplayEvent>,
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

    pub fn find_run(&self, id: &str) -> Option<&PlayRecord> {
        self.plays.iter().find(|p| p.run_id == id)
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn make_run_id(ts: u64, ordinal: usize) -> String {
    format!("{:x}-{:04}", ts, ordinal)
}

#[derive(Debug, Clone, Default)]
pub struct RunBreakdown {
    pub hit_events: u32,
    pub misses: u32,
    pub avg_offset_ms: Option<f64>,
    pub early: u32,
    pub late: u32,
    pub on_time: u32,
    pub worst_lane: Option<(u8, u32)>,
    pub worst_section: Option<(u64, u32)>,
}

pub fn breakdown(events: &[ReplayEvent], duration_ms: u64) -> RunBreakdown {
    let mut out = RunBreakdown::default();
    let mut offset_sum = 0i64;
    let mut offset_count = 0u32;
    let mut lane_misses = [0u32; 5];

    let bucket_ms = 15_000u64;
    let bucket_count = ((duration_ms / bucket_ms) + 1).clamp(1, 256) as usize;
    let mut section_misses = vec![0u32; bucket_count];

    for ev in events {
        if ev.judgement.eq_ignore_ascii_case("miss") {
            out.misses += 1;
            if (ev.lane as usize) < lane_misses.len() {
                lane_misses[ev.lane as usize] += 1;
            }
            let idx = (ev.note_time_ms / bucket_ms).min(bucket_count as u64 - 1) as usize;
            section_misses[idx] += 1;
        }

        let Some(offset) = ev.offset_ms else {
            continue;
        };
        out.hit_events += 1;
        offset_sum += offset;
        offset_count += 1;
        if offset < -5 {
            out.early += 1;
        } else if offset > 5 {
            out.late += 1;
        } else {
            out.on_time += 1;
        }
    }

    if offset_count > 0 {
        out.avg_offset_ms = Some(offset_sum as f64 / offset_count as f64);
    }

    out.worst_lane = lane_misses
        .iter()
        .enumerate()
        .filter(|&(_, &count)| count > 0)
        .max_by_key(|&(_, &count)| count)
        .map(|(lane, &count)| (lane as u8, count));

    out.worst_section = section_misses
        .iter()
        .enumerate()
        .filter(|&(_, &count)| count > 0)
        .max_by_key(|&(_, &count)| count)
        .map(|(idx, &count)| (idx as u64 * bucket_ms, count));

    out
}
