use crate::achievements::{AchievementId, AchievementStore};
use crate::play_history::PlayHistory;
use std::collections::HashMap;

pub const HEATMAP_DAYS: usize = 30;

#[derive(Debug, Clone)]
pub struct StatsSummary {
    pub total_plays: u32,
    pub total_time_played_ms: u64,
    pub total_notes_hit: u64,
    pub top_songs: Vec<TopSong>,
    pub per_difficulty: Vec<DiffRow>,
    pub accuracy_30d: [Option<f64>; HEATMAP_DAYS],
    pub heatmap_30d: [u32; HEATMAP_DAYS],
    pub achievements_unlocked: usize,
    pub achievements_total: usize,
}

#[derive(Debug, Clone)]
pub struct TopSong {
    pub slug: String,
    pub title: String,
    pub plays: u32,
}

#[derive(Debug, Clone)]
pub struct DiffRow {
    pub difficulty: String,
    pub plays: u32,
    pub best_accuracy: f64,
    pub best_score: u64,
    pub avg_accuracy: f64,
}

const DIFFICULTY_ORDER: &[&str] = &["Easy", "Medium", "Hard", "Expert"];
const SECONDS_PER_DAY: u64 = 86_400;

pub fn summarize(
    history: &PlayHistory,
    achievements: &AchievementStore,
    now_unix: u64,
) -> StatsSummary {
    let plays = &history.plays;

    let total_plays = plays.len() as u32;
    let total_time_played_ms: u64 = plays.iter().map(|p| p.duration_played_ms).sum();
    // Perfect + Great + Good — misses excluded.
    let total_notes_hit: u64 = plays
        .iter()
        .map(|p| (p.judgements[0] + p.judgements[1] + p.judgements[2]) as u64)
        .sum();

    let top_songs = compute_top_songs(plays, 5);
    let per_difficulty = compute_per_difficulty(plays);
    let (accuracy_30d, heatmap_30d) = compute_30day_windows(plays, now_unix);

    let achievements_unlocked = AchievementId::ALL
        .iter()
        .filter(|id| achievements.is_unlocked(**id))
        .count();
    let achievements_total = AchievementId::ALL.len();

    StatsSummary {
        total_plays,
        total_time_played_ms,
        total_notes_hit,
        top_songs,
        per_difficulty,
        accuracy_30d,
        heatmap_30d,
        achievements_unlocked,
        achievements_total,
    }
}

fn compute_top_songs(plays: &[crate::play_history::PlayRecord], limit: usize) -> Vec<TopSong> {
    struct Entry {
        title: String,
        latest_ts: u64,
        plays: u32,
    }
    let mut by_slug: HashMap<String, Entry> = HashMap::new();
    for p in plays {
        let e = by_slug.entry(p.slug.clone()).or_insert(Entry {
            title: p.title.clone(),
            latest_ts: 0,
            plays: 0,
        });
        e.plays += 1;
        if p.ts >= e.latest_ts {
            e.latest_ts = p.ts;
            // Prefer non-empty title; fall back otherwise.
            if !p.title.is_empty() {
                e.title = p.title.clone();
            }
        }
    }
    let mut out: Vec<TopSong> = by_slug
        .into_iter()
        .map(|(slug, e)| TopSong {
            slug,
            title: e.title,
            plays: e.plays,
        })
        .collect();
    // Sort by plays DESC, ties by slug ASC (deterministic).
    out.sort_by(|a, b| b.plays.cmp(&a.plays).then_with(|| a.slug.cmp(&b.slug)));
    out.truncate(limit);
    out
}

fn compute_per_difficulty(plays: &[crate::play_history::PlayRecord]) -> Vec<DiffRow> {
    struct Agg {
        plays: u32,
        best_accuracy: f64,
        best_score: u64,
        acc_sum: f64,
    }
    let mut map: HashMap<String, Agg> = HashMap::new();
    for p in plays {
        let key = normalize_difficulty(&p.difficulty);
        let a = map.entry(key).or_insert(Agg {
            plays: 0,
            best_accuracy: 0.0,
            best_score: 0,
            acc_sum: 0.0,
        });
        a.plays += 1;
        a.acc_sum += p.accuracy;
        if p.accuracy > a.best_accuracy {
            a.best_accuracy = p.accuracy;
        }
        if p.score > a.best_score {
            a.best_score = p.score;
        }
    }

    let mut out = Vec::new();
    for label in DIFFICULTY_ORDER {
        if let Some(a) = map.remove(*label) {
            let avg = if a.plays > 0 {
                a.acc_sum / a.plays as f64
            } else {
                0.0
            };
            out.push(DiffRow {
                difficulty: label.to_string(),
                plays: a.plays,
                best_accuracy: a.best_accuracy,
                best_score: a.best_score,
                avg_accuracy: avg,
            });
        }
    }
    // Any exotic/unknown labels that didn't match — append alphabetically.
    let mut leftovers: Vec<_> = map.into_iter().collect();
    leftovers.sort_by(|a, b| a.0.cmp(&b.0));
    for (label, a) in leftovers {
        let avg = if a.plays > 0 {
            a.acc_sum / a.plays as f64
        } else {
            0.0
        };
        out.push(DiffRow {
            difficulty: label,
            plays: a.plays,
            best_accuracy: a.best_accuracy,
            best_score: a.best_score,
            avg_accuracy: avg,
        });
    }
    out
}

