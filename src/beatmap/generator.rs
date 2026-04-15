use crate::beatmap::types::{Beatmap, Note, Difficulty, SongMeta};
use rustfft::{FftPlanner, num_complex::Complex};

pub struct OnsetInfo {
    pub time_ms: u64,
    pub strength: f32,
    pub freq_band: u8, // 0=low, 1=mid, 2=high
}

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = 512;

pub fn detect_onsets(samples: &[f32], sample_rate: u32) -> Vec<OnsetInfo> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let mut prev_magnitudes = vec![0.0f32; FFT_SIZE / 2];
    let mut onsets = Vec::new();

    let mut pos = 0;
    while pos + FFT_SIZE <= samples.len() {
        let mut buffer: Vec<Complex<f32>> = samples[pos..pos + FFT_SIZE]
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos());
                Complex::new(s * window, 0.0)
            })
            .collect();

        fft.process(&mut buffer);

        let magnitudes: Vec<f32> = buffer[..FFT_SIZE / 2].iter().map(|c| c.norm()).collect();

        let mut flux_low = 0.0f32;
        let mut flux_mid = 0.0f32;
        let mut flux_high = 0.0f32;

        let bin_freq = sample_rate as f32 / FFT_SIZE as f32;
        for (i, (curr, prev)) in magnitudes.iter().zip(prev_magnitudes.iter()).enumerate() {
            let diff = (curr - prev).max(0.0);
            let freq = i as f32 * bin_freq;
            if freq < 300.0 { flux_low += diff; }
            else if freq < 2000.0 { flux_mid += diff; }
            else { flux_high += diff; }
        }

        let total_flux = flux_low + flux_mid + flux_high;

        let freq_band = if flux_low >= flux_mid && flux_low >= flux_high { 0 }
        else if flux_mid >= flux_high { 1 }
        else { 2 };

        let time_ms = (pos as f64 / sample_rate as f64 * 1000.0) as u64;

        if total_flux > 0.0 {
            onsets.push(OnsetInfo { time_ms, strength: total_flux, freq_band });
        }

        prev_magnitudes = magnitudes;
        pos += HOP_SIZE;
    }

    // Normalize
    let max_strength = onsets.iter().map(|o| o.strength).fold(0.0f32, f32::max);
    if max_strength > 0.0 {
        for onset in &mut onsets {
            onset.strength /= max_strength;
        }
    }

    // Peak picking — only keep local maxima
    let mut peaks = Vec::new();
    for i in 1..onsets.len().saturating_sub(1) {
        if onsets[i].strength > onsets[i - 1].strength
            && onsets[i].strength > onsets[i + 1].strength
            && onsets[i].strength > 0.1
        {
            peaks.push(OnsetInfo {
                time_ms: onsets[i].time_ms,
                strength: onsets[i].strength,
                freq_band: onsets[i].freq_band,
            });
        }
    }

    peaks
}

/// Detect BPM from onset times using autocorrelation.
pub fn detect_bpm(onsets: &[OnsetInfo]) -> u32 {
    if onsets.len() < 4 { return 120; }

    let intervals: Vec<u64> = onsets.windows(2)
        .map(|w| w[1].time_ms - w[0].time_ms)
        .filter(|&i| i > 200 && i < 2000)
        .collect();

    if intervals.is_empty() { return 120; }

    let mut best_interval = intervals[0];
    let mut best_count = 0;

    for &interval in &intervals {
        let count = intervals.iter()
            .filter(|&&i| (i as i64 - interval as i64).unsigned_abs() < 30)
            .count();
        if count > best_count {
            best_count = count;
            best_interval = interval;
        }
    }

    (60000.0 / best_interval as f64).round().clamp(60.0, 300.0) as u32
}

/// Quantize a timestamp to the nearest beat grid position.
fn quantize_to_grid(time_ms: u64, beat_ms: f64, grid_division: u32) -> u64 {
    let grid_ms = beat_ms / grid_division as f64;
    let grid_pos = (time_ms as f64 / grid_ms).round();
    (grid_pos * grid_ms) as u64
}

