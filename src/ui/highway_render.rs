use ratatui::prelude::*;
use ratatui::widgets::Widget;

use crate::game::effects::{Particle, Star};
use crate::game::highway::VisibleNote;
use crate::game::hit_judge::Judgement;
use crate::game::modifiers::{Modifier, Mods};
use crate::ui::color::{Rgb, add as color_add, mul as color_mul, smoothstep};
use crate::ui::pixel_buffer::PixelBuffer;
use crate::ui::theme;

const LANE_COUNT: usize = 5;
const NOTE_PX_HEIGHT: i32 = 3;
const ANTICIPATION_MS: f64 = 250.0;

pub struct HighwayWidget<'a> {
    pub notes: &'a [VisibleNote],
    pub lane_labels: [&'a str; LANE_COUNT],
    pub hit_flash: [u8; LANE_COUNT],
    pub lane_burst: [u8; LANE_COUNT],
    pub last_judgement: Option<Judgement>,
    pub judgement_timer: u8,
    pub judgement_elapsed: u8,
    pub particles: &'a [Particle],
    pub stars: &'a [Star],
    pub spectrum_bands: &'a [f32],
    pub energy: f32,
    pub beat_pulse: f32,
    pub combo: u32,
    pub current_ms: u64,
    pub look_ahead_ms: f64,
    pub mods: Mods,
    /// Chromatic aberration strength in 0..=1. Peaks on a Perfect hit and
    /// decays over a few frames. Splits R/B channels ±1px horizontally.
    pub aberration: f32,
}

impl<'a> HighwayWidget<'a> {
    pub fn new(notes: &'a [VisibleNote]) -> Self {
        Self {
            notes,
            lane_labels: ["D", "F", "_", "J", "K"],
            hit_flash: [0; LANE_COUNT],
            lane_burst: [0; LANE_COUNT],
            last_judgement: None,
            judgement_timer: 0,
            judgement_elapsed: 0,
            particles: &[],
            stars: &[],
            spectrum_bands: &[],
            energy: 0.0,
            beat_pulse: 0.0,
            combo: 0,
            current_ms: 0,
            look_ahead_ms: 2000.0,
            mods: Mods::new(),
            aberration: 0.0,
        }
    }

    pub fn with_mods(mut self, mods: Mods) -> Self {
        self.mods = mods;
        self
    }

    pub fn with_aberration(mut self, v: f32) -> Self {
        self.aberration = v.clamp(0.0, 1.0);
        self
    }

    pub fn with_hit_flash(mut self, flash: [u8; LANE_COUNT]) -> Self {
        self.hit_flash = flash;
        self
    }
    pub fn with_lane_burst(mut self, burst: [u8; LANE_COUNT]) -> Self {
        self.lane_burst = burst;
        self
    }
    pub fn with_judgement(mut self, j: Option<Judgement>, timer: u8, elapsed: u8) -> Self {
        self.last_judgement = j;
        self.judgement_timer = timer;
        self.judgement_elapsed = elapsed;
        self
    }
    pub fn with_particles(mut self, p: &'a [Particle]) -> Self {
        self.particles = p;
        self
    }
    pub fn with_stars(mut self, s: &'a [Star]) -> Self {
        self.stars = s;
        self
    }
    pub fn with_spectrum(mut self, bands: &'a [f32]) -> Self {
        self.spectrum_bands = bands;
        self
    }
    pub fn with_energy(mut self, e: f32) -> Self {
        self.energy = e;
        self
    }
    pub fn with_beat_pulse(mut self, p: f32) -> Self {
        self.beat_pulse = p;
        self
    }
    pub fn with_combo(mut self, c: u32) -> Self {
        self.combo = c;
        self
    }
    pub fn with_timing(mut self, current_ms: u64, look_ahead_ms: f64) -> Self {
        self.current_ms = current_ms;
        self.look_ahead_ms = look_ahead_ms;
        self
    }

    fn lane_rect(&self, lane: usize, row: u16, highway_height: u16, area: Rect) -> (u16, u16) {
        let t = row as f64 / highway_height.max(1) as f64;
        let squeeze = 0.78 + 0.22 * t;
        let total_w = area.width as f64 * squeeze;
        let lane_w = (total_w / LANE_COUNT as f64).max(3.0);
        let highway_start = (area.width as f64 - total_w) / 2.0 + area.x as f64;
        ((highway_start + lane as f64 * lane_w) as u16, lane_w as u16)
    }
}

