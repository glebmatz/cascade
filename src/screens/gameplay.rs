use anyhow::Result;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::*;
use std::path::Path;

use crate::app::{Action, Screen};
use crate::audio::analyzer::{SpectrumAnalyzer, SpectrumData};
use crate::audio::player::AudioPlayer;
use crate::audio::sfx;
use crate::beatmap::types::Beatmap;
use crate::game::effects::{Particle, Star};
use crate::game::highway::Highway;
use crate::game::hit_judge::{HitJudge, Judgement};
use crate::game::modifiers::{Modifier, Mods};
use crate::game::practice::PracticeConfig;
use crate::game::state::GameState;
use crate::ui::highway_render::HighwayWidget;
use crate::ui::hud::{HudBottom, HudTop};

const SPECTRUM_CHUNK: usize = 1024;
const HIGHWAY_LOOK_AHEAD_MS: u64 = 2000;
const JUDGEMENT_DISPLAY_FRAMES: u8 = 30;
const MILESTONE_DISPLAY_FRAMES: u8 = 48;
const HIT_FLASH_FRAMES: u8 = 8;
const LANE_BURST_FRAMES: u8 = 8;
const SHAKE_FRAMES_ON_MISS: u8 = 6;
const HOLD_RELEASE_GRACE_MS: u64 = 50;
const STAR_COUNT: usize = 40;
const ABERRATION_FRAMES_ON_PERFECT: u8 = 6;

const COMBO_MILESTONES: &[(u32, &str)] = &[
    (25, "NICE!"),
    (50, "FIRE!"),
    (100, "UNSTOPPABLE"),
    (200, "LEGENDARY"),
    (300, "INSANITY"),
    (500, "GODLIKE"),
    (750, "SCREAM"),
    (1000, "SCREAM"),
];

pub struct GameplayScreen {
    pub beatmap: Beatmap,
    pub audio: AudioPlayer,
    pub state: GameState,
    pub highway: Highway,
    pub judge: HitJudge,
    pub hit_notes: Vec<bool>,
    pub held_notes: Vec<Option<usize>>,
    pub hit_flash: [u8; 5],
    pub lane_burst: [u8; 5],
    pub paused: bool,
    pub finished: bool,
    pub spectrum: SpectrumData,
    pub judgement_timer: u8,
    pub judgement_elapsed: u8,
    pub analyzer: SpectrumAnalyzer,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub particles: Vec<Particle>,
    pub stars: Vec<Star>,
    pub last_render_area: Option<Rect>,
    pub health_enabled: bool,
    pub shake_frames: u8,
    pub shake_seed: u32,
    pub sfx_volume: f32,
    pub scroll_speed: f64,
    pub milestone: Option<MilestoneSplash>,
    pub last_milestone: u32,
    pub mods: Mods,
    /// Last track-time position used by `tick_drain`. Only meaningful in drain mode.
    pub last_drain_tick_ms: u64,
    /// Remaining frames for the chromatic-aberration flash on Perfect hits.
    pub aberration_frames: u8,
    /// Pre-downsampled waveform (one peak-amplitude value per bucket, 0..=1).
    /// Empty when the song is too short to bother. Resampled per-frame to
    /// terminal width inside the widget.
    pub waveform: Vec<f32>,
    /// Whether the terminal reports key release events natively (kitty keyboard
    /// protocol). When false, the game falls back to OS key-repeat based
    /// emulation of hold releases.
    pub kb_enhanced: bool,
    /// Per-lane timestamp of the last press/repeat event seen. Used by hold
    /// release emulation only.
    pub hold_last_seen_ms: [u64; 5],
    /// Per-lane timestamp of the initial press for the current hold. Used to
    /// wait out the OS key-repeat initial delay before declaring a release.
    pub hold_pressed_at_ms: [u64; 5],
    /// When set, the run is a practice loop: gameplay loops the section, score
    /// and achievements are not recorded, and `mods` are expected to be empty.
    pub practice: Option<PracticeConfig>,
    /// Pre-computed badge shown in the top HUD while practising.
    pub practice_label: Option<String>,
    /// Audio speed multiplier. Always set (1.0 when not in practice) so every
    /// caller goes through `position_ms_in_track` uniformly.
    pub speed: f32,
}

