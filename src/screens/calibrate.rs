use crate::app::{Action, Screen};
use crate::config::Config;
use crate::ui::chrome::{render_bottom_bar, render_top_bar};
use ratatui::prelude::*;
use rodio::{OutputStream, OutputStreamHandle, Sink, buffer::SamplesBuffer};
use std::time::Instant;

const BPM: u64 = 120;
const BEAT_MS: u64 = 60_000 / BPM; // 500 ms
const WARMUP_BEATS: u64 = 4;
const MEASURE_BEATS: u64 = 16;
const TOTAL_BEATS: u64 = WARMUP_BEATS + MEASURE_BEATS;

pub struct CalibrateScreen {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    start: Option<Instant>,
    last_beat_played: i64,
    hits: Vec<i64>, // signed diff in ms (press - expected_beat)
    config: Config,
    result: Option<i32>,
}

impl CalibrateScreen {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        Ok(Self {
            _stream: stream,
            _stream_handle: stream_handle,
            sink,
            start: None,
            last_beat_played: -1,
            hits: Vec::new(),
            config,
            result: None,
        })
    }

    fn click_samples() -> SamplesBuffer<f32> {
        // 40 ms of 1 kHz sine, fast decay
        let sr = 44_100u32;
        let len = (sr as f32 * 0.04) as usize;
        let mut samples = Vec::with_capacity(len);
        for i in 0..len {
            let t = i as f32 / sr as f32;
            let env = (1.0 - i as f32 / len as f32).powf(1.5);
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * env * 0.4;
            samples.push(s);
        }
        SamplesBuffer::new(1, sr, samples)
    }

    pub fn start(&mut self) {
        self.start = Some(Instant::now());
        self.last_beat_played = -1;
        self.hits.clear();
        self.result = None;
    }

    pub fn update(&mut self) {
        let Some(start) = self.start else { return };
        if self.result.is_some() {
            return;
        }

        let elapsed_ms = start.elapsed().as_millis() as i64;
        let beat_idx = elapsed_ms / BEAT_MS as i64;

        if beat_idx > self.last_beat_played && beat_idx < TOTAL_BEATS as i64 {
            self.sink.append(Self::click_samples());
            self.last_beat_played = beat_idx;
        }

        if beat_idx >= TOTAL_BEATS as i64 {
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
                let Some(start) = self.start else { return None };
                if self.result.is_some() {
                    return None;
                }
                if matches!(action, Action::GameKeyRelease(_)) {
                    return None;
                }

                let elapsed_ms = start.elapsed().as_millis() as i64;
                let beat_idx = elapsed_ms / BEAT_MS as i64;
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

        let elapsed_ms = self
            .start
            .map(|s| s.elapsed().as_millis() as i64)
            .unwrap_or(0);
        let beat_idx = elapsed_ms / BEAT_MS as i64;

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
        let ms_in_beat = (elapsed_ms % BEAT_MS as i64).max(0);
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