impl<'a> Widget for HighwayWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 || area.height < 8 {
            return;
        }
        let highway_height = area.height.saturating_sub(3);
        if highway_height == 0 {
            return;
        }
        let height_px = highway_height as i32 * 2;

        // Snapshot the active theme once per frame so a mid-frame switch can't
        // produce inconsistent palettes within a single draw call.
        let theme = theme::active();

        let mut px = PixelBuffer::new(area.width, height_px);
        let local_x = |ax: u16| -> u16 { ax.saturating_sub(area.x) };
        let combo_heat = ((self.combo as f32 / 50.0).min(1.0)).powf(0.7);

        self.draw_starfield(&mut px, &local_x);
        self.draw_lane_backgrounds(&mut px, &local_x, area, highway_height, combo_heat, &theme);
        self.draw_lane_bursts(&mut px, &local_x, area, highway_height, &theme);
        self.draw_hold_trails(&mut px, &local_x, area, highway_height, &theme);
        self.draw_notes(&mut px, &local_x, area, highway_height, &theme);
        self.draw_particles(&mut px, &local_x);
        apply_vignette(&mut px, area.width, height_px);
        if self.mods.contains(Modifier::Flashlight) {
            apply_flashlight(&mut px, area.width, height_px);
        }
        if self.aberration > 0.0 {
            apply_aberration(&mut px, area.width, height_px, self.aberration);
        }
        flush_pixels(buf, &px, area, highway_height);

        if !self.spectrum_bands.is_empty() {
            self.draw_spectrum_margins(buf, area, highway_height);
        }
        self.draw_hit_zone(buf, area, highway_height, &theme);
        self.draw_judgement_feedback(buf, area, highway_height, &theme);
    }
}

impl<'a> HighwayWidget<'a> {
    fn draw_starfield(&self, px: &mut PixelBuffer, local_x: &impl Fn(u16) -> u16) {
        for star in self.stars {
            if star.y_px < 0.0 || star.y_px >= px.height_px as f32 {
                continue;
            }
            let cx = star.x as u16;
            let b = star.brightness;
            let color = (b, b, (b as u16 + 20).min(200) as u8);
            px.blend(local_x(cx), star.y_px as i32, color, 0.55);
        }
    }

    fn draw_lane_backgrounds(
        &self,
        px: &mut PixelBuffer,
        local_x: &impl Fn(u16) -> u16,
        area: Rect,
        highway_height: u16,
        combo_heat: f32,
        theme: &theme::Theme,
    ) {
        let height_px = highway_height as i32 * 2;
        for py in 0..height_px {
            let row = (py / 2) as u16;
            let t = py as f32 / height_px as f32;
            let energy_mul = 1.0 + self.energy * 0.22 + self.beat_pulse * 0.12;

            for lane in 0..LANE_COUNT {
                let (lx, lw) = self.lane_rect(lane, row, highway_height, area);
                let base_intensity = (t * 0.09 + 0.02) * energy_mul;
                let bg = color_mul(theme.lane_colors[lane], base_intensity);
                let heat = color_mul(theme.combo_heat, combo_heat * 0.06 * t);
                let color = color_add(bg, heat);

                for cx in lx..lx + lw {
                    if cx < area.x + area.width {
                        let cur = px.get(local_x(cx), py);
                        let blended = (
                            ((cur.0 as f32) * 0.25 + color.0 as f32 * 0.85).min(255.0) as u8,
                            ((cur.1 as f32) * 0.25 + color.1 as f32 * 0.85).min(255.0) as u8,
                            ((cur.2 as f32) * 0.25 + color.2 as f32 * 0.85).min(255.0) as u8,
                        );
                        px.set(local_x(cx), py, blended);
                    }
                }

                let sep_x = lx + lw;
                if sep_x < area.x + area.width {
                    let sep_v = ((t * 35.0) as u8).saturating_add(10);
                    px.set(local_x(sep_x), py, (sep_v, sep_v, sep_v));
                }
            }
        }
    }

