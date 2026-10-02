use cascade::beatmap::generator::generate_all_beatmaps;
use cascade::beatmap::types::{Beatmap, SongMeta};

const SR: u32 = 44_100;

struct Lcg(u64);

impl Lcg {
    fn noise(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
    }
}

fn add_click(s: &mut [f32], at_ms: f64, rng: &mut Lcg) {
    let start = (at_ms / 1000.0 * SR as f64) as usize;
    for i in 0..2000 {
        if let Some(v) = s.get_mut(start + i) {
            *v += rng.noise() * (-(i as f32) / 300.0).exp() * 0.8;
        }
    }
}

fn add_kick(s: &mut [f32], at_ms: f64) {
    let start = (at_ms / 1000.0 * SR as f64) as usize;
    let mut phase = 0.0f32;
    for i in 0..12_000 {
        let t = i as f32 / SR as f32;
        phase += 2.0 * std::f32::consts::PI * (55.0 + 120.0 * (-t * 70.0).exp()) / SR as f32;
        if let Some(v) = s.get_mut(start + i) {
            *v += phase.sin() * (-t * 25.0).exp() * 0.9;
        }
    }
}

fn add_hat(s: &mut [f32], at_ms: f64, rng: &mut Lcg) {
    let start = (at_ms / 1000.0 * SR as f64) as usize;
    let mut prev = 0.0;
    for i in 0..1500 {
        let n = rng.noise();
        let hp = n - prev;
        prev = n;
        if let Some(v) = s.get_mut(start + i) {
            *v += hp * (-(i as f32) / 120.0).exp() * 0.35;
        }
    }
}

fn generate(samples: &[f32]) -> Vec<Beatmap> {
    let meta = SongMeta {
        title: "t".into(),
        artist: String::new(),
        audio_file: "audio.wav".into(),
        bpm: 0.0,
        beat_offset_ms: 0.0,
        duration_ms: (samples.len() as f64 / SR as f64 * 1000.0) as u64,
    };
    generate_all_beatmaps(samples, SR, meta)
}

fn click_track(bpm: f64, secs: f64) -> (Vec<f32>, Vec<f64>) {
    let mut s = vec![0.0f32; (secs * SR as f64) as usize];
    let mut rng = Lcg(7);
    let mut events = Vec::new();
    let mut t = 437.0;
    while t < secs * 1000.0 - 300.0 {
        add_click(&mut s, t, &mut rng);
        events.push(t);
        t += 60_000.0 / bpm;
    }
    (s, events)
}

fn assert_notes_on_events(maps: &[Beatmap], events: &[f64], label: &str) {
    for bm in maps {
        assert!(
            !bm.notes.is_empty(),
            "{label} {:?}: no notes",
            bm.difficulty
        );
        let errors: Vec<f64> = bm
            .notes
            .iter()
            .map(|n| {
                events
                    .iter()
                    .map(|e| (n.time_ms as f64 - e).abs())
                    .fold(f64::MAX, f64::min)
            })
            .collect();
        let mean = errors.iter().sum::<f64>() / errors.len() as f64;
        let max = errors.iter().cloned().fold(0.0, f64::max);
        assert!(
            mean < 8.0 && max < 20.0,
            "{label} {:?}: mean error {mean:.1}ms, max {max:.0}ms",
            bm.difficulty
        );
    }
}

#[test]
fn tempo_detection_on_click_tracks() {
    for bpm in [92.0, 100.0, 128.4, 141.7, 174.0] {
        let (samples, _) = click_track(bpm, 40.0);
        let detected = generate(&samples)[0].song.bpm;
        assert!(
            (detected - bpm).abs() < 0.5,
            "true {bpm} BPM, detected {detected:.2}"
        );
    }
}

#[test]
fn notes_land_on_clicks() {
    for bpm in [100.0, 128.4, 141.7, 174.0] {
        let (samples, events) = click_track(bpm, 40.0);
        assert_notes_on_events(&generate(&samples), &events, &format!("{bpm} BPM"));
    }
}

#[test]
fn notes_land_on_kick_and_hat_pattern() {
    let bpm = 122.0;
    let beat = 60_000.0 / bpm;
    let secs = 40.0;
    let mut s = vec![0.0f32; (secs * SR as f64) as usize];
    let mut rng = Lcg(11);
    let mut events = Vec::new();
    let mut t = 611.0;
    while t < secs * 1000.0 - 300.0 {
        add_kick(&mut s, t);
        add_hat(&mut s, t + beat / 2.0, &mut rng);
        events.push(t);
        events.push(t + beat / 2.0);
        t += beat;
    }
    assert_notes_on_events(&generate(&s), &events, "kick/hat");
}

#[test]
fn beat_offset_points_at_a_click() {
    let (samples, events) = click_track(128.4, 40.0);
    let song = &generate(&samples)[0].song;
    let beat = 60_000.0 / song.bpm;
    let phase_err = events
        .iter()
        .map(|e| {
            let k = ((e - song.beat_offset_ms) / beat).round();
            (e - song.beat_offset_ms - k * beat).abs()
        })
        .fold(0.0, f64::max);
    assert!(
        phase_err < 15.0,
        "beat grid drifts {phase_err:.1}ms from clicks"
    );
}