fn compute_30day_windows(
    plays: &[crate::play_history::PlayRecord],
    now_unix: u64,
) -> ([Option<f64>; HEATMAP_DAYS], [u32; HEATMAP_DAYS]) {
    let today_day = now_unix / SECONDS_PER_DAY;
    // Bucket index 0..29 where 29 = today, 0 = 29 days ago.
    let first_day = today_day.saturating_sub((HEATMAP_DAYS as u64) - 1);

    let mut counts = [0u32; HEATMAP_DAYS];
    let mut sums = [0.0f64; HEATMAP_DAYS];

    for p in plays {
        let day = p.ts / SECONDS_PER_DAY;
        if day < first_day || day > today_day {
            continue;
        }
        let idx = (day - first_day) as usize;
        if idx < HEATMAP_DAYS {
            counts[idx] += 1;
            sums[idx] += p.accuracy;
        }
    }

    let mut accuracy: [Option<f64>; HEATMAP_DAYS] = [None; HEATMAP_DAYS];
    for i in 0..HEATMAP_DAYS {
        if counts[i] > 0 {
            accuracy[i] = Some(sums[i] / counts[i] as f64);
        }
    }
    (accuracy, counts)
}

fn normalize_difficulty(s: &str) -> String {
    // Title-case: first char upper, rest lower. Works for Easy/Medium/Hard/Expert.
    let mut out = String::with_capacity(s.len());
    for (i, ch) in s.chars().enumerate() {
        if i == 0 {
            for u in ch.to_uppercase() {
                out.push(u);
            }
        } else {
            for l in ch.to_lowercase() {
                out.push(l);
            }
        }
    }
    out
}

/// Render a 30-char unicode block sparkline from a 30-day accuracy window.
/// Empty days render as a space. Scale is local: min → lowest glyph,
/// max → highest glyph.
pub fn sparkline_30d(data: &[Option<f64>; HEATMAP_DAYS]) -> String {
    const GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let values: Vec<f64> = data.iter().filter_map(|v| *v).collect();
    if values.is_empty() {
        return " ".repeat(HEATMAP_DAYS);
    }
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).max(f64::EPSILON);
    let mut out = String::with_capacity(HEATMAP_DAYS);
    for v in data.iter() {
        match v {
            None => out.push(' '),
            Some(val) => {
                let t = ((val - min) / span).clamp(0.0, 1.0);
                let idx =
                    ((t * (GLYPHS.len() as f64 - 1.0)).round() as usize).min(GLYPHS.len() - 1);
                out.push(GLYPHS[idx]);
            }
        }
    }
    out
}

/// Map a 30-day play-count heatmap to five density glyphs.
/// Zero-plays → `·`; non-zero days are split into four quartiles.
pub fn heatmap_glyphs(counts: &[u32; HEATMAP_DAYS]) -> String {
    const GLYPHS: [char; 5] = ['·', '░', '▒', '▓', '█'];
    let max = *counts.iter().max().unwrap_or(&0);
    if max == 0 {
        return std::iter::repeat_n(GLYPHS[0], HEATMAP_DAYS).collect();
    }
    let mut out = String::with_capacity(HEATMAP_DAYS);
    for &c in counts {
        let idx = if c == 0 {
            0
        } else {
            // 1..=max → 1..=4
            let t = c as f64 / max as f64;
            ((t * 4.0).ceil() as usize).clamp(1, 4)
        };
        out.push(GLYPHS[idx]);
    }
    out
}

pub fn format_duration_ms(ms: u64) -> String {
    let total_sec = ms / 1000;
    let hours = total_sec / 3600;
    let minutes = (total_sec % 3600) / 60;
    let seconds = total_sec % 60;
    if hours > 0 {
        format!("{}h {:02}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {:02}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}