    fn draw_lane_bursts(
        &self,
        px: &mut PixelBuffer,
        local_x: &impl Fn(u16) -> u16,
        area: Rect,
        highway_height: u16,
        theme: &theme::Theme,
    ) {
        let height_px = highway_height as i32 * 2;
        for lane in 0..LANE_COUNT {
            let burst = self.lane_burst[lane];
            if burst == 0 {
                continue;
            }
            let strength = burst as f32 / 8.0;
            let color = color_mul(theme.lane_colors[lane], 0.8);
            let streak_len = (height_px as f32 * (0.25 + 0.3 * strength)) as i32;
            for dy in 0..streak_len {
                let py = height_px - 1 - dy;
                let fade = (1.0 - dy as f32 / streak_len as f32).powf(1.5) * strength;
                let row = (py / 2) as u16;
                let (lx, lw) = self.lane_rect(lane, row, highway_height, area);
                for cx in lx..lx + lw {
                    if cx < area.x + area.width {
                        px.add(local_x(cx), py, color, 0.35 * fade);
                    }
                }
            }
        }
    }

    fn draw_hold_trails(
        &self,
        px: &mut PixelBuffer,
        local_x: &impl Fn(u16) -> u16,
        area: Rect,
        highway_height: u16,
        theme: &theme::Theme,
    ) {
        let height_px = highway_height as i32 * 2;
        let hidden = self.mods.contains(Modifier::Hidden);
        for note in self.notes {
            if note.duration_ms == 0 || note.end_position <= note.position {
                continue;
            }
            // Hidden mod: only render the part of the trail in the bottom 35%.
            let visible_top = if hidden { 0.35_f64 } else { 1.0 };
            let trail_top = note.end_position.min(visible_top);
            if trail_top <= note.position {
                continue;
            }
            let py_top = ((1.0 - trail_top.clamp(0.0, 1.0)) * height_px as f64) as i32;
            let py_bot = ((1.0 - note.position.clamp(0.0, 1.0)) * height_px as f64) as i32;
            let src_lane = note.lane as usize;
            let dst_lane = note.slide_to.map(|l| l as usize).unwrap_or(src_lane);
            let is_slide = dst_lane != src_lane;
            // Slides tint toward the target lane color to preview the transition.
            let color_base = if is_slide {
                let src_c = theme.lane_colors[src_lane % LANE_COUNT];
                let dst_c = theme.lane_colors[dst_lane % LANE_COUNT];
                (
                    ((src_c.0 as u16 + dst_c.0 as u16) / 2) as u8,
                    ((src_c.1 as u16 + dst_c.1 as u16) / 2) as u8,
                    ((src_c.2 as u16 + dst_c.2 as u16) / 2) as u8,
                )
            } else {
                theme.lane_colors[src_lane % LANE_COUNT]
            };
            let span_px = (py_bot - py_top).max(1) as f32;

            for py in py_top.max(0)..=py_bot.min(height_px - 1) {
                let row = (py / 2) as u16;
                // Interpolation from source (bottom, py_bot) up to target (top, py_top).
                // Smoothstep so the slide reads as an arc rather than a linear diagonal.
                let t_linear = ((py_bot - py) as f32 / span_px).clamp(0.0, 1.0);
                let t = crate::ui::color::smoothstep(0.0, 1.0, t_linear);
                let (src_lx, src_lw) = self.lane_rect(src_lane, row, highway_height, area);
                let (dst_lx, dst_lw) = self.lane_rect(dst_lane, row, highway_height, area);
                let src_cx = src_lx as f32 + src_lw as f32 * 0.5;
                let dst_cx = dst_lx as f32 + dst_lw as f32 * 0.5;
                let cx = src_cx * (1.0 - t) + dst_cx * t;
                let center = cx as u16;

                let intensity = (py as f32 / height_px as f32) * 0.55 + 0.45;
                let color = color_mul(color_base, intensity);

                px.blend(local_x(center), py, color, 0.9);
                if center > area.x {
                    px.blend(local_x(center - 1), py, color, 0.35);
                }
                if center + 1 < area.x + area.width {
                    px.blend(local_x(center + 1), py, color, 0.35);
                }
            }

            // Slide tail: small chevron/arrow hint at the target-lane top of the
            // trail so the player can read the direction at a glance.
            if is_slide && py_top >= 0 && !hidden {
                let row = (py_top / 2) as u16;
                let (lx, lw) = self.lane_rect(dst_lane, row, highway_height, area);
                let cx = lx + lw / 2;
                let dst_c = theme.lane_colors[dst_lane % LANE_COUNT];
                let tip_color = color_mul(dst_c, 1.0);
                // Two-pixel-wide dot on the target lane axis.
                for dx in -1..=1i32 {
                    let x = cx as i32 + dx;
                    if x >= area.x as i32 && x < (area.x + area.width) as i32 {
                        px.blend(local_x(x as u16), py_top, tip_color, 0.85);
                    }
                }
            }
        }
    }