pub struct MilestoneSplash {
    pub text: String,
    pub timer: u8,
    pub elapsed: u8,
}

impl GameplayScreen {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        beatmap: Beatmap,
        _audio_path: &Path,
        samples: Vec<f32>,
        sample_rate: u32,
        offset_ms: i32,
        scroll_speed: f64,
        volume: f64,
        health_enabled: bool,
        holds_enabled: bool,
        drain_mode: bool,
        kb_enhanced: bool,
        mods: Mods,
        practice: Option<PracticeConfig>,
    ) -> Result<Self> {
        let mut audio = AudioPlayer::new()?;
        audio.load_samples(&samples, sample_rate)?;
        audio.set_volume(volume as f32);

        let mut beatmap = beatmap;
        if !holds_enabled {
            for note in &mut beatmap.notes {
                note.duration_ms = 0;
            }
        }

        let hit_notes = vec![false; beatmap.notes.len()];
        let speed = practice.as_ref().map(|p| p.speed).unwrap_or(1.0);
        let practice_label = practice.as_ref().map(|p| p.badge());

        // Drain implies health on. Practice disables drain: it's a learning mode.
        let drain_active = drain_mode && health_enabled && practice.is_none();
        let mut state = GameState::new();
        state.drain_mode = drain_active;

        let waveform = downsample_waveform(&samples, 512);

        Ok(Self {
            highway: Highway::new(scroll_speed),
            judge: HitJudge::new(offset_ms),
            state,
            hit_notes,
            held_notes: vec![None; 5],
            hit_flash: [0; 5],
            lane_burst: [0; 5],
            paused: false,
            finished: false,
            spectrum: SpectrumData::empty(32),
            judgement_timer: 0,
            judgement_elapsed: 0,
            analyzer: SpectrumAnalyzer::new(),
            samples,
            sample_rate,
            particles: Vec::new(),
            stars: init_stars(),
            last_render_area: None,
            health_enabled,
            shake_frames: 0,
            shake_seed: 0,
            sfx_volume: (volume as f32 * 0.8).clamp(0.0, 1.0),
            scroll_speed,
            milestone: None,
            last_milestone: 0,
            mods,
            last_drain_tick_ms: 0,
            aberration_frames: 0,
            waveform,
            kb_enhanced,
            hold_last_seen_ms: [0; 5],
            hold_pressed_at_ms: [0; 5],
            practice,
            practice_label,
            speed,
            beatmap,
            audio,
        })
    }

    pub fn start(&mut self) {
        // Apply practice speed/seek before we start playback so the first
        // frame is already at the right position.
        if let Some(p) = &self.practice {
            self.audio.set_speed(p.speed);
        }
        self.audio.play();
        if let Some(p) = &self.practice {
            let _ = self.audio.seek_to_ms(p.section_start_wallclock_ms());
        }
    }

    pub fn is_practice(&self) -> bool {
        self.practice.is_some()
    }

    /// Playback position mapped to track-time (beatmap-time) milliseconds.
    /// `rodio`'s `sink.get_pos()` reports wall-clock elapsed time; when speed
    /// differs from 1.0 we multiply to recover the track position.
    pub fn position_ms_in_track(&self) -> u64 {
        (self.audio.position_ms() as f64 * self.speed as f64) as u64
    }

    pub fn update(&mut self) {
        if self.paused || self.finished {
            return;
        }

        self.audio.update_position();

        // Practice loop: if we've played past the section end, seek back and
        // reset game state so the run remains non-accumulative.
        if let Some(end_ms) = self.practice.as_ref().map(|p| p.section_end_ms)
            && self.position_ms_in_track() >= end_ms
        {
            self.reset_practice_loop();
            return;
        }

        let current_ms = self.position_ms_in_track();

        self.highway.update(
            &self.beatmap.notes,
            current_ms,
            HIGHWAY_LOOK_AHEAD_MS,
            &self.hit_notes,
        );

        self.process_auto_events(current_ms);
        self.emulate_hold_releases(current_ms);
        self.tick_visual_timers();
        self.advance_stars();
        self.advance_particles();
        self.update_spectrum(current_ms);
        self.tick_drain(current_ms);
        self.check_finish_conditions(current_ms);
    }

    /// On terminals that don't report key release events, we infer releases from
    /// the absence of OS key-repeat events. A repeat stream starts after the
    /// initial OS delay (~300–500ms), so we wait that out before declaring any
    /// release; after that, two repeat-intervals of silence counts as a release.
    fn emulate_hold_releases(&mut self, current_ms: u64) {
        if self.kb_enhanced {
            return;
        }
        const INITIAL_GRACE_MS: u64 = 550;
        const REPEAT_GRACE_MS: u64 = 120;
        for lane in 0..5 {
            if self.held_notes[lane].is_none() {
                continue;
            }
            let since_press = current_ms.saturating_sub(self.hold_pressed_at_ms[lane]);
            let since_refresh = current_ms.saturating_sub(self.hold_last_seen_ms[lane]);
            if since_press > INITIAL_GRACE_MS && since_refresh > REPEAT_GRACE_MS {
                self.handle_key_release(lane);
            }
        }
    }

    /// Advance continuous health drain if drain mode is on. Uses track-time
    /// deltas so slowing with practice doesn't cheat drain (though drain is
    /// disabled in practice anyway).
    fn tick_drain(&mut self, current_ms: u64) {
        if !self.state.drain_mode {
            return;
        }
        if self.last_drain_tick_ms == 0 {
            self.last_drain_tick_ms = current_ms;
            return;
        }
        let dt = current_ms.saturating_sub(self.last_drain_tick_ms);
        if dt == 0 {
            return;
        }
        self.state.tick_drain(dt);
        self.last_drain_tick_ms = current_ms;
    }

    fn reset_practice_loop(&mut self) {
        let Some(p) = self.practice.as_ref() else {
            return;
        };
        let wallclock = p.section_start_wallclock_ms();
        let _ = self.audio.seek_to_ms(wallclock);

        // Practice disables drain, but we still reset the timer so re-entry is
        // clean if practice is ever removed.
        self.state = GameState::new();
        self.last_drain_tick_ms = 0;
        for flag in &mut self.hit_notes {
            *flag = false;
        }
        for slot in &mut self.held_notes {
            *slot = None;
        }
        self.hit_flash = [0; 5];
        self.lane_burst = [0; 5];
        self.judgement_timer = 0;
        self.judgement_elapsed = 0;
        self.shake_frames = 0;
        self.aberration_frames = 0;
        self.milestone = None;
        self.last_milestone = 0;
        self.particles.clear();
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
                let now = self.position_ms_in_track();
                let prev_seen = self.hold_last_seen_ms[lane];
                self.hold_last_seen_ms[lane] = now;
                // On terminals that don't report Repeat events (e.g. macOS
                // Terminal.app), OS auto-repeat arrives as a stream of Press
                // events. While a hold is already active on this lane, ignore
                // Press events that land within the OS repeat window so they
                // can't accidentally auto-hit the next note.
                let is_repeat_like = !self.kb_enhanced
                    && self.held_notes[lane].is_some()
                    && now.saturating_sub(prev_seen) < 80;
                if !is_repeat_like {
                    self.hold_pressed_at_ms[lane] = now;
                    self.handle_key_press(lane);
                }
                None
            }
            Action::GameKeyHeld(lane) if !self.paused && !self.finished => {
                // Repeat event — refresh "still held" timer for hold emulation.
                // Ignored on terminals that report real release events.
                if !self.kb_enhanced {
                    self.hold_last_seen_ms[lane] = self.position_ms_in_track();
                }
                None
            }
            Action::GameKeyRelease(lane) if !self.paused && !self.finished => {
                self.handle_key_release(lane);
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

        let shake_dx = self.compute_shake();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .split(area);

        let top_area = layout[0];
        let mut mid_area = layout[1];
        let bot_area = layout[2];
        apply_shake(&mut mid_area, area, shake_dx);

        HudTop {
            state: &self.state,
            health_enabled: self.health_enabled,
            practice_label: self.practice_label.as_deref(),
        }
        .render(top_area, buf);

        let aberration = if self.aberration_frames > 0 {
            self.aberration_frames as f32 / ABERRATION_FRAMES_ON_PERFECT as f32 * 0.7
        } else {
            0.0
        };
        HighwayWidget::new(&self.highway.visible_notes)
            .with_hit_flash(self.hit_flash)
            .with_lane_burst(self.lane_burst)
            .with_judgement(
                self.state.last_judgement,
                self.judgement_timer,
                self.judgement_elapsed,
            )
            .with_particles(&self.particles)
            .with_stars(&self.stars)
            .with_spectrum(&self.spectrum.bands)
            .with_energy(self.spectrum.energy)
            .with_beat_pulse(self.beat_pulse())
            .with_combo(self.state.combo)
            .with_timing(self.position_ms_in_track(), 2000.0 / self.scroll_speed)
            .with_mods(self.mods.clone())
            .with_aberration(aberration)
            .render(mid_area, buf);

        self.draw_milestone_splash(buf, area);

        let progress = if let Some(p) = &self.practice {
            // Progress through the practice section (0..1) — not the song.
            let span = p.section_end_ms.saturating_sub(p.section_start_ms).max(1);
            let within = self
                .position_ms_in_track()
                .saturating_sub(p.section_start_ms);
            (within as f64 / span as f64).clamp(0.0, 1.0)
        } else if self.beatmap.song.duration_ms > 0 {
            self.position_ms_in_track() as f64 / self.beatmap.song.duration_ms as f64
        } else {
            0.0
        };
        HudBottom {
            state: &self.state,
            song_title: &format!("{} — {}", self.beatmap.song.title, self.beatmap.song.artist),
            progress,
            difficulty: &self.beatmap.difficulty.to_string().to_uppercase(),
            total_notes: self.beatmap.notes.len() as u32,
            waveform: &self.waveform,
        }
        .render(bot_area, buf);

        if self.paused {
            draw_pause_overlay(buf, area);
        }
    }
}

