use crate::beatmap::types::Note;

const WINDOW_MS: u64 = 2_000;
const STEP_MS: u64 = 250;
const JACK_MS: f64 = 300.0;
const PREVIEW_WINDOW_MS: u64 = 12_000;

pub fn star_rating(notes: &[Note]) -> f32 {
    let Some(last) = notes.iter().map(|n| n.time_ms).max() else {
        return 0.0;
    };
    let mut sorted: Vec<&Note> = notes.iter().collect();
    sorted.sort_by_key(|n| n.time_ms);

    let mut lane_last: [Option<u64>; 5] = [None; 5];
    let weighted: Vec<(u64, f64)> = sorted
        .iter()
        .map(|n| {
            let lane = (n.lane as usize).min(4);
            let mut weight = 1.0;
            if let Some(prev) = lane_last[lane] {
                let gap = n.time_ms.saturating_sub(prev) as f64;
                weight += 0.8 * (1.0 - gap / JACK_MS).max(0.0);
            }
            if n.duration_ms > 0 {
                weight += 0.3;
            }
            if n.slide_to.is_some() {
                weight += 0.4;
            }
            lane_last[lane] = Some(n.time_ms + n.duration_ms);
            (n.time_ms, weight)
        })
        .collect();

    let mut densities = Vec::new();
    let mut start = 0usize;
    let mut end = 0usize;
    let mut sum = 0.0;
    let mut t = 0u64;
    while t <= last {
        while end < weighted.len() && weighted[end].0 < t + WINDOW_MS {
            sum += weighted[end].1;
            end += 1;
        }
        while start < end && weighted[start].0 < t {
            sum -= weighted[start].1;
            start += 1;
        }
        densities.push(sum * 1000.0 / WINDOW_MS as f64);
        t += STEP_MS;
    }

    densities.sort_by(|a, b| b.total_cmp(a));
    let top = (densities.len() / 10).max(1);
    let peak = densities[..top].iter().sum::<f64>() / top as f64;
    (0.8 * peak.powf(0.8)) as f32
}

pub fn preview_start_ms(notes: &[Note], duration_ms: u64) -> u64 {
    let mut times: Vec<u64> = notes.iter().map(|n| n.time_ms).collect();
    if times.is_empty() || duration_ms <= PREVIEW_WINDOW_MS {
        return duration_ms * 35 / 100;
    }
    times.sort_unstable();
    let mut best = (0usize, 0u64);
    let mut end = 0usize;
    for (start, &t) in times.iter().enumerate() {
        while end < times.len() && times[end] < t + PREVIEW_WINDOW_MS {
            end += 1;
        }
        if end - start > best.0 {
            best = (end - start, t);
        }
    }
    best.1
        .saturating_sub(1_000)
        .min(duration_ms.saturating_sub(PREVIEW_WINDOW_MS))
}
