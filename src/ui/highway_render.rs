use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::highway::VisibleNote;
use crate::game::hit_judge::Judgement;

const LANE_COUNT: usize = 5;

/// Colors for each lane (muted pastels)
const LANE_COLORS: [(u8, u8, u8); LANE_COUNT] = [
    (180, 80, 80),    // D — red
    (80, 160, 80),    // F — green
    (180, 160, 60),   // _ — yellow
    (80, 120, 200),   // J — blue
    (160, 80, 180),   // K — purple
];

pub struct HighwayWidget<'a> {
    pub notes: &'a [VisibleNote],
    pub lane_labels: [&'a str; LANE_COUNT],
    pub hit_flash: [u8; LANE_COUNT],
    pub last_judgement: Option<Judgement>,
    pub judgement_timer: u8,
    pub particles: &'a [(u16, u16, u8)],
    pub energy: f32,
}

impl<'a> HighwayWidget<'a> {
    pub fn new(notes: &'a [VisibleNote]) -> Self {
        Self {
            notes,
            lane_labels: ["D", "F", "_", "J", "K"],
            hit_flash: [0; LANE_COUNT],
            last_judgement: None,
            judgement_timer: 0,
            particles: &[],
            energy: 0.0,
        }
    }

    pub fn with_hit_flash(mut self, flash: [u8; LANE_COUNT]) -> Self {
        self.hit_flash = flash;
        self
    }

    pub fn with_judgement(mut self, judgement: Option<Judgement>, timer: u8) -> Self {
        self.last_judgement = judgement;
        self.judgement_timer = timer;
        self
    }

    pub fn with_particles(mut self, particles: &'a [(u16, u16, u8)]) -> Self {
        self.particles = particles;
        self
    }

    pub fn with_energy(mut self, energy: f32) -> Self {
        self.energy = energy;
        self
    }

    /// Compute left edge X and width for a lane at a given vertical position.
    /// Slight narrowing at top gives subtle perspective without ugly ASCII walls.
    fn lane_rect(&self, lane: usize, row: u16, highway_height: u16, area: Rect) -> (u16, u16) {
        // Perspective factor: 1.0 at bottom, shrinks toward top
        let t = row as f64 / highway_height.max(1) as f64; // 0=top, 1=bottom
        let squeeze = 0.7 + 0.3 * t; // 0.7 at top, 1.0 at bottom

        let total_lane_width = area.width as f64 * squeeze;
        let lane_w = (total_lane_width / LANE_COUNT as f64).max(3.0);
        let highway_start = (area.width as f64 - total_lane_width) / 2.0 + area.x as f64;

        let x = (highway_start + lane as f64 * lane_w) as u16;
        let w = lane_w as u16;
        (x, w)
    }
}

