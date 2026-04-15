use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::highway::VisibleNote;
use crate::game::hit_judge::Judgement;

const LANE_COUNT: usize = 5;

pub struct HighwayWidget<'a> {
    pub notes: &'a [VisibleNote],
    pub lane_labels: [&'a str; LANE_COUNT],
    pub hit_flash: [u8; LANE_COUNT],
    pub last_judgement: Option<Judgement>,
    pub judgement_timer: u8,
}

impl<'a> HighwayWidget<'a> {
    pub fn new(notes: &'a [VisibleNote]) -> Self {
        Self {
            notes,
            lane_labels: ["D", "F", "_", "J", "K"],
            hit_flash: [0; LANE_COUNT],
            last_judgement: None,
            judgement_timer: 0,
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

    fn lane_x(&self, lane: usize, position: f64, area: Rect) -> u16 {
        let center_x = area.x as f64 + area.width as f64 / 2.0;
        let lane_offset = lane as f64 - 2.0;

        let bottom_spacing = (area.width as f64 / 6.0).max(6.0);
        let top_spacing = bottom_spacing * 0.25;

        let t = position.clamp(0.0, 1.0);
        let spacing = bottom_spacing + (top_spacing - bottom_spacing) * t;
        (center_x + lane_offset * spacing) as u16
    }

    fn note_str(position: f64) -> &'static str {
        if position > 0.7 { "+" }
        else if position > 0.4 { "=*=" }
        else { "=[#]=" }
    }

    fn note_style(position: f64) -> Style {
        let t = 1.0 - position.clamp(0.0, 1.0);
        let brightness = (t * 200.0 + 55.0).min(255.0) as u8;
        Style::default().fg(Color::Rgb(brightness, brightness, brightness)).bold()
    }
}

impl<'a> Widget for HighwayWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 || area.height < 8 {
            return;
        }

        let highway_height = area.height.saturating_sub(3); // room for hit zone + effect row

        // Draw perspective walls and lane guides
        for row in 0..highway_height {
            let position = 1.0 - (row as f64 / highway_height as f64);
            let y = area.y + row;

            let brightness = ((1.0 - position) * 80.0).clamp(12.0, 80.0) as u8;
            let wall_style = Style::default().fg(Color::Rgb(brightness, brightness, brightness));
            let dot_brightness = ((1.0 - position) * 40.0).clamp(8.0, 40.0) as u8;
            let dot_style = Style::default().fg(Color::Rgb(dot_brightness, dot_brightness, dot_brightness));

            // Left wall
            let left_x = self.lane_x(0, position, area).saturating_sub(3);
            if left_x >= area.x && left_x < area.x + area.width {
                buf.set_string(left_x, y, "\\", wall_style);
            }

            // Right wall
            let right_x = self.lane_x(4, position, area) + 3;
            if right_x >= area.x && right_x < area.x + area.width {
                buf.set_string(right_x, y, "/", wall_style);
            }

            // Lane center dots
            for lane in 0..LANE_COUNT {
                let x = self.lane_x(lane, position, area);
                if x >= area.x && x < area.x + area.width {
                    buf.set_string(x, y, ".", dot_style);
                }
            }
        }

        // Draw hold note trails
        for note in self.notes {
            if note.duration_ms == 0 { continue; }
            if note.end_position <= note.position { continue; }

            let start_row = ((1.0 - note.end_position.clamp(0.0, 1.0)) * highway_height as f64) as u16;
            let end_row = ((1.0 - note.position.clamp(0.0, 1.0)) * highway_height as f64) as u16;

            for row in start_row..=end_row.min(highway_height.saturating_sub(1)) {
                let y = area.y + row;
                let pos = 1.0 - (row as f64 / highway_height as f64);
                let cx = self.lane_x(note.lane as usize, pos, area);
                if cx >= area.x && cx < area.x + area.width && y >= area.y && y < area.y + area.height {
                    let brightness = ((1.0 - pos) * 100.0 + 30.0).min(130.0) as u8;
                    buf.set_string(cx, y, "|", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
                }
            }
        }

        // Draw note heads
        for note in self.notes {
            if note.position < -0.05 || note.position > 1.0 { continue; }

            let row = ((1.0 - note.position) * highway_height as f64) as u16;
            let y = area.y + row.min(highway_height.saturating_sub(1));
            let cx = self.lane_x(note.lane as usize, note.position, area);
            let note_str = Self::note_str(note.position);
            let note_w = note_str.chars().count() as u16;
            let x = cx.saturating_sub(note_w / 2);

            if x >= area.x && x + note_w < area.x + area.width && y >= area.y && y < area.y + area.height {
                buf.set_string(x, y, note_str, Self::note_style(note.position));
            }
        }

        // === HIT ZONE (2 rows) ===
        let hit_y = area.y + highway_height;
        if hit_y + 1 >= area.y + area.height { return; }

        // Row 1: receptor line with lane markers
        let left_wall = self.lane_x(0, 0.0, area).saturating_sub(4);
        let right_wall = self.lane_x(4, 0.0, area) + 5;

        // Base line
        for x in left_wall..right_wall.min(area.x + area.width) {
            if x >= area.x {
                buf.set_string(x, hit_y, "-", Style::default().fg(Color::Rgb(35, 35, 35)));
            }
        }

        // Lane receptors with flash effect
        for (i, label) in self.lane_labels.iter().enumerate() {
            let cx = self.lane_x(i, 0.0, area);
            let flash = self.hit_flash[i];

            if flash > 0 {
                // Active flash — bright receptor + glow around it
                let intensity = (flash as f32 / 8.0 * 255.0) as u8;

                // Glow: brighten the hit line around this lane
                let glow_radius = 3u16;
                for dx in 0..=glow_radius {
                    let glow_b = (intensity as f32 * (1.0 - dx as f32 / glow_radius as f32) * 0.3) as u8;
                    if glow_b > 5 {
                        let style = Style::default().fg(Color::Rgb(glow_b, glow_b, glow_b));
                        if cx + dx < area.x + area.width {
                            buf.set_string(cx + dx, hit_y, "=", style);
                        }
                        if cx >= area.x + dx {
                            buf.set_string(cx - dx, hit_y, "=", style);
                        }
                    }
                }

                // Bright receptor
                let receptor = format!("[{}]", label);
                let w = receptor.chars().count() as u16;
                let x = cx.saturating_sub(w / 2);
                if x >= area.x && x + w < area.x + area.width {
                    buf.set_string(x, hit_y, &receptor,
                        Style::default().fg(Color::Rgb(intensity, intensity, intensity)).bold());
                }

                // Spark effect above receptor (row -1)
                if hit_y > area.y {
                    let spark_y = hit_y - 1;
                    let spark_b = (intensity as f32 * 0.5) as u8;
                    if cx >= area.x && cx < area.x + area.width {
                        buf.set_string(cx, spark_y, "*",
                            Style::default().fg(Color::Rgb(spark_b, spark_b, spark_b)));
                    }
                }
            } else {
                // Idle receptor
                let receptor = format!("[{}]", label);
                let w = receptor.chars().count() as u16;
                let x = cx.saturating_sub(w / 2);
                if x >= area.x && x + w < area.x + area.width {
                    buf.set_string(x, hit_y, &receptor,
                        Style::default().fg(Color::Rgb(60, 60, 60)));
                }
            }
        }

        // Row 2: judgement feedback centered under hit zone
        let feedback_y = hit_y + 1;
        if feedback_y < area.y + area.height {
            if let Some(judgement) = self.last_judgement {
                if self.judgement_timer > 0 {
                    let fade = self.judgement_timer as f32 / 30.0;
                    let (text, base_b) = match judgement {
                        Judgement::Perfect => ("* PERFECT *", 255.0),
                        Judgement::Great =>   ("  GREAT  ", 200.0),
                        Judgement::Good =>    ("  GOOD   ", 140.0),
                        Judgement::Miss =>    ("  MISS   ", 70.0),
                    };
                    let b = (base_b * fade) as u8;
                    let x = (area.x + area.width / 2).saturating_sub(text.len() as u16 / 2);
                    buf.set_string(x, feedback_y, text,
                        Style::default().fg(Color::Rgb(b, b, b)).bold());
                }
            }
        }
    }
}