// ─── update helpers ──────────────────────────────────────────────────────────

enum AutoEvent {
    HoldComplete(usize, usize),
    Miss(usize),
}

impl GameplayScreen {
    fn process_auto_events(&mut self, current_ms: u64) {
        let mut events = Vec::new();
        for (i, note) in self.beatmap.notes.iter().enumerate() {
            if self.hit_notes[i] {
                continue;
            }
            if note.duration_ms > 0 {
                // Is this note held on any lane? For ordinary holds it'll be
                // on its source lane; during a slide the source slot is kept
                // filled even after finger release, so the check is identical.
                let held_on: Option<usize> = (0..5).find(|&l| self.held_notes[l] == Some(i));
                match (note.slide_to, held_on) {
                    (Some(_), Some(src)) => {
                        // Slide in progress: don't auto-complete. Miss only
                        // when the hold's end has passed without a slide_to press.
                        if current_ms > note.time_ms + note.duration_ms + HitJudge::MISS_MS {
                            events.push(AutoEvent::Miss(i));
                            // Clear the slot so we don't re-enter this branch.
                            self.held_notes[src] = None;
                        }
                    }
                    (None, Some(src)) => {
                        if current_ms >= note.time_ms + note.duration_ms {
                            events.push(AutoEvent::HoldComplete(i, src));
                        }
                    }
                    (_, None) => {
                        if self.judge.is_expired(note.time_ms, current_ms) {
                            events.push(AutoEvent::Miss(i));
                        }
                    }
                }
            } else if self.judge.is_expired(note.time_ms, current_ms) {
                events.push(AutoEvent::Miss(i));
            }
        }
        for ev in events {
            match ev {
                AutoEvent::HoldComplete(i, lane) => self.complete_hold(i, lane),
                AutoEvent::Miss(i) => self.register_miss(i),
            }
        }
    }