    fn draw_notes(
        &self,
        px: &mut PixelBuffer,
        local_x: &impl Fn(u16) -> u16,
        area: Rect,
        highway_height: u16,
        theme: &theme::Theme,
    ) {
        let height_px = highway_height as i32 * 2;
        let hidden = self.mods.contains(Modifier::Hidden);
        for note in self.notes {
            if note.position < -0.1 || note.position > 1.05 {
                continue;
            }
            // Hidden mod: notes only become visible in the bottom 35% of the highway.
            if hidden && note.position > 0.35 {
                continue;
            }
            let approach = smoothstep(0.5, 0.0, note.position as f32);
            let center_py = ((1.0 - note.position) * height_px as f64) as i32;
            let row = (center_py / 2).clamp(0, highway_height as i32 - 1) as u16;
            let (lx, lw) = self.lane_rect(note.lane as usize, row, highway_height, area);
            let lane_color = theme.lane_colors[note.lane as usize % LANE_COUNT];

            let note_w = lw.saturating_sub(1).max(1);
            let note_x = lx + lw.saturating_sub(note_w) / 2;

            for dy in 0..NOTE_PX_HEIGHT {
                let py = center_py + dy - NOTE_PX_HEIGHT / 2;
                let base_i = match dy {
                    0 => 0.65,
                    1 => 1.0,
                    _ => 0.85,
                };
                let intensity = (base_i + 0.25 * approach).min(1.25);
                let color = color_mul(lane_color, intensity);
                for cx in note_x..note_x + note_w {
                    px.blend(local_x(cx), py, color, 1.0);
                }
            }

            let halo = color_mul(lane_color, 0.55 + 0.35 * approach);
            for &dy in &[-NOTE_PX_HEIGHT / 2 - 1, NOTE_PX_HEIGHT / 2 + 1] {
                let py = center_py + dy;
                for cx in note_x..note_x + note_w {
                    px.blend(local_x(cx), py, halo, 0.35 + 0.25 * approach);
                }
            }
            for dy in -NOTE_PX_HEIGHT / 2..=NOTE_PX_HEIGHT / 2 {
                let py = center_py + dy;
                if note_x > area.x {
                    px.blend(local_x(note_x - 1), py, halo, 0.4);
                }
                let right = note_x + note_w;
                if right < area.x + area.width {
                    px.blend(local_x(right), py, halo, 0.4);
                }
            }
        }
    }

    fn draw_particles(&self, px: &mut PixelBuffer, local_x: &impl Fn(u16) -> u16) {
        for p in self.particles {
            if p.life == 0 {
                continue;
            }
            let alpha = p.alpha();
            let color = color_mul(p.color, 0.6 + 0.4 * alpha);
            px.add(local_x(p.x as u16), p.y_px as i32, color, alpha * 0.8);
        }
    }

    fn draw_spectrum_margins(&self, buf: &mut Buffer, area: Rect, highway_height: u16) {
        let (left_x, _) = self.lane_rect(0, highway_height, highway_height, area);
        let (right_x, right_w) =
            self.lane_rect(LANE_COUNT - 1, highway_height, highway_height, area);
        let inner_right = right_x + right_w;
        let left_margin = left_x.saturating_sub(area.x);
        let right_margin = (area.x + area.width).saturating_sub(inner_right);

        if left_margin >= 3 {
            draw_spectrum(
                buf,
                area.x,
                area.y,
                left_margin - 1,
                highway_height,
                self.spectrum_bands,
                true,
            );
        }
        if right_margin >= 3 {
            draw_spectrum(
                buf,
                inner_right + 1,
                area.y,
                right_margin.saturating_sub(2),
                highway_height,
                self.spectrum_bands,
                false,
            );
        }
    }

