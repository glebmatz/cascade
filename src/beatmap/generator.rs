use crate::beatmap::types::{Beatmap, Difficulty, Note, SongMeta};
use rustfft::{FftPlanner, num_complex::Complex};

pub const NUM_BANDS: usize = 8;
const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = 512;

#[derive(Clone, Copy)]
pub struct NoveltyFrame {
    pub time_ms: f64,
    pub novelty: f32,
    pub band_fluxes: [f32; NUM_BANDS],
    pub band_energy: [f32; NUM_BANDS],
    pub dominant_band: u8,
}

#[derive(Clone, Copy)]
pub struct Onset {
    pub time_ms: u64,
    pub strength: f32,
    pub dominant_band: u8,
    pub band_fluxes: [f32; NUM_BANDS],
    pub frame_idx: usize,
    /// Coarse melodic lane hint (0..=4), derived from the spectral centroid of
    /// mid/high bands. Only meaningful when `melodic` is true; otherwise falls
    /// back to the percussive lane mapping in [`pick_lane`].
    pub pitch_lane: u8,
    /// True when the onset's spectral flux is weighted toward mid/high bands —
    /// i.e. a melodic event rather than a drum hit.
    pub melodic: bool,
    /// Centered ~1 s running mean of whitened band energies at this onset, in
    /// `[0..=1]`. Drives density/chord/slide decisions so loud sections get
    /// more notes than quiet ones regardless of their flux magnitude.
    pub local_rms: f32,
}

pub fn generate_all_beatmaps(
    samples: &[f32],
    sample_rate: u32,
    song_meta: SongMeta,
) -> Vec<Beatmap> {
    let frames = compute_novelty(samples, sample_rate);
    let bpm = detect_bpm(&frames);
    let phase = detect_phase(&frames, bpm);
    let onsets = pick_peaks(&frames, 80);

    let mut meta = song_meta;
    meta.bpm = bpm;

    Difficulty::all()
        .iter()
        .map(|&diff| Beatmap {
            version: 1,
            song: meta.clone(),
            difficulty: diff,
            notes: place_notes(&onsets, &frames, bpm, phase, diff),
        })
        .collect()
}

pub fn compute_novelty(samples: &[f32], sample_rate: u32) -> Vec<NoveltyFrame> {
    if samples.len() < FFT_SIZE {
        return Vec::new();
    }

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos()))
        .collect();

    let band_ranges = log_band_ranges(sample_rate);

    // Prime `running_max` on the first ~3.5s so the intro doesn't ride a cold
    // start (previously `running_max = 1e-3` caused the first 5–10 s to look
    // abnormally novel). We replay these frames, discarding output.
    let mut band_running_max = [1e-3_f32; NUM_BANDS];
    let mut buffer: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    const PRIME_FRAMES: usize = 300;
    let prime_end_sample = (PRIME_FRAMES * HOP_SIZE).min(samples.len().saturating_sub(FFT_SIZE));
    let mut pos = 0;
    while pos + FFT_SIZE <= samples.len() && pos < prime_end_sample {
        fill_windowed(&mut buffer, samples, pos, &window);
        fft.process(&mut buffer);
        let _ = whitened_band_energy(&buffer, &band_ranges, &mut band_running_max);
        pos += HOP_SIZE;
    }

    // Main pass.
    let mut prev_band_energy = [0.0_f32; NUM_BANDS];
    let mut frames = Vec::with_capacity(samples.len() / HOP_SIZE);
    pos = 0;
    while pos + FFT_SIZE <= samples.len() {
        fill_windowed(&mut buffer, samples, pos, &window);
        fft.process(&mut buffer);

        let band_energy = whitened_band_energy(&buffer, &band_ranges, &mut band_running_max);
        let (novelty, band_fluxes, dominant_band) = band_flux(&band_energy, &prev_band_energy);
        prev_band_energy = band_energy;

        frames.push(NoveltyFrame {
            time_ms: pos as f64 / sample_rate as f64 * 1000.0,
            novelty,
            band_fluxes,
            band_energy,
            dominant_band,
        });
        pos += HOP_SIZE;
    }

    frames
}