/// Main note placement with patterns, variety, and musical awareness.
pub fn place_notes(onsets: &[OnsetInfo], difficulty: Difficulty) -> Vec<Note> {
    let (threshold, min_gap_ms, grid_div, hold_chance) = match difficulty {
        Difficulty::Easy =>   (0.60, 550u64, 2, 0.15),  // half notes
        Difficulty::Medium => (0.42, 380,    4, 0.12),   // quarter notes
        Difficulty::Hard =>   (0.30, 250,    4, 0.10),   // quarter notes, more notes
        Difficulty::Expert => (0.18, 150,    8, 0.08),   // eighth notes
    };

    // Detect BPM for grid quantization
    let bpm = detect_bpm(onsets);
    let beat_ms = 60000.0 / bpm as f64;

    // Filter onsets by strength threshold
    let filtered: Vec<&OnsetInfo> = onsets.iter()
        .filter(|o| o.strength >= threshold)
        .collect();

    let mut notes: Vec<Note> = Vec::new();
    let mut last_time: Option<u64> = None;
    let mut lane_history: Vec<u8> = Vec::new(); // last N lanes for pattern detection

    // Lane patterns for variety — cycle through these
    let patterns: &[&[u8]] = &[
        &[0, 2, 4, 2],       // zigzag wide
        &[1, 2, 3, 2],       // zigzag center
        &[0, 1, 3, 4],       // left-to-right
        &[4, 3, 1, 0],       // right-to-left
        &[0, 4, 1, 3],       // alternating wide
        &[2, 0, 2, 4],       // center bounce
    ];
    let mut pattern_idx = 0;
    let mut pattern_pos = 0;

    for onset in &filtered {
        // Enforce minimum gap
        if let Some(lt) = last_time {
            if onset.time_ms.saturating_sub(lt) < min_gap_ms {
                continue;
            }
        }

        // Quantize to beat grid for musical feel
        let quantized_time = quantize_to_grid(onset.time_ms, beat_ms, grid_div);

        // Skip if quantized time is too close to last note
        if let Some(lt) = last_time {
            if quantized_time.saturating_sub(lt) < min_gap_ms / 2 {
                continue;
            }
        }

        // Lane assignment: blend frequency band hint with pattern
        let lane = pick_lane(onset.freq_band, &lane_history, patterns[pattern_idx], pattern_pos);

        // Determine hold note duration
        let duration_ms = if onset.strength > 0.55 {
            let next_time = filtered.iter()
                .find(|o| o.time_ms > onset.time_ms + 100)
                .map(|o| o.time_ms);

            if let Some(next_t) = next_time {
                let gap = next_t - onset.time_ms;
                // Use hold_chance as a pseudo-random threshold based on time
                let pseudo_rand = (onset.time_ms % 100) as f64 / 100.0;
                if gap > min_gap_ms * 3 && pseudo_rand < hold_chance * 3.0 {
                    // Hold for portion of gap, quantized to grid
                    let raw_dur = ((gap as f64 * 0.5) as u64).min(1500).max(200);
                    quantize_to_grid(raw_dur, beat_ms, grid_div).max(200)
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        notes.push(Note {
            time_ms: quantized_time,
            lane,
            duration_ms,
        });

        last_time = Some(quantized_time + duration_ms);
        lane_history.push(lane);
        if lane_history.len() > 8 { lane_history.remove(0); }

        pattern_pos += 1;
        if pattern_pos >= patterns[pattern_idx].len() {
            pattern_pos = 0;
            pattern_idx = (pattern_idx + 1) % patterns.len();
        }
    }

    notes
}

/// Pick a lane using frequency hints and patterns, ensuring variety.
fn pick_lane(freq_band: u8, history: &[u8], pattern: &[u8], pattern_pos: usize) -> u8 {
    // Frequency band gives a region preference
    let freq_region: (u8, u8) = match freq_band {
        0 => (0, 1),   // low → left side
        1 => (1, 3),   // mid → center
        _ => (3, 4),   // high → right side
    };

    // Pattern gives the ideal lane
    let pattern_lane = pattern[pattern_pos % pattern.len()];

    // Blend: if pattern lane is within freq region range (+/-1), use it.
    // Otherwise, use freq region with pattern-based offset.
    let candidate = if pattern_lane >= freq_region.0.saturating_sub(1)
        && pattern_lane <= freq_region.1 + 1
    {
        pattern_lane
    } else {
        // Map pattern position to freq region
        let range = freq_region.1 - freq_region.0 + 1;
        freq_region.0 + (pattern_pos as u8 % range)
    };

    // Avoid repeating last lane
    if let Some(&last) = history.last() {
        if candidate == last {
            // Shift direction based on position in pattern
            let shift: i8 = if pattern_pos % 2 == 0 { 1 } else { -1 };
            let alt = (candidate as i8 + shift).clamp(0, 4) as u8;
            if alt != last { return alt; }
            let alt2 = (candidate as i8 - shift).clamp(0, 4) as u8;
            if alt2 != last { return alt2; }
        }

        // Avoid repeating same lane 3 times in a row
        if history.len() >= 2 && history[history.len() - 2] == last && last == candidate {
            return (candidate + 2) % 5;
        }
    }

    candidate
}

pub fn generate_all_beatmaps(
    samples: &[f32],
    sample_rate: u32,
    song_meta: SongMeta,
) -> Vec<Beatmap> {
    let onsets = detect_onsets(samples, sample_rate);
    let bpm = detect_bpm(&onsets);

    let mut meta = song_meta;
    meta.bpm = bpm;

    Difficulty::all()
        .iter()
        .map(|&diff| {
            let notes = place_notes(&onsets, diff);
            Beatmap {
                version: 1,
                song: meta.clone(),
                difficulty: diff,
                notes,
            }
        })
        .collect()
}