    fn complete_hold(&mut self, idx: usize, lane: usize) {
        self.hit_notes[idx] = true;
        self.held_notes[lane] = None;
        self.aberration_frames = ABERRATION_FRAMES_ON_PERFECT;
        self.register_judgement(Judgement::Perfect, lane);
        self.check_milestone();
    }

    fn register_miss(&mut self, idx: usize) {
        self.hit_notes[idx] = true;
        self.state.register_judgement(Judgement::Miss);
        self.judgement_timer = JUDGEMENT_DISPLAY_FRAMES;
        self.judgement_elapsed = 0;
        self.shake_frames = SHAKE_FRAMES_ON_MISS;
        self.last_milestone = 0;
        self.audio.play_sfx(
            sfx::sfx_for(Judgement::Miss),
            sfx::SFX_SAMPLE_RATE,
            self.sfx_volume * 0.6,
        );
        if self.mods.contains(Modifier::SuddenDeath) {
            self.finished = true;
        }
    }

    fn register_judgement(&mut self, judgement: Judgement, lane: usize) {
        let judgement = self.transform_judgement(judgement);
        self.state.register_judgement(judgement);
        self.judgement_timer = JUDGEMENT_DISPLAY_FRAMES;
        self.judgement_elapsed = 0;
        if judgement != Judgement::Miss {
            self.lane_burst[lane] = LANE_BURST_FRAMES;
        }
        if judgement == Judgement::Perfect {
            self.aberration_frames = ABERRATION_FRAMES_ON_PERFECT;
        }
        if judgement == Judgement::Miss {
            self.shake_frames = SHAKE_FRAMES_ON_MISS;
            if self.mods.contains(Modifier::SuddenDeath) {
                self.finished = true;
            }
        }
        self.audio.play_sfx(
            sfx::sfx_for(judgement),
            sfx::SFX_SAMPLE_RATE,
            self.sfx_volume,
        );
    }