fn fill_windowed(buffer: &mut [Complex<f32>], samples: &[f32], pos: usize, window: &[f32]) {
    for i in 0..FFT_SIZE {
        buffer[i] = Complex::new(samples[pos + i] * window[i], 0.0);
    }
}

fn log_band_ranges(sample_rate: u32) -> [(usize, usize); NUM_BANDS] {
    let log_min = 30.0_f32.ln();
    let log_max = (sample_rate as f32 / 2.0).min(16_000.0).ln();
    let bins_per_hz = FFT_SIZE as f32 / sample_rate as f32;
    let mut ranges = [(0usize, 0usize); NUM_BANDS];
    for i in 0..NUM_BANDS {
        let lo = (log_min + (log_max - log_min) * i as f32 / NUM_BANDS as f32).exp();
        let hi = (log_min + (log_max - log_min) * (i + 1) as f32 / NUM_BANDS as f32).exp();
        let lo_bin = (lo * bins_per_hz) as usize;
        let hi_bin = (hi * bins_per_hz) as usize;
        ranges[i] = (lo_bin.max(1), hi_bin.max(lo_bin + 1).min(FFT_SIZE / 2));
    }
    ranges
}

fn whitened_band_energy(
    spectrum: &[Complex<f32>],
    band_ranges: &[(usize, usize); NUM_BANDS],
    running_max: &mut [f32; NUM_BANDS],
) -> [f32; NUM_BANDS] {
    // Half-life ≈ 1.6 s. 0.98 (old) was too fast — long loud passages like
    // choruses saturated running_max within ~400 ms and suppressed flux, so
    // the system saw climactic sections as less novel than verses.
    const DECAY: f32 = 0.995;
    let mut out = [0.0_f32; NUM_BANDS];
    for (b, &(lo, hi)) in band_ranges.iter().enumerate() {
        let sum: f32 = spectrum[lo..hi].iter().map(|c| (1.0 + c.norm()).ln()).sum();
        let avg = sum / (hi - lo).max(1) as f32;
        running_max[b] = (running_max[b] * DECAY).max(avg);
        out[b] = avg / running_max[b].max(1e-3);
    }
    out
}

fn band_flux(
    energy: &[f32; NUM_BANDS],
    prev_energy: &[f32; NUM_BANDS],
) -> (f32, [f32; NUM_BANDS], u8) {
    let mut total = 0.0_f32;
    let mut fluxes = [0.0_f32; NUM_BANDS];
    let mut best_band = 0u8;
    let mut best_flux = 0.0_f32;
    for b in 0..NUM_BANDS {
        let diff = (energy[b] - prev_energy[b]).max(0.0);
        fluxes[b] = diff;
        if diff > best_flux {
            best_flux = diff;
            best_band = b as u8;
        }
        total += diff;
    }
    (total, fluxes, best_band)
}

