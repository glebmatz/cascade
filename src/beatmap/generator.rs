use crate::beatmap::types::{Beatmap, Note, Difficulty, SongMeta};
use rustfft::{FftPlanner, num_complex::Complex};

pub struct OnsetInfo {
    pub time_ms: u64,
    pub strength: f32,
    pub freq_band: u8,
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

    // Peak picking
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

pub fn place_notes(onsets: &[OnsetInfo], difficulty: Difficulty) -> Vec<Note> {
    let (threshold, min_gap_ms) = match difficulty {
        Difficulty::Easy => (0.65, 600u64),
        Difficulty::Medium => (0.45, 400),
        Difficulty::Hard => (0.35, 280),
        Difficulty::Expert => (0.2, 160),
    };

    let mut notes = Vec::new();
    let mut last_time: Option<u64> = None;
    let mut last_lane: Option<u8> = None;

    for onset in onsets {
        if onset.strength < threshold { continue; }
        if let Some(lt) = last_time {
            if onset.time_ms.saturating_sub(lt) < min_gap_ms { continue; }
        }

        let lane = freq_band_to_lane(onset.freq_band, last_lane);

        // Determine if this should be a hold note:
        // Strong onsets with a big gap before next onset become holds
        let duration_ms = if onset.strength > 0.6 {
            // Find time to next onset
            let next_onset_time = onsets.iter()
                .find(|o| o.time_ms > onset.time_ms + 100)
                .map(|o| o.time_ms);

            if let Some(next_t) = next_onset_time {
                let gap = next_t - onset.time_ms;
                if gap > min_gap_ms * 2 {
                    // Hold for ~60% of the gap, capped at 2 seconds
                    ((gap as f64 * 0.6) as u64).min(2000).max(200)
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        notes.push(Note { time_ms: onset.time_ms, lane, duration_ms });
        let end_time = onset.time_ms + duration_ms;
        last_time = Some(end_time.max(onset.time_ms));
        last_lane = Some(lane);
    }

    notes
}

fn freq_band_to_lane(freq_band: u8, last_lane: Option<u8>) -> u8 {
    let base_lanes: &[u8] = match freq_band {
        0 => &[0, 1],
        1 => &[2],
        _ => &[3, 4],
    };

    let mut best = base_lanes[0];
    if let Some(prev) = last_lane {
        let mut min_jump = u8::MAX;
        for &lane in base_lanes {
            let jump = (lane as i8 - prev as i8).unsigned_abs();
            if jump < min_jump {
                min_jump = jump;
                best = lane;
            }
        }
    }

    best
}

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