    fn transform_judgement(&self, j: Judgement) -> Judgement {
        if self.mods.contains(Modifier::PerfectOnly) && j != Judgement::Perfect {
            Judgement::Miss
        } else {
            j
        }
    }

    fn handle_key_press(&mut self, lane: usize) {
        self.hit_flash[lane] = HIT_FLASH_FRAMES;
        let current_ms = self.position_ms_in_track();

        // Slide completion takes priority over regular hit detection: pressing
        // the slide target while its source hold is still active completes the
        // slide for a Perfect.
        if self.try_slide_complete(lane, current_ms) {
            return;
        }

        let best = self.find_closest_note(lane, current_ms);
        let Some((note_idx, _)) = best else { return };

        let note = &self.beatmap.notes[note_idx];
        let judgement = self.judge.judge(note.time_ms, current_ms);

        if note.duration_ms > 0 {
            self.state.register_judgement(judgement);
            self.judgement_timer = JUDGEMENT_DISPLAY_FRAMES;
            self.judgement_elapsed = 0;
            self.held_notes[lane] = Some(note_idx);
            self.lane_burst[lane] = LANE_BURST_FRAMES;
            self.audio.play_sfx(
                sfx::sfx_for(judgement),
                sfx::SFX_SAMPLE_RATE,
                self.sfx_volume,
            );
        } else {
            self.hit_notes[note_idx] = true;
            self.register_judgement(judgement, lane);
        }

        if let Some(area) = self.last_render_area {
            self.spawn_hit_particles(lane, judgement, area);
        }
        self.check_milestone();
    }

    fn handle_key_release(&mut self, lane: usize) {
        let Some(idx) = self.held_notes[lane].take() else {
            return;
        };
        let note = &self.beatmap.notes[idx];
        // Slides: early release from the source lane is expected during finger
        // transition. Put the note back into the slot so the follow-up press
        // on `slide_to` can still find it; miss is decided only at the end of
        // the hold (enforced in process_auto_events).
        if note.slide_to.is_some() {
            self.held_notes[lane] = Some(idx);
            return;
        }
        let note_end = note.time_ms + note.duration_ms;
        let current_ms = self.position_ms_in_track();
        if current_ms + HOLD_RELEASE_GRACE_MS < note_end {
            self.register_miss(idx);
        }
    }