pub fn pick_peaks(frames: &[NoveltyFrame], min_gap_ms: u64) -> Vec<Onset> {
    if frames.len() < 9 {
        return Vec::new();
    }

    // Centered ~1 s loudness envelope (mean of whitened band energies), then
    // percentile-normalised across the whole song so `local_rms` spans a
    // meaningful `[0..=1]` range — a track that never drops below ~0.85 raw
    // still produces quiet verses near 0 and loud choruses near 1, which is
    // what drives the density/chord/slide gating.
    let local_rms = normalize_by_percentile(&compute_local_rms(frames));

    let mut sorted: Vec<f32> = frames.iter().map(|f| f.novelty).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = sorted[(sorted.len() as f32 * 0.95) as usize].max(1e-6);
    let novelty: Vec<f32> = frames.iter().map(|f| (f.novelty / p95).min(1.0)).collect();

    const W: usize = 4;
    const STATS_W: usize = 17;
    let mut peaks: Vec<Onset> = Vec::new();
    let mut last_peak_ms: Option<u64> = None;

    for i in W..novelty.len().saturating_sub(W) {
        let c = novelty[i];
        if c < 0.05 {
            continue;
        }
        if !is_local_max(&novelty, i, W) {
            continue;
        }
        // Intensity-weighted threshold: quiet sections raise the bar (drop
        // peaks), loud sections lower it (keep more peaks). This actually
        // redistributes density — multiplying the novelty itself is
        // self-cancelling because `adaptive_threshold` also rescales with it.
        let rms = local_rms[i];
        let thresh_mul = (1.5 - 0.9 * rms).clamp(0.7, 1.5);
        let effective_thresh = adaptive_threshold(&novelty, i, STATS_W) * thresh_mul;
        if c < effective_thresh {
            continue;
        }

        let (pitch_lane, melodic) = classify_pitch(&frames[i].band_fluxes);
        let onset = Onset {
            time_ms: frames[i].time_ms as u64,
            strength: c,
            dominant_band: frames[i].dominant_band,
            band_fluxes: frames[i].band_fluxes,
            frame_idx: i,
            pitch_lane,
            melodic,
            local_rms: local_rms[i],
        };

        if let Some(prev) = last_peak_ms
            && onset.time_ms.saturating_sub(prev) < min_gap_ms
        {
            if let Some(last) = peaks.last_mut()
                && c > last.strength
            {
                *last = onset;
                last_peak_ms = Some(onset.time_ms);
            }
            continue;
        }
        peaks.push(onset);
        last_peak_ms = Some(onset.time_ms);
    }

    peaks
}

/// Map a raw envelope into `[0..=1]` by percentile: p10 → 0, p90 → 1. Makes a
/// relatively flat loudness curve (e.g. a track that never drops below 0.85)
/// still produce meaningful quiet/loud labels so downstream gating works.
fn normalize_by_percentile(values: &[f32]) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lo = sorted[sorted.len() / 10];
    let hi = sorted[sorted.len() * 9 / 10];
    let range = (hi - lo).max(1e-3);
    values
        .iter()
        .map(|&v| ((v - lo) / range).clamp(0.0, 1.0))
        .collect()
}

/// Centered ~1 s running mean of per-frame mean whitened band energy. Output
/// is in `[0..=1]` — same range as `band_energy`. Falls back to zero-padded
/// windows at the edges.
fn compute_local_rms(frames: &[NoveltyFrame]) -> Vec<f32> {
    const HALF_W: usize = 43; // 43 * 11.6ms ≈ 500ms → ~1s window
    let n = frames.len();
    let mut out = vec![0.0_f32; n];
    if n == 0 {
        return out;
    }
    // Per-frame mean across bands, then smooth with a box filter.
    let frame_mean: Vec<f32> = frames
        .iter()
        .map(|f| f.band_energy.iter().sum::<f32>() / NUM_BANDS as f32)
        .collect();
    // Prefix sum for O(n) box filter.
    let mut prefix = Vec::with_capacity(n + 1);
    prefix.push(0.0_f32);
    for &v in &frame_mean {
        prefix.push(prefix.last().unwrap() + v);
    }
    for i in 0..n {
        let lo = i.saturating_sub(HALF_W);
        let hi = (i + HALF_W + 1).min(n);
        let sum = prefix[hi] - prefix[lo];
        let cnt = (hi - lo) as f32;
        out[i] = sum / cnt.max(1.0);
    }
    out
}

fn is_local_max(novelty: &[f32], i: usize, half_window: usize) -> bool {
    let c = novelty[i];
    let lo = i.saturating_sub(half_window);
    let hi = (i + half_window).min(novelty.len() - 1);
    !novelty[lo..=hi]
        .iter()
        .enumerate()
        .any(|(j, &v)| lo + j != i && v > c)
}

