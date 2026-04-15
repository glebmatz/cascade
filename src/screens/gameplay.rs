use ratatui::prelude::*;
use ratatui::layout::{Layout, Constraint, Direction};
use crate::app::{Action, Screen};
use crate::audio::player::AudioPlayer;
use crate::audio::analyzer::{SpectrumData, SpectrumAnalyzer};
use crate::beatmap::types::Beatmap;
use crate::game::state::GameState;
use crate::game::hit_judge::{HitJudge, Judgement};
use crate::game::highway::Highway;
use crate::ui::highway_render::HighwayWidget;
use crate::ui::hud::{HudTop, HudBottom};
use std::path::Path;
use anyhow::Result;

pub struct GameplayScreen {
    pub beatmap: Beatmap,
    pub audio: AudioPlayer,
    pub state: GameState,
    pub highway: Highway,
    pub judge: HitJudge,
    pub hit_notes: Vec<bool>,
    pub held_notes: Vec<Option<usize>>, // per lane: index of note being held
    pub hit_flash: [u8; 5],
    pub paused: bool,
    pub finished: bool,
    pub spectrum: SpectrumData,
    pub judgement_timer: u8,
    pub analyzer: SpectrumAnalyzer,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub particles: Vec<(u16, u16, u8)>,
    pub last_render_area: Option<Rect>,
    pub health_enabled: bool,
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
        health_enabled: bool,
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
            held_notes: vec![None; 5],
            hit_flash: [0; 5],
            paused: false,
            finished: false,
            spectrum: SpectrumData::empty(32),
            judgement_timer: 0,
            analyzer: SpectrumAnalyzer::new(),
            samples,
            sample_rate,
            particles: Vec::new(),
            last_render_area: None,
            health_enabled,
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

        // Auto-miss expired tap notes, check hold note completions
        for (i, note) in self.beatmap.notes.iter().enumerate() {
            if self.hit_notes[i] { continue; }

            if note.duration_ms > 0 {
                // Hold note: check if it's being held and has completed
                let lane = note.lane as usize;
                if self.held_notes[lane] == Some(i) {
                    let note_end = note.time_ms + note.duration_ms;
                    if current_ms >= note_end {
                        // Successfully held to end
                        self.hit_notes[i] = true;
                        self.held_notes[lane] = None;
                        self.state.register_judgement(Judgement::Perfect);
                        self.judgement_timer = 30;
                    }
                } else if self.judge.is_expired(note.time_ms, current_ms) {
                    // Never started holding
                    self.hit_notes[i] = true;
                    self.state.register_judgement(Judgement::Miss);
                    self.judgement_timer = 30;
                }
            } else {
                // Tap note
                if self.judge.is_expired(note.time_ms, current_ms) {
                    self.hit_notes[i] = true;
                    self.state.register_judgement(Judgement::Miss);
                    self.judgement_timer = 30;
                }
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

        // Decay particles
        for p in &mut self.particles {
            p.2 = p.2.saturating_sub(1);
        }
        self.particles.retain(|p| p.2 > 0);

        // Check if song finished or player died
        if self.audio.is_finished() || (current_ms > self.beatmap.song.duration_ms + 2000) {
            self.finished = true;
        }
        if self.health_enabled && self.state.is_dead() {
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

                // Skip notes that are already being held
                let mut best: Option<(usize, u64)> = None;
                for (i, note) in self.beatmap.notes.iter().enumerate() {
                    if self.hit_notes[i] || note.lane as usize != lane { continue; }
                    if self.held_notes[lane] == Some(i) { continue; }
                    let diff = (note.time_ms as i64 - current_ms as i64).unsigned_abs();
                    if diff <= 100 {
                        if best.is_none() || diff < best.unwrap().1 {
                            best = Some((i, diff));
                        }
                    }
                }

                if let Some((note_idx, _)) = best {
                    let note = &self.beatmap.notes[note_idx];
                    let judgement = self.judge.judge(note.time_ms, current_ms);

                    if note.duration_ms > 0 {
                        // Hold note: register initial hit and start holding
                        self.state.register_judgement(judgement);
                        self.judgement_timer = 30;
                        self.held_notes[lane] = Some(note_idx);
                        // Don't mark as hit yet — completed when hold ends
                    } else {
                        // Tap note
                        self.hit_notes[note_idx] = true;
                        self.state.register_judgement(judgement);
                        self.judgement_timer = 30;
                    }

                    // Spawn particles on Perfect/Great hits
                    if matches!(judgement, Judgement::Perfect | Judgement::Great) {
                        if let Some(area) = self.last_render_area {
                            let count = if judgement == Judgement::Perfect { 5 } else { 3 };
                            // Approximate receptor position for this lane
                            let lane_width = area.width / 5;
                            let lane_cx = area.x + lane as u16 * lane_width + lane_width / 2;
                            let hit_y = area.y + area.height.saturating_sub(4);
                            for i in 0..count {
                                let dx = (i as i16 - count as i16 / 2) * 2;
                                let dy = -(i as i16 % 3);
                                let px = (lane_cx as i16 + dx).max(0) as u16;
                                let py = (hit_y as i16 + dy).max(0) as u16;
                                self.particles.push((px, py, 10));
                            }
                        }
                    }
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.last_render_area = Some(area);
        let buf = frame.buffer_mut();

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),   // top HUD
                Constraint::Min(10),     // highway (full width)
                Constraint::Length(1),   // bottom info
            ])
            .split(area);

        let top_area = vertical[0];
        let mid_area = vertical[1];
        let bot_area = vertical[2];

        // Top HUD: combo left, score right
        HudTop { state: &self.state, health_enabled: self.health_enabled }.render(top_area, buf);

        // Highway — full width, no side panels
        HighwayWidget::new(&self.highway.visible_notes)
            .with_hit_flash(self.hit_flash)
            .with_judgement(self.state.last_judgement, self.judgement_timer)
            .with_particles(&self.particles)
            .with_energy(self.spectrum.energy)
            .render(mid_area, buf);

        // Bottom info: song + accuracy + progress
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
            total_notes: self.beatmap.notes.len() as u32,
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