    /// Pressing the `slide_to` lane while its source hold is in progress
    /// completes the slide instantly: the lead note has been held through the
    /// required transition. Returns true when the press consumed an active
    /// slide so the caller skips regular hit detection.
    fn try_slide_complete(&mut self, lane: usize, current_ms: u64) -> bool {
        for src in 0..5 {
            let Some(idx) = self.held_notes[src] else {
                continue;
            };
            let note = &self.beatmap.notes[idx];
            let Some(target) = note.slide_to else {
                continue;
            };
            if target as usize != lane {
                continue;
            }
            // Slide press is valid from the moment the hold starts through a
            // small grace past its end — missing slides leak to process_auto_events.
            let valid_until = note.time_ms + note.duration_ms + HitJudge::MISS_MS;
            if current_ms < note.time_ms || current_ms > valid_until {
                continue;
            }
            self.hit_notes[idx] = true;
            self.held_notes[src] = None;
            self.held_notes[lane] = None;
            self.lane_burst[lane] = LANE_BURST_FRAMES;
            self.aberration_frames = ABERRATION_FRAMES_ON_PERFECT;
            self.register_judgement(Judgement::Perfect, lane);
            self.check_milestone();
            if let Some(area) = self.last_render_area {
                self.spawn_hit_particles(lane, Judgement::Perfect, area);
            }
            return true;
        }
        false
    }

    fn find_closest_note(&self, lane: usize, current_ms: u64) -> Option<(usize, u64)> {
        let mut best: Option<(usize, u64)> = None;
        for (i, note) in self.beatmap.notes.iter().enumerate() {
            if self.hit_notes[i] || note.lane as usize != lane {
                continue;
            }
            // Skip a note that's already held on any lane — covers both plain
            // holds and slides where the source slot stays filled through the
            // transition.
            if (0..5).any(|l| self.held_notes[l] == Some(i)) {
                continue;
            }
            let diff = (note.time_ms as i64 - current_ms as i64).unsigned_abs();
            if diff <= HitJudge::GOOD_MS && best.is_none_or(|(_, d)| diff < d) {
                best = Some((i, diff));
            }
        }
        best
    }

    fn tick_visual_timers(&mut self) {
        for v in &mut self.hit_flash {
            *v = v.saturating_sub(1);
        }
        for v in &mut self.lane_burst {
            *v = v.saturating_sub(1);
        }
        if self.judgement_timer > 0 {
            self.judgement_timer -= 1;
            self.judgement_elapsed = self.judgement_elapsed.saturating_add(1);
            if self.judgement_timer == 0 {
                self.state.last_judgement = None;
                self.judgement_elapsed = 0;
            }
        }
        if self.shake_frames > 0 {
            self.shake_frames -= 1;
        }
        if self.aberration_frames > 0 {
            self.aberration_frames -= 1;
        }
        if let Some(m) = self.milestone.as_mut() {
            m.timer = m.timer.saturating_sub(1);
            m.elapsed = m.elapsed.saturating_add(1);
            if m.timer == 0 {
                self.milestone = None;
            }
        }
    }

    fn advance_stars(&mut self) {
        let Some(area) = self.last_render_area else {
            return;
        };
        let h_px = (area.height.saturating_sub(3) as i32) * 2;
        let w = area.width.max(1);
        let speed_mult = self.scroll_speed as f32;
        for s in &mut self.stars {
            s.y_px += s.speed * speed_mult;
            if s.y_px >= h_px as f32 {
                s.y_px = 0.0;
                self.shake_seed = self
                    .shake_seed
                    .wrapping_mul(1664525)
                    .wrapping_add(1013904223);
                s.x = (self.shake_seed & 0xFFFF) as f32 % w as f32 + area.x as f32;
            } else if s.x < area.x as f32 || s.x >= (area.x + w) as f32 {
                s.x = (s.x as u32 % w as u32) as f32 + area.x as f32;
            }
        }
    }