    fn draw_hit_zone(
        &self,
        buf: &mut Buffer,
        area: Rect,
        highway_height: u16,
        theme: &theme::Theme,
    ) {
        let hit_y = area.y + highway_height;
        if hit_y >= area.y + area.height {
            return;
        }

        let anticipation = self.anticipation_per_lane();

        for (i, label) in self.lane_labels.iter().enumerate() {
            let (lx, lw) = self.lane_rect(i, highway_height, highway_height, area);
            let base_color = theme.lane_colors[i];
            let flash = self.hit_flash[i];
            let pulse_intensity = if flash > 0 {
                (flash as f32 / 8.0).min(1.0)
            } else {
                let ambient = 0.28 + self.beat_pulse * 0.25 + self.energy * 0.06;
                (ambient + anticipation[i] * 0.7).min(1.1)
            };
            let bg_color = color_mul(base_color, pulse_intensity);
            let fg_color = if flash > 0 {
                (255, 255, 255)
            } else if anticipation[i] > 0.5 {
                color_mul(base_color, 1.2)
            } else {
                color_mul(base_color, 0.75)
            };

            let bg = Color::Rgb(bg_color.0, bg_color.1, bg_color.2);
            for cx in lx..lx + lw {
                if cx < area.x + area.width {
                    buf.set_string(cx, hit_y, " ", Style::default().bg(bg));
                }
            }
            let label_x = lx + lw / 2;
            if label_x < area.x + area.width {
                buf.set_string(
                    label_x,
                    hit_y,
                    label,
                    Style::default()
                        .fg(Color::Rgb(fg_color.0, fg_color.1, fg_color.2))
                        .bg(bg)
                        .bold(),
                );
            }
        }
    }

    fn anticipation_per_lane(&self) -> [f32; LANE_COUNT] {
        let anticipation_pos = (ANTICIPATION_MS / self.look_ahead_ms).min(1.0);
        let mut out = [0.0_f32; LANE_COUNT];
        for note in self.notes {
            let lane = note.lane as usize;
            if lane >= LANE_COUNT || note.position < 0.0 || note.position > anticipation_pos {
                continue;
            }
            let strength = (1.0 - note.position as f32 / anticipation_pos as f32).clamp(0.0, 1.0);
            if strength > out[lane] {
                out[lane] = strength;
            }
        }
        out
    }

    fn draw_judgement_feedback(
        &self,
        buf: &mut Buffer,
        area: Rect,
        highway_height: u16,
        theme: &theme::Theme,
    ) {
        let feedback_y = area.y + highway_height + 1;
        if feedback_y >= area.y + area.height {
            return;
        }
        let (Some(judgement), timer) = (self.last_judgement, self.judgement_timer) else {
            return;
        };
        if timer == 0 {
            return;
        }

        let elapsed = self.judgement_elapsed as f32;
        let progress = (elapsed / 30.0).clamp(0.0, 1.0);
        let scale = if elapsed < 6.0 {
            (elapsed / 6.0).min(1.0)
        } else {
            1.0
        };
        let fade = (1.0 - progress).powf(0.7);

        let (text, base_color) = match judgement {
            Judgement::Perfect => ("PERFECT!", theme.judgement[0]),
            Judgement::Great => ("GREAT", theme.judgement[1]),
            Judgement::Good => ("good", theme.judgement[2]),
            Judgement::Miss => ("miss", theme.judgement[3]),
        };
        let color = color_mul(base_color, fade);
        let tw = (text.len() as f32 * scale).ceil() as u16;
        let tx = (area.x + area.width / 2).saturating_sub(tw / 2);
        let y_off = (elapsed / 12.0) as u16;
        let y = feedback_y.saturating_sub(y_off);
        if y < area.y {
            return;
        }

        let visible: String = if scale >= 1.0 {
            text.to_string()
        } else {
            text.chars()
                .take((text.len() as f32 * scale).ceil() as usize)
                .collect()
        };
        buf.set_string(
            tx,
            y,
            &visible,
            Style::default()
                .fg(Color::Rgb(color.0, color.1, color.2))
                .bold(),
        );
    }
}

/// Flashlight mod: keep a narrow horizontal band near the bottom (hit zone)
/// fully visible; fade the rest to near-black.
fn apply_flashlight(px: &mut PixelBuffer, width: u16, height_px: i32) {
    // Center of the flashlight: just above the hit zone (~88% down).
    let center_py = (height_px as f32 * 0.88) as i32;
    let band_radius = (height_px as f32 * 0.18).max(4.0);
    for py in 0..height_px {
        let dy = (py - center_py).abs() as f32;
        let visibility = (1.0 - (dy / band_radius)).clamp(0.0, 1.0).powf(1.5);
        if visibility >= 1.0 {
            continue;
        }
        let dim = 0.05 + 0.95 * visibility;
        for x in 0..width {
            let cur = px.get(x, py);
            px.set(x, py, color_mul(cur, dim));
        }
    }
}

