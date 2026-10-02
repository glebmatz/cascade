use crate::app::{Action, Screen};
use crate::audio::player::AudioPlayer;
use crate::config::Config;
use crate::ui::chrome::{render_bottom_bar, render_top_bar};
use ratatui::prelude::*;

const BPM: u64 = 120;
const BEAT_MS: u64 = 60_000 / BPM; // 500 ms
const WARMUP_BEATS: u64 = 4;
const MEASURE_BEATS: u64 = 16;
const TOTAL_BEATS: u64 = WARMUP_BEATS + MEASURE_BEATS;
const LEAD_IN_MS: u64 = BEAT_MS;
const SAMPLE_RATE: u32 = 44_100;

pub struct CalibrateScreen {
    player: AudioPlayer,
    started: bool,
    hits: Vec<i64>, // signed diff in ms (press - expected_beat)
    config: Config,
    result: Option<i32>,
}

impl CalibrateScreen {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let mut player = AudioPlayer::new()?;
        player.load_samples(Self::metronome_track(), 1, SAMPLE_RATE)?;
        player.set_volume(config.audio.volume as f32);
        Ok(Self {
            player,
            started: false,
            hits: Vec::new(),
            config,
            result: None,
        })
    }

    fn metronome_track() -> Vec<i16> {
        let ms_to_samples = |ms: u64| (ms * SAMPLE_RATE as u64 / 1000) as usize;
        let mut samples = vec![0i16; ms_to_samples(LEAD_IN_MS + (TOTAL_BEATS + 1) * BEAT_MS)];
        let click_len = ms_to_samples(40);
        for beat in 0..TOTAL_BEATS {
            let start = ms_to_samples(LEAD_IN_MS + beat * BEAT_MS);
            for i in 0..click_len {
                let t = i as f32 / SAMPLE_RATE as f32;
                let env = (1.0 - i as f32 / click_len as f32).powf(1.5);
                let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * env * 0.4;
                samples[start + i] = (s * i16::MAX as f32) as i16;
            }
        }
        samples
    }

    fn track_ms(&self) -> i64 {
        self.player.position_ms() as i64 - LEAD_IN_MS as i64
    }

    pub fn start(&mut self) {
        self.player.play();
        self.started = true;
        self.hits.clear();
        self.result = None;
    }

    pub fn update(&mut self) {
        if !self.started || self.result.is_some() {
            return;
        }
        if self.track_ms() >= (TOTAL_BEATS * BEAT_MS) as i64 {
            self.finish();
        }
    }

    fn finish(&mut self) {
        if self.hits.len() < 4 {
            self.result = Some(self.config.audio.offset_ms); // keep old
            return;
        }

        // Trim outliers via IQR
        let mut sorted = self.hits.clone();
        sorted.sort();
        let q1 = sorted[sorted.len() / 4];
        let q3 = sorted[sorted.len() * 3 / 4];
        let iqr = q3 - q1;
        let lo = q1 - (iqr * 3 / 2);
        let hi = q3 + (iqr * 3 / 2);
        let trimmed: Vec<i64> = sorted.into_iter().filter(|&v| v >= lo && v <= hi).collect();
        if trimmed.is_empty() {
            self.result = Some(self.config.audio.offset_ms);
            return;
        }
        let median = trimmed[trimmed.len() / 2];
        let offset = median.clamp(-200, 200) as i32;
        self.config.audio.offset_ms = offset;
        let _ = self.config.save(&Config::default_path());
        self.result = Some(offset);
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::Back | Action::Pause | Action::Quit => Some(Action::Navigate(Screen::Settings)),
            Action::MenuSelect if self.result.is_some() => Some(Action::Navigate(Screen::Settings)),
            Action::GameKey(_) | Action::GameKeyRelease(_) => {
                if !self.started || self.result.is_some() {
                    return None;
                }
                if matches!(action, Action::GameKeyRelease(_)) {
                    return None;
                }

                let elapsed_ms = self.track_ms();
                let beat_idx = (elapsed_ms + BEAT_MS as i64 / 2).div_euclid(BEAT_MS as i64);
                // Ignore warmup beats
                if beat_idx < WARMUP_BEATS as i64 {
                    return None;
                }
                if beat_idx >= TOTAL_BEATS as i64 {
                    return None;
                }

                let nearest_beat =
                    ((elapsed_ms + BEAT_MS as i64 / 2) / BEAT_MS as i64) * BEAT_MS as i64;
                let diff = elapsed_ms - nearest_beat;
                // Only count presses within ±250 ms of a beat
                if diff.abs() <= 250 {
                    self.hits.push(diff);
                }
                None
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();
        let cx = area.x + area.width / 2;
        let cy = area.y + area.height / 2;

        // Chrome
        let top = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        render_top_bar(buf, top, &["MENU", "SETTINGS", "CALIBRATE"]);
        let bot = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        render_bottom_bar(buf, bot, &[("SPACE", "tap"), ("Esc", "cancel")]);

        let title = "AUDIO CALIBRATION";
        buf.set_string(
            cx - title.len() as u16 / 2,
            area.y + 2,
            title,
            Style::default().fg(Color::White).bold(),
        );

        let elapsed_ms = self.track_ms();
        let beat_idx = elapsed_ms.div_euclid(BEAT_MS as i64);

        if let Some(offset) = self.result {
            let lines = [
                "Calibration complete.".to_string(),
                format!("Measured offset: {:+} ms", offset),
                format!("Samples used: {}/{}", self.hits.len(), MEASURE_BEATS),
                String::new(),
                String::from("Saved to config."),
                String::new(),
                String::from("Enter: Back"),
            ];
            for (i, line) in lines.iter().enumerate() {
                let w = line.chars().count() as u16;
                let y = cy.saturating_sub(3) + i as u16;
                buf.set_string(
                    cx.saturating_sub(w / 2),
                    y,
                    line,
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                );
            }
            return;
        }

        let phase = if beat_idx < WARMUP_BEATS as i64 {
            format!("Listen — {}/{}", beat_idx.max(0) + 1, WARMUP_BEATS)
        } else if beat_idx < TOTAL_BEATS as i64 {
            format!(
                "Tap SPACE on each beat — {}/{}",
                (beat_idx - WARMUP_BEATS as i64).max(0) + 1,
                MEASURE_BEATS
            )
        } else {
            String::from("Finalizing...")
        };

        let phase_w = phase.chars().count() as u16;
        buf.set_string(
            cx.saturating_sub(phase_w / 2),
            cy.saturating_sub(2),
            &phase,
            Style::default().fg(Color::Rgb(180, 180, 180)),
        );

        // Beat indicator: fills on downbeat, fades
        let ms_in_beat = elapsed_ms.rem_euclid(BEAT_MS as i64);
        let beat_frac = 1.0 - (ms_in_beat as f64 / BEAT_MS as f64);
        let width = 20u16.min(area.width / 3);
        let bar_x = cx.saturating_sub(width / 2);
        let filled = (width as f64 * beat_frac) as u16;
        for i in 0..width {
            let style = if i < filled {
                Style::default().bg(Color::Rgb(80, 140, 220))
            } else {
                Style::default().bg(Color::Rgb(30, 30, 40))
            };
            buf.set_string(bar_x + i, cy, " ", style);
        }

        let hits_txt = format!("Hits: {}", self.hits.len());
        buf.set_string(
            cx.saturating_sub(hits_txt.len() as u16 / 2),
            cy + 2,
            &hits_txt,
            Style::default().fg(Color::Rgb(120, 120, 120)),
        );
    }
}