    fn advance_particles(&mut self) {
        for p in &mut self.particles {
            p.step();
        }
        self.particles.retain(|p| p.life > 0);
    }

    fn update_spectrum(&mut self, current_ms: u64) {
        let sample_pos = (current_ms as f64 / 1000.0 * self.sample_rate as f64) as usize;
        if sample_pos + SPECTRUM_CHUNK > self.samples.len() {
            return;
        }
        self.analyzer
            .process(&self.samples[sample_pos..sample_pos + SPECTRUM_CHUNK]);
        if let Ok(spectrum) = self.analyzer.spectrum.lock() {
            self.spectrum = SpectrumData {
                bands: spectrum.bands.clone(),
                energy: spectrum.energy,
            };
        }
    }

    fn check_finish_conditions(&mut self, current_ms: u64) {
        if self.practice.is_some() {
            // Practice never auto-finishes — the user exits via Esc + Q.
            return;
        }
        if self.audio.is_finished() || current_ms > self.beatmap.song.duration_ms + 2000 {
            self.finished = true;
        }
        if self.health_enabled && self.state.is_dead() {
            self.finished = true;
        }
    }

    fn check_milestone(&mut self) {
        let combo = self.state.combo;
        for &(threshold, label) in COMBO_MILESTONES {
            if combo >= threshold && self.last_milestone < threshold {
                self.last_milestone = threshold;
                self.milestone = Some(MilestoneSplash {
                    text: format!("{} COMBO  {}", threshold, label),
                    timer: MILESTONE_DISPLAY_FRAMES,
                    elapsed: 0,
                });
                self.audio
                    .play_sfx(sfx::milestone_ding(), sfx::SFX_SAMPLE_RATE, self.sfx_volume);
                return;
            }
        }
    }

    fn beat_pulse(&self) -> f32 {
        let bpm = self.beatmap.song.bpm.max(60) as f32;
        let beat_ms = 60_000.0 / bpm;
        let phase = (self.position_ms_in_track() as f32 % beat_ms) / beat_ms;
        let env = (1.0 - phase).powf(4.0);
        (env * (0.5 + 0.5 * self.spectrum.energy)).clamp(0.0, 1.0)
    }

    fn spawn_hit_particles(&mut self, lane: usize, judgement: Judgement, area: Rect) {
        let count = match judgement {
            Judgement::Perfect => 10,
            Judgement::Great => 6,
            Judgement::Good => 3,
            Judgement::Miss => return,
        };
        let highway_h = area.height.saturating_sub(3);
        let lane_w = area.width / 5;
        let lane_cx = area.x + lane as u16 * lane_w + lane_w / 2;
        let hit_py = (highway_h as i32) * 2 - 1;
        let palette = crate::ui::theme::active();
        let color = match judgement {
            Judgement::Perfect => palette.particle[0],
            Judgement::Great => palette.particle[1],
            _ => palette.particle[2],
        };
        for i in 0..count {
            let angle = (i as f32 / count as f32) * std::f32::consts::PI + std::f32::consts::PI;
            let speed = 0.6 + (i as f32 * 0.13).sin().abs() * 0.8;
            self.particles.push(Particle {
                x: lane_cx as f32 + angle.cos() * 0.5,
                y_px: hit_py as f32,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed - 0.3,
                life: 14,
                max_life: 14,
                color,
            });
        }
    }

    fn compute_shake(&mut self) -> i16 {
        if self.shake_frames == 0 {
            return 0;
        }
        self.shake_seed = self
            .shake_seed
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        let amp = (self.shake_frames as i16).min(3);
        ((self.shake_seed >> 16) as i16 % (2 * amp + 1)) - amp
    }