fn apply_vignette(px: &mut PixelBuffer, width: u16, height_px: i32) {
    let edge_x = (width as f32 * 0.12).max(3.0);
    let edge_y = (height_px as f32 * 0.08).max(2.0);
    for py in 0..height_px {
        let dy = (py as f32).min((height_px - 1 - py) as f32);
        let vy = (dy / edge_y).min(1.0);
        for x in 0..width {
            let dx = (x as f32).min((width - 1 - x) as f32);
            let vx = (dx / edge_x).min(1.0);
            let v = (vx * vy).powf(0.8);
            if v < 1.0 {
                let cur = px.get(x, py);
                px.set(x, py, color_mul(cur, 0.4 + 0.6 * v));
            }
        }
    }
}

/// Chromatic aberration: shift the R channel one pixel right and the B channel
/// one pixel left, blending with the original by `strength`. Cheap RGB split
/// post-process used to punctuate Perfect hits.
fn apply_aberration(px: &mut PixelBuffer, width: u16, height_px: i32, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 || width < 3 {
        return;
    }
    // Snapshot once; we read from the original buffer and write into `px`.
    let mut rows: Vec<Vec<Rgb>> = Vec::with_capacity(height_px as usize);
    for py in 0..height_px {
        let mut row: Vec<Rgb> = Vec::with_capacity(width as usize);
        for x in 0..width {
            row.push(px.get(x, py));
        }
        rows.push(row);
    }
    for py in 0..height_px {
        let row = &rows[py as usize];
        for x in 0..width {
            let orig = row[x as usize];
            // R shifts right, B shifts left. Out-of-bounds neighbours fall back to the origin pixel.
            let r_src = if x == 0 { orig } else { row[(x - 1) as usize] };
            let b_src = if x + 1 >= width {
                orig
            } else {
                row[(x + 1) as usize]
            };
            let mixed = (
                (orig.0 as f32 * (1.0 - strength) + r_src.0 as f32 * strength) as u8,
                orig.1,
                (orig.2 as f32 * (1.0 - strength) + b_src.2 as f32 * strength) as u8,
            );
            px.set(x, py, mixed);
        }
    }
}

fn flush_pixels(buf: &mut Buffer, px: &PixelBuffer, area: Rect, highway_height: u16) {
    for row in 0..highway_height {
        let y = area.y + row;
        let py_top = (row as i32) * 2;
        let py_bot = py_top + 1;
        for x_local in 0..area.width {
            let top = px.get(x_local, py_top);
            let bot = px.get(x_local, py_bot);
            buf.set_string(
                area.x + x_local,
                y,
                "\u{2580}",
                Style::default()
                    .fg(Color::Rgb(top.0, top.1, top.2))
                    .bg(Color::Rgb(bot.0, bot.1, bot.2)),
            );
        }
    }
}

fn draw_spectrum(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    bands: &[f32],
    flipped: bool,
) {
    if width == 0 || height == 0 || bands.is_empty() {
        return;
    }
    let bars = width.clamp(4, 12);
    let band_count = bands.len();
    let bar_spacing = width / bars;
    if bar_spacing == 0 {
        return;
    }

    for b in 0..bars {
        let band_idx = (b as usize * band_count) / bars.max(1) as usize;
        let level = bands[band_idx.min(band_count - 1)].clamp(0.0, 1.0);
        let bar_h_px = ((level.powf(0.6)) * height as f32 * 2.0) as i32;

        let bar_x = if flipped {
            x + width.saturating_sub((b + 1) * bar_spacing)
        } else {
            x + b * bar_spacing
        };
        if bar_x >= x + width {
            continue;
        }

        let color = spectrum_color(level);

        for row in 0..height {
            let dist_px = bar_h_px - ((height - 1 - row) as i32 * 2);
            let top_filled = dist_px >= 2;
            let bot_filled = dist_px >= 1;
            let top = if top_filled { color } else { (0, 0, 0) };
            let bot = if bot_filled { color } else { (0, 0, 0) };
            buf.set_string(
                bar_x,
                y + row,
                "\u{2580}",
                Style::default()
                    .fg(Color::Rgb(top.0, top.1, top.2))
                    .bg(Color::Rgb(bot.0, bot.1, bot.2)),
            );
        }
    }
}

fn spectrum_color(level: f32) -> Rgb {
    if level < 0.5 {
        let t = level / 0.5;
        (
            (40.0 + 20.0 * t) as u8,
            (120.0 + 100.0 * t) as u8,
            (200.0 * (1.0 - t * 0.3)) as u8,
        )
    } else {
        let t = (level - 0.5) / 0.5;
        (
            (60.0 + 180.0 * t) as u8,
            (220.0 - 60.0 * t) as u8,
            (140.0 * (1.0 - t)) as u8,
        )
    }
}
