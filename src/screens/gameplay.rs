use ratatui::prelude::*;
use ratatui::layout::{Layout, Constraint, Direction};
use ratatui::widgets::Widget;
use crate::app::{Action, Screen};
use crate::audio::player::AudioPlayer;
use crate::audio::analyzer::{SpectrumData, SpectrumAnalyzer};
use crate::beatmap::types::Beatmap;
use crate::game::state::GameState;
use crate::game::hit_judge::{HitJudge, Judgement};
use crate::game::highway::Highway;
use crate::ui::highway_render::HighwayWidget;
use crate::ui::hud::{HudTop, HudBottom};
use crate::ui::visualizer::{WaveVisualizer, BlockVisualizer, Side};
use std::path::Path;
use anyhow::Result;

pub struct GameplayScreen {
    pub beatmap: Beatmap,
    pub audio: AudioPlayer,
    pub state: GameState,
    pub highway: Highway,
    pub judge: HitJudge,
    pub hit_notes: Vec<bool>,
    pub hit_flash: [u8; 5],
    pub paused: bool,
    pub finished: bool,
    pub spectrum: SpectrumData,
    pub judgement_timer: u8,
    pub analyzer: SpectrumAnalyzer,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl GameplayScreen {
    pub fn new(
        beatmap: Beatmap,
        audio_path: &Path,
        samples: Vec<f32>,
        sample_rate: u32,
        offset_ms: i32,
        scroll_speed: f64,
        volume: f64,
    ) -> Result<Self> {
        let mut audio = AudioPlayer::new()?;
        audio.load(audio_path)?;
        audio.set_volume(volume as f32);

        let hit_notes = vec![false; beatmap.notes.len()];

        Ok(Self {
            highway: Highway::new(scroll_speed),
            judge: HitJudge::new(offset_ms),
            state: GameState::new(),
            hit_notes,
            hit_flash: [0; 5],
            paused: false,
            finished: false,
            spectrum: SpectrumData::empty(32),
            judgement_timer: 0,
            analyzer: SpectrumAnalyzer::new(),
            samples,
            sample_rate,
            beatmap,
            audio,
        })
    }

    pub fn start(&mut self) {
        self.audio.play();
    }

    pub fn update(&mut self) {
        if self.paused || self.finished {
            return;
        }

        self.audio.update_position();
        let current_ms = self.audio.position_ms();

        // Update highway
        self.highway.update(&self.beatmap.notes, current_ms, 2000, &self.hit_notes);

        // Auto-miss expired notes
        for (i, note) in self.beatmap.notes.iter().enumerate() {
            if !self.hit_notes[i] && self.judge.is_expired(note.time_ms, current_ms) {
                self.hit_notes[i] = true;
                self.state.register_judgement(Judgement::Miss);
                self.judgement_timer = 30;
            }
        }

        // Decay hit flash
        for flash in &mut self.hit_flash {
            *flash = flash.saturating_sub(1);
        }

        // Decay judgement display
        if self.judgement_timer > 0 {
            self.judgement_timer -= 1;
            if self.judgement_timer == 0 {
                self.state.last_judgement = None;
            }
        }

        // Update spectrum analyzer
        let sample_pos = (current_ms as f64 / 1000.0 * self.sample_rate as f64) as usize;
        let chunk_size = 1024;
        if sample_pos + chunk_size <= self.samples.len() {
            self.analyzer.process(&self.samples[sample_pos..sample_pos + chunk_size]);
            if let Ok(spectrum) = self.analyzer.spectrum.lock() {
                self.spectrum = SpectrumData {
                    bands: spectrum.bands.clone(),
                    energy: spectrum.energy,
                };
            }
        }

        // Check if song finished
        if self.audio.is_finished() || (current_ms > self.beatmap.song.duration_ms + 2000) {
            self.finished = true;
        }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::Pause | Action::Back => {
                if self.finished {
                    return Some(Action::Navigate(Screen::Results));
                }
                self.paused = !self.paused;
                if self.paused {
                    self.audio.pause();
                } else {
                    self.audio.resume();
                }
                None
            }
            Action::GameKey(lane) if !self.paused && !self.finished => {
                self.hit_flash[lane] = 8;
                let current_ms = self.audio.position_ms();

                let mut best: Option<(usize, u64)> = None;
                for (i, note) in self.beatmap.notes.iter().enumerate() {
                    if self.hit_notes[i] || note.lane as usize != lane { continue; }
                    let diff = (note.time_ms as i64 - current_ms as i64).unsigned_abs();
                    if diff <= 100 {
                        if best.is_none() || diff < best.unwrap().1 {
                            best = Some((i, diff));
                        }
                    }
                }

                if let Some((note_idx, _)) = best {
                    let note_time = self.beatmap.notes[note_idx].time_ms;
                    let judgement = self.judge.judge(note_time, current_ms);
                    self.hit_notes[note_idx] = true;
                    self.state.register_judgement(judgement);
                    self.judgement_timer = 30;
                }
                None
            }
            Action::Quit if self.paused => {
                self.audio.stop();
                Some(Action::Navigate(Screen::SongSelect))
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

        let top_area = vertical[0];
        let mid_area = vertical[1];
        let bot_area = vertical[2];

        // Top: wave + HUD
        if top_area.height >= 1 {
            WaveVisualizer { spectrum: &self.spectrum }
                .render(Rect { height: 1, ..top_area }, buf);
        }
        if top_area.height >= 2 {
            HudTop { state: &self.state }
                .render(Rect { y: top_area.y + 1, height: 1, ..top_area }, buf);
        }

        // Middle: side visualizers + highway
        let side_width = (mid_area.width / 5).max(3);
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(side_width),
                Constraint::Min(20),
                Constraint::Length(side_width),
            ])
            .split(mid_area);

        BlockVisualizer { spectrum: &self.spectrum, side: Side::Left }.render(horizontal[0], buf);
        HighwayWidget::new(&self.highway.visible_notes)
            .with_hit_flash(self.hit_flash)
            .render(horizontal[1], buf);
        BlockVisualizer { spectrum: &self.spectrum, side: Side::Right }.render(horizontal[2], buf);

        // Bottom HUD
        let progress = if self.beatmap.song.duration_ms > 0 {
            self.audio.position_ms() as f64 / self.beatmap.song.duration_ms as f64
        } else {
            0.0
        };

        HudBottom {
            state: &self.state,
            song_title: &format!("{} — {}", self.beatmap.song.title, self.beatmap.song.artist),
            progress,
            difficulty: &self.beatmap.difficulty.to_string().to_uppercase(),
        }.render(bot_area, buf);

        // Pause overlay
        if self.paused {
            let y = area.y + area.height / 2;
            let pause_text = "PAUSED";
            let x = area.x + (area.width.saturating_sub(pause_text.len() as u16)) / 2;
            buf.set_string(x, y, pause_text, Style::default().fg(Color::White).bold());

            let hint = "ESC: Resume   Q: Quit to songs";
            let x = area.x + (area.width.saturating_sub(hint.len() as u16)) / 2;
            buf.set_string(x, y + 1, hint, Style::default().fg(Color::Rgb(100, 100, 100)));
        }
    }
}