    fn draw_milestone_splash(&self, buf: &mut Buffer, area: Rect) {
        let Some(splash) = &self.milestone else {
            return;
        };
        let elapsed = splash.elapsed as f32;
        let max_life = MILESTONE_DISPLAY_FRAMES as f32;
        let progress = (elapsed / max_life).clamp(0.0, 1.0);
        let scale = if elapsed < 5.0 { elapsed / 5.0 } else { 1.0 };
        if scale <= 0.0 {
            return;
        }
        let alpha = if progress < 0.7 {
            1.0
        } else {
            1.0 - (progress - 0.7) / 0.3
        };
        let brightness = (alpha * 255.0) as u8;
        let accent = (alpha * 200.0) as u8;

        let visible: String = splash
            .text
            .chars()
            .take((splash.text.len() as f32 * scale).ceil() as usize)
            .collect();
        let tw = visible.chars().count() as u16;
        let tx = (area.x + area.width / 2).saturating_sub(tw / 2);
        let ty = area.y + area.height / 2 - 2;
        buf.set_string(
            tx,
            ty,
            &visible,
            Style::default()
                .fg(Color::Rgb(brightness, brightness, accent))
                .bold(),
        );
    }
}

fn apply_shake(mid_area: &mut Rect, parent: Rect, shake_dx: i16) {
    if shake_dx == 0 {
        return;
    }
    if shake_dx > 0 {
        mid_area.x = mid_area
            .x
            .saturating_add(shake_dx as u16)
            .min(parent.x + parent.width.saturating_sub(mid_area.width));
    } else {
        mid_area.x = mid_area.x.saturating_sub((-shake_dx) as u16);
    }
}

fn draw_pause_overlay(buf: &mut Buffer, area: Rect) {
    let y = area.y + area.height / 2;
    let pause_text = "PAUSED";
    let x = area.x + (area.width.saturating_sub(pause_text.len() as u16)) / 2;
    buf.set_string(x, y, pause_text, Style::default().fg(Color::White).bold());

    let hint = "ESC: Resume   Q: Quit to songs";
    let x = area.x + (area.width.saturating_sub(hint.len() as u16)) / 2;
    buf.set_string(
        x,
        y + 1,
        hint,
        Style::default().fg(Color::Rgb(100, 100, 100)),
    );
}

/// Downsample raw samples into `buckets` peak-amplitude entries in `[0..=1]`.
/// Each bucket is the max |sample| over its source range, renormalised by the
/// global peak so quiet tracks still fill the highway. Returns empty if the
/// input is shorter than `buckets`.
fn downsample_waveform(samples: &[f32], buckets: usize) -> Vec<f32> {
    if samples.len() < buckets || buckets == 0 {
        return Vec::new();
    }
    let mut out = vec![0.0_f32; buckets];
    let mut peak = 0.0_f32;
    let step = samples.len() as f32 / buckets as f32;
    for (i, slot) in out.iter_mut().enumerate() {
        let start = (i as f32 * step) as usize;
        let end = (((i + 1) as f32) * step) as usize;
        let end = end.min(samples.len()).max(start + 1);
        let mut local_peak = 0.0_f32;
        for &s in &samples[start..end] {
            let a = s.abs();
            if a > local_peak {
                local_peak = a;
            }
        }
        *slot = local_peak;
        if local_peak > peak {
            peak = local_peak;
        }
    }
    if peak > 1e-6 {
        for v in &mut out {
            *v = (*v / peak).clamp(0.0, 1.0);
        }
    }
    out
}

fn init_stars() -> Vec<Star> {
    let mut stars = Vec::with_capacity(STAR_COUNT);
    let mut rng: u32 = 0xBADC0FFE;
    for _ in 0..STAR_COUNT {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let x = ((rng >> 8) & 0xFF) as f32 / 255.0;
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let y = ((rng >> 8) & 0xFF) as f32 / 255.0;
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let bright = 60 + ((rng >> 8) & 0x3F) as u8;
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let speed = 0.3 + ((rng >> 8) & 0x7F) as f32 / 64.0;
        stars.push(Star {
            x: x * 200.0,
            y_px: y * 100.0,
            speed,
            brightness: bright,
        });
    }
    stars
}