impl<'a> Widget for HighwayWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 || area.height < 8 {
            return;
        }

        let highway_height = area.height.saturating_sub(3); // hit zone + judgement + 1

        // Draw lane backgrounds (subtle colored columns)
        for row in 0..highway_height {
            let y = area.y + row;
            let t = row as f64 / highway_height as f64; // 0=top, 1=bottom

            for lane in 0..LANE_COUNT {
                let (lx, lw) = self.lane_rect(lane, row, highway_height, area);
                let (cr, cg, cb) = LANE_COLORS[lane];

                // Very subtle background: brighter at bottom, dimmer at top
                // Pulse with energy from spectrum analyzer
                let energy_mult = 1.0 + self.energy * 0.3;
                let intensity = (t * 0.12 + 0.03) as f32 * energy_mult;
                let bg_r = (cr as f32 * intensity).min(50.0) as u8;
                let bg_g = (cg as f32 * intensity).min(50.0) as u8;
                let bg_b = (cb as f32 * intensity).min(50.0) as u8;

                for x in lx..lx + lw {
                    if x < area.x + area.width {
                        buf.set_string(x, y, " ", Style::default().bg(Color::Rgb(bg_r, bg_g, bg_b)));
                    }
                }

                // Lane separator (right edge)
                if lx + lw < area.x + area.width {
                    let sep_b = (t * 30.0 + 10.0) as u8;
                    buf.set_string(lx + lw, y, " ",
                        Style::default().bg(Color::Rgb(sep_b, sep_b, sep_b)));
                }
            }
        }

        // Draw hold note trails
        for note in self.notes {
            if note.duration_ms == 0 || note.end_position <= note.position { continue; }

            let start_row = ((1.0 - note.end_position.clamp(0.0, 1.0)) * highway_height as f64) as u16;
            let end_row = ((1.0 - note.position.clamp(0.0, 1.0)) * highway_height as f64) as u16;
            let (cr, cg, cb) = LANE_COLORS[note.lane as usize % LANE_COUNT];

            for row in start_row..=end_row.min(highway_height.saturating_sub(1)) {
                let y = area.y + row;
                let (lx, lw) = self.lane_rect(note.lane as usize, row, highway_height, area);
                let cx = lx + lw / 2;
                if cx >= area.x && cx < area.x + area.width && y < area.y + area.height {
                    let t = row as f64 / highway_height as f64;
                    let intensity = (t * 0.5 + 0.3) as f32;
                    let r = (cr as f32 * intensity) as u8;
                    let g = (cg as f32 * intensity) as u8;
                    let b = (cb as f32 * intensity) as u8;
                    buf.set_string(cx, y, "|", Style::default().fg(Color::Rgb(r, g, b)));
                }
            }
        }

        // Draw notes
        for note in self.notes {
            if note.position < -0.05 || note.position > 1.0 { continue; }

            let row = ((1.0 - note.position) * highway_height as f64) as u16;
            let y = area.y + row.min(highway_height.saturating_sub(1));
            let (lx, lw) = self.lane_rect(note.lane as usize, row, highway_height, area);
            let (cr, cg, cb) = LANE_COLORS[note.lane as usize % LANE_COUNT];

            // Note = filled block spanning most of lane width
            let t = row as f64 / highway_height as f64; // brightness by position
            let intensity = (t * 0.7 + 0.3) as f32;
            let r = (cr as f32 * intensity).min(255.0) as u8;
            let g = (cg as f32 * intensity).min(255.0) as u8;
            let b = (cb as f32 * intensity).min(255.0) as u8;

            // Note width: narrower at top, wider at bottom
            let note_w = (lw.saturating_sub(2)).max(1);
            let note_x = lx + (lw - note_w) / 2;

            for x in note_x..note_x + note_w {
                if x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height {
                    buf.set_string(x, y, " ", Style::default().bg(Color::Rgb(r, g, b)));
                }
            }
        }

        // === HIT ZONE ===
        let hit_y = area.y + highway_height;
        if hit_y >= area.y + area.height { return; }

        // Draw receptors per lane
        for (i, label) in self.lane_labels.iter().enumerate() {
            let (lx, lw) = self.lane_rect(i, highway_height, highway_height, area);
            let (cr, cg, cb) = LANE_COLORS[i];
            let flash = self.hit_flash[i];

            if flash > 0 {
                // Flash: bright lane color
                let f = flash as f32 / 8.0;
                let r = (cr as f32 * f).min(255.0) as u8;
                let g = (cg as f32 * f).min(255.0) as u8;
                let b = (cb as f32 * f).min(255.0) as u8;

                for x in lx..lx + lw {
                    if x < area.x + area.width {
                        buf.set_string(x, hit_y, " ", Style::default().bg(Color::Rgb(r, g, b)));
                    }
                }
                // Label on top
                let label_x = lx + lw / 2;
                if label_x < area.x + area.width {
                    buf.set_string(label_x, hit_y, label,
                        Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(r, g, b)).bold());
                }
            } else {
                // Idle: dim colored receptor
                let r = (cr as f32 * 0.25) as u8;
                let g = (cg as f32 * 0.25) as u8;
                let b = (cb as f32 * 0.25) as u8;

                for x in lx..lx + lw {
                    if x < area.x + area.width {
                        buf.set_string(x, hit_y, " ", Style::default().bg(Color::Rgb(r, g, b)));
                    }
                }
                let label_x = lx + lw / 2;
                if label_x < area.x + area.width {
                    buf.set_string(label_x, hit_y, label,
                        Style::default().fg(Color::Rgb(cr / 2, cg / 2, cb / 2)).bg(Color::Rgb(r, g, b)));
                }
            }
        }

        // Draw particles
        for &(px, py, lifetime) in self.particles {
            if px >= area.x && px < area.x + area.width && py >= area.y && py < area.y + area.height {
                let brightness = (lifetime as f32 / 10.0 * 255.0).min(255.0) as u8;
                let ch = if lifetime > 5 { "*" } else { "'" };
                buf.set_string(px, py, ch,
                    Style::default().fg(Color::Rgb(brightness, brightness, (brightness as f32 * 0.7) as u8)));
            }
        }

        // Judgement feedback row
        let feedback_y = hit_y + 1;
        if feedback_y < area.y + area.height {
            if let Some(judgement) = self.last_judgement {
                if self.judgement_timer > 0 {
                    let fade = self.judgement_timer as f32 / 30.0;
                    let (text, color) = match judgement {
                        Judgement::Perfect => ("PERFECT", Color::Rgb((255.0 * fade) as u8, (255.0 * fade) as u8, (200.0 * fade) as u8)),
                        Judgement::Great =>   ("GREAT",   Color::Rgb((180.0 * fade) as u8, (220.0 * fade) as u8, (180.0 * fade) as u8)),
                        Judgement::Good =>    ("GOOD",    Color::Rgb((140.0 * fade) as u8, (140.0 * fade) as u8, (140.0 * fade) as u8)),
                        Judgement::Miss =>    ("MISS",    Color::Rgb((120.0 * fade) as u8, (50.0 * fade) as u8, (50.0 * fade) as u8)),
                    };
                    let tw = text.len() as u16;
                    let tx = area.x + area.width / 2 - tw / 2;
                    buf.set_string(tx, feedback_y, text, Style::default().fg(color).bold());
                }
            }
        }
    }
}