fn adaptive_threshold(novelty: &[f32], i: usize, half_window: usize) -> f32 {
    let lo = i.saturating_sub(half_window);
    let hi = (i + half_window).min(novelty.len() - 1);
    let mut window: Vec<f32> = novelty[lo..=hi].to_vec();
    window.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = window[window.len() / 2];
    let mut devs: Vec<f32> = window.iter().map(|v| (v - median).abs()).collect();
    devs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad = devs[devs.len() / 2];
    (median + 1.5 * mad).max(0.04)
}

pub fn detect_bpm(frames: &[NoveltyFrame]) -> u32 {
    if frames.len() < 32 {
        return 120;
    }
    let hop_ms = (frames[1].time_ms - frames[0].time_ms).max(1.0);
    let novelty: Vec<f32> = frames.iter().map(|f| f.novelty).collect();
    let mean: f32 = novelty.iter().sum::<f32>() / novelty.len() as f32;
    let centered: Vec<f32> = novelty.iter().map(|v| v - mean).collect();

    let min_lag = ((60_000.0 / 200.0) / hop_ms).floor() as usize;
    let max_lag = ((60_000.0 / 60.0) / hop_ms).ceil() as usize;

    let mut best_lag = min_lag;
    let mut best_score = f32::MIN;
    for lag in min_lag..max_lag.min(centered.len() / 2) {
        let mut sum = 0.0_f32;
        for i in 0..(centered.len() - lag) {
            sum += centered[i] * centered[i + lag];
        }
        let bpm_est = 60_000.0 / (lag as f32 * hop_ms as f32);
        let mid_bias = 1.0 - ((bpm_est - 120.0).abs() / 240.0).min(0.3);
        let score = sum * mid_bias;
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    let bpm = 60_000.0 / (best_lag as f64 * hop_ms);
    bpm.round().clamp(60.0, 200.0) as u32
}

pub fn detect_phase(frames: &[NoveltyFrame], bpm: u32) -> u64 {
    if frames.is_empty() || bpm == 0 {
        return 0;
    }
    let hop_ms = (frames[1].time_ms - frames[0].time_ms).max(1.0);
    let beat_ms = 60_000.0 / bpm as f64;
    let beat_hops = (beat_ms / hop_ms).round() as usize;
    if beat_hops == 0 {
        return 0;
    }

    let novelty: Vec<f32> = frames.iter().map(|f| f.novelty).collect();
    let mut best_offset = 0usize;
    let mut best_score = f32::MIN;
    for offset in 0..beat_hops {
        let mut sum = 0.0_f32;
        let mut k = offset;
        while k < novelty.len() {
            let lo = k.saturating_sub(1);
            let hi = (k + 1).min(novelty.len() - 1);
            sum += novelty[lo..=hi].iter().cloned().fold(0.0_f32, f32::max);
            k += beat_hops;
        }
        if sum > best_score {
            best_score = sum;
            best_offset = offset;
        }
    }
    (best_offset as f64 * hop_ms) as u64
}

pub fn place_notes(
    onsets: &[Onset],
    frames: &[NoveltyFrame],
    bpm: u32,
    phase_ms: u64,
    difficulty: Difficulty,
) -> Vec<Note> {
    let params = difficulty_params(difficulty);
    let beat_ms = 60_000.0 / bpm as f64;
    let grid_ms = beat_ms / params.grid_div as f64;
    let hop_ms = if frames.len() >= 2 {
        frames[1].time_ms - frames[0].time_ms
    } else {
        11.6
    };

    let filtered: Vec<&Onset> = onsets
        .iter()
        .filter(|o| o.strength >= params.strength_threshold)
        .collect();

    let mut notes: Vec<Note> = Vec::new();
    let mut last_time: u64 = 0;
    let mut lane_history: Vec<u8> = Vec::with_capacity(16);

    for onset in &filtered {
        // Dynamic min-gap: loud passages compress the gap so climaxes reach
        // their musical density; quiet passages stretch it. Range [0.8, 1.3]
        // — narrow enough that total density doesn't balloon while still
        // giving choruses ~40% more note capacity than verses.
        let gap_factor = (1.25 - 0.45 * onset.local_rms).clamp(0.8, 1.3);
        let effective_gap = (params.min_gap_ms as f32 * gap_factor) as u64;

        if !notes.is_empty() && onset.time_ms.saturating_sub(last_time) < effective_gap {
            continue;
        }
        let quantized = quantize_phased(onset.time_ms, phase_ms, grid_ms);
        if !notes.is_empty() && quantized.saturating_sub(last_time) < effective_gap / 2 {
            continue;
        }

        let seed = hash_u64(quantized ^ ((onset.dominant_band as u64) << 32) ^ (difficulty as u64));

        // Chord probability scales with local intensity. Range [0.3, 1.0] —
        // we never go above the difficulty's base rate (avoids 60%+ chord
        // walls in loud sections), but quiet pre-choruses drop to 30%.
        let intensity_factor = (0.3 + 0.7 * onset.local_rms).clamp(0.3, 1.0);
        let chord_prob_eff = (params.chord_prob * intensity_factor as f64).min(0.8);

        let chord_bands = pick_chord_bands(onset, chord_prob_eff, params.max_chord_notes, seed);
        let duration_ms = detect_hold(onset, frames, hop_ms, beat_ms, phase_ms, grid_ms, quantized);
        let slide_to = pick_slide_target(
            onset,
            duration_ms,
            beat_ms,
            difficulty,
            seed.wrapping_add(17),
        );

        let mut used_lanes: Vec<u8> = Vec::with_capacity(chord_bands.len());
        for (ci, &band) in chord_bands.iter().enumerate() {
            let sub_seed = seed.wrapping_add(ci as u64 * 31);
            // Only the lead (first) note of a melodic onset follows the pitch
            // contour; chord companions still use the percussive mapping so
            // they fan out visually rather than pile up near the lead.
            let hint = if ci == 0 && onset.melodic {
                Some(onset.pitch_lane)
            } else {
                None
            };
            let lane = pick_lane(band, &lane_history, &used_lanes, sub_seed, hint);
            used_lanes.push(lane);
            // Slide only attaches to the lead (hold-bearing) note.
            let note_slide = if ci == 0 && duration_ms > 0 {
                slide_to.filter(|&t| t != lane)
            } else {
                None
            };
            notes.push(Note {
                time_ms: quantized,
                lane,
                duration_ms: if ci == 0 { duration_ms } else { 0 },
                slide_to: note_slide,
            });
            lane_history.push(lane);
            if lane_history.len() > 10 {
                lane_history.remove(0);
            }
        }
        last_time = quantized + duration_ms;
    }

    notes
}

/// Decide whether to turn a hold into a slide and, if so, pick the target
/// lane. Slide probability scales with `local_rms` so finger transitions
/// cluster on climactic moments rather than sleepy verses.
fn pick_slide_target(
    onset: &Onset,
    duration_ms: u64,
    beat_ms: f64,
    difficulty: Difficulty,
    seed: u64,
) -> Option<u8> {
    let base_prob = match difficulty {
        Difficulty::Easy | Difficulty::Medium => 0.0,
        Difficulty::Hard => 0.20,
        Difficulty::Expert => 0.35,
    };
    if base_prob <= 0.0 {
        return None;
    }
    let min_duration_ms = (beat_ms * 1.5) as u64;
    if duration_ms < min_duration_ms {
        return None;
    }
    // Intensity gate: slides strongly favour climactic moments. Factor in
    // [0.2, 1.0] — base is the max, so Hard caps at 20% and Expert at 35%
    // slide-probability among long holds.
    let intensity_factor = (0.2 + 0.8 * onset.local_rms).clamp(0.2, 1.0) as f64;
    let effective_prob = base_prob * intensity_factor;
    let roll = hash_f64(seed);
    if roll >= effective_prob {
        return None;
    }

    // Use the pitch lane as source anchor so slide direction matches the
    // melodic contour when the onset is melodic.
    let anchor = if onset.melodic {
        onset.pitch_lane as i32
    } else {
        band_to_lane(onset.dominant_band) as i32
    };
    // Bias toward ±1; longer holds occasionally jump ±2.
    let step = if duration_ms > (beat_ms * 3.0) as u64 && roll < effective_prob * 0.3 {
        2
    } else {
        1
    };
    let dir = if (seed & 1) == 0 { 1 } else { -1 };
    let mut target = anchor + dir * step;
    if !(0..=4).contains(&target) {
        target = anchor - dir * step;
    }
    let target = target.clamp(0, 4) as u8;
    Some(target)
}

struct DifficultyParams {
    strength_threshold: f32,
    min_gap_ms: u64,
    grid_div: u32,
    chord_prob: f64,
    max_chord_notes: usize,
}

fn difficulty_params(difficulty: Difficulty) -> DifficultyParams {
    match difficulty {
        Difficulty::Easy => DifficultyParams {
            strength_threshold: 0.30,
            min_gap_ms: 500,
            grid_div: 2,
            chord_prob: 0.00,
            max_chord_notes: 1,
        },
        Difficulty::Medium => DifficultyParams {
            strength_threshold: 0.20,
            min_gap_ms: 320,
            grid_div: 4,
            chord_prob: 0.15,
            max_chord_notes: 2,
        },
        Difficulty::Hard => DifficultyParams {
            strength_threshold: 0.12,
            min_gap_ms: 200,
            grid_div: 4,
            chord_prob: 0.35,
            max_chord_notes: 2,
        },
        Difficulty::Expert => DifficultyParams {
            strength_threshold: 0.06,
            min_gap_ms: 130,
            grid_div: 8,
            chord_prob: 0.55,
            max_chord_notes: 3,
        },
    }
}

fn pick_chord_bands(onset: &Onset, chord_prob: f64, max_notes: usize, seed: u64) -> Vec<u8> {
    let mut bands: Vec<(u8, f32)> = (0..NUM_BANDS as u8)
        .map(|b| (b, onset.band_fluxes[b as usize]))
        .collect();
    bands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let top_flux = bands[0].1.max(1e-6);

    let mut picked: Vec<u8> = Vec::with_capacity(max_notes);
    picked.push(bands[0].0);

    for &(band, flux) in bands.iter().skip(1) {
        if picked.len() >= max_notes {
            break;
        }
        if flux < 0.5 * top_flux {
            break;
        }
        if picked
            .iter()
            .any(|&b| band_to_lane(b) == band_to_lane(band))
        {
            continue;
        }
        let roll = hash_f64(seed.wrapping_add(picked.len() as u64 * 7));
        if roll < chord_prob {
            picked.push(band);
        }
    }
    picked
}

fn detect_hold(
    onset: &Onset,
    frames: &[NoveltyFrame],
    hop_ms: f64,
    beat_ms: f64,
    phase_ms: u64,
    grid_ms: f64,
    quantized: u64,
) -> u64 {
    let dom_flux = onset.band_fluxes[onset.dominant_band as usize];
    if dom_flux <= 0.25 {
        return 0;
    }
    let max_lookahead = (beat_ms * 2.5 / hop_ms) as usize;
    let sustain = sustain_ms(
        frames,
        onset.frame_idx,
        onset.dominant_band,
        hop_ms,
        max_lookahead,
    );
    if (sustain as f64) < beat_ms * 1.5 {
        return 0;
    }
    let raw = (sustain as f64).min(beat_ms * 2.5).max(beat_ms);
    quantize_phased((quantized as f64 + raw) as u64, phase_ms, grid_ms)
        .saturating_sub(quantized)
        .max(beat_ms as u64)
}

fn sustain_ms(
    frames: &[NoveltyFrame],
    start_frame: usize,
    band: u8,
    hop_ms: f64,
    max_lookahead_frames: usize,
) -> u64 {
    let band = band as usize;
    if start_frame >= frames.len() {
        return 0;
    }
    let start_energy = frames[start_frame].band_energy[band];
    let threshold = (start_energy * 0.75).max(0.3);
    let mut end = start_frame;
    for i in start_frame + 1..(start_frame + max_lookahead_frames).min(frames.len()) {
        if frames[i].band_energy[band] < threshold {
            break;
        }
        end = i;
    }
    ((end - start_frame) as f64 * hop_ms) as u64
}

fn quantize_phased(time_ms: u64, phase_ms: u64, grid_ms: f64) -> u64 {
    if grid_ms <= 0.0 {
        return time_ms;
    }
    let rel = time_ms as i64 - phase_ms as i64;
    let steps = (rel as f64 / grid_ms).round();
    (phase_ms as i64 + (steps * grid_ms) as i64).max(0) as u64
}

fn hash_u64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

fn hash_f64(seed: u64) -> f64 {
    (hash_u64(seed) >> 11) as f64 / (1u64 << 53) as f64
}

fn band_to_lane(band: u8) -> u8 {
    match band {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        4 | 5 => 3,
        _ => 4,
    }
}

/// Classify an onset's spectral flux into (pitch_lane, melodic). The pitch lane
/// is the spectral centroid over bands 2..8, linearly remapped onto 5 lanes;
/// `melodic` is true when mid/high-band flux dominates the low bands, i.e.
/// this is a tonal event rather than a kick or snare.
fn classify_pitch(fluxes: &[f32; NUM_BANDS]) -> (u8, bool) {
    let low: f32 = fluxes[0] + fluxes[1];
    let high: f32 = fluxes[2..NUM_BANDS].iter().sum();
    let total = (low + high).max(1e-6);
    let melodic = high > low * 1.2 && high / total > 0.55;

    // Weighted centroid over bands 2..8 mapped to 0..=4.
    let mut sum = 0.0_f32;
    let mut wsum = 0.0_f32;
    for (b, &flux) in fluxes.iter().enumerate().skip(2) {
        sum += (b as f32) * flux;
        wsum += flux;
    }
    let centroid = if wsum > 1e-6 { sum / wsum } else { 4.0 };
    // Bands 2..=7 → lanes 0..=4 linearly.
    let pitch_lane = (((centroid - 2.0) / 5.0) * 4.0).round().clamp(0.0, 4.0) as u8;
    (pitch_lane, melodic)
}

fn pick_lane(band: u8, history: &[u8], forbidden: &[u8], seed: u64, hint: Option<u8>) -> u8 {
    let base = band_to_lane(band);
    // Melodic hint: pick lanes near the spectral centroid so a rising melody
    // drifts right, falling drifts left. Candidate window widens to ±1 so the
    // anti-repeat heuristic still has options.
    let mut candidates: Vec<u8> = if let Some(h) = hint {
        let lo = h.saturating_sub(1);
        let hi = (h + 1).min(4);
        (lo..=hi).collect()
    } else {
        match base {
            0 => vec![0, 1],
            1 => vec![0, 1, 2],
            2 => vec![1, 2, 3],
            3 => vec![2, 3, 4],
            _ => vec![3, 4],
        }
    };
    candidates.retain(|c| !forbidden.contains(c));
    if candidates.is_empty() {
        candidates = (0..5u8).filter(|c| !forbidden.contains(c)).collect();
        if candidates.is_empty() {
            return hint.unwrap_or(base);
        }
    }

    let last = history.last().copied();
    let last2 = history.get(history.len().wrapping_sub(2)).copied();
    if candidates.len() > 1 {
        candidates.retain(|&c| Some(c) != last);
    }
    if candidates.len() > 1 && last == last2 {
        candidates.retain(|&c| Some(c) != last2);
    }
    candidates[(hash_u64(seed) as usize) % candidates.len()]
}
