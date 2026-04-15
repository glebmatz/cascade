use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::highway::VisibleNote;

const LANE_COUNT: usize = 5;

pub struct HighwayWidget<'a> {
    pub notes: &'a [VisibleNote],
    pub lane_labels: [&'a str; LANE_COUNT],
    pub hit_flash: [u8; LANE_COUNT],
}

impl<'a> HighwayWidget<'a> {
    pub fn new(notes: &'a [VisibleNote]) -> Self {
        Self {
            notes,
            lane_labels: ["D", "F", "_", "J", "K"],
            hit_flash: [0; LANE_COUNT],
        }
    }

    pub fn with_hit_flash(mut self, flash: [u8; LANE_COUNT]) -> Self {
        self.hit_flash = flash;
        self
    }

    /// Map lane + vertical position to terminal X coordinate
    fn lane_x(&self, lane: usize, position: f64, area: Rect) -> u16 {
        let center_x = area.x as f64 + area.width as f64 / 2.0;
        let lane_offset = lane as f64 - 2.0; // -2, -1, 0, 1, 2

        // Wider spacing for better visibility
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

        let highway_height = area.height.saturating_sub(2);

        // Draw perspective walls and lane guides
        for row in 0..highway_height {
            let position = 1.0 - (row as f64 / highway_height as f64);
            let y = area.y + row;

            let brightness = ((1.0 - position) * 100.0).clamp(15.0, 100.0) as u8;
            let wall_style = Style::default().fg(Color::Rgb(brightness, brightness, brightness));
            let dot_brightness = ((1.0 - position) * 60.0).clamp(10.0, 60.0) as u8;
            let dot_style = Style::default().fg(Color::Rgb(dot_brightness, dot_brightness, dot_brightness));

            // Left wall — single character
            let left_x = self.lane_x(0, position, area).saturating_sub(3);
            if left_x >= area.x && left_x < area.x + area.width {
                buf.set_string(left_x, y, "\\", wall_style);
            }

            // Right wall — single character
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

        // Draw notes
        for note in self.notes {
            let is_hold = note.duration_ms > 0;

            // Draw hold note trail first (behind the note head)
            if is_hold && note.end_position > note.position {
                let start_row = ((1.0 - note.end_position.clamp(0.0, 1.0)) * highway_height as f64) as u16;
                let end_row = ((1.0 - note.position.clamp(0.0, 1.0)) * highway_height as f64) as u16;

                for row in start_row..=end_row {
                    let y = area.y + row.min(highway_height.saturating_sub(1));
                    let pos = 1.0 - (row as f64 / highway_height as f64);
                    let cx = self.lane_x(note.lane as usize, pos, area);
                    if cx >= area.x && cx < area.x + area.width && y >= area.y && y < area.y + area.height {
                        let brightness = ((1.0 - pos) * 120.0 + 40.0).min(160.0) as u8;
                        buf.set_string(cx, y, "|", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
                    }
                }
            }

            // Draw note head
            if note.position < -0.05 || note.position > 1.0 {
                continue;
            }
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

        // Hit zone line
        let hit_y = area.y + highway_height;
        if hit_y < area.y + area.height {
            // Draw hit zone with lane receptors
            let left_wall = self.lane_x(0, 0.0, area).saturating_sub(4);
            let right_wall = self.lane_x(4, 0.0, area) + 5;
            for x in left_wall..right_wall.min(area.x + area.width) {
                if x >= area.x {
                    buf.set_string(x, hit_y, "-", Style::default().fg(Color::Rgb(60, 60, 60)));
                }
            }

            // Lane receptors — wider
            for (i, label) in self.lane_labels.iter().enumerate() {
                let cx = self.lane_x(i, 0.0, area);
                let receptor = format!("[{}]", label);
                let w = receptor.chars().count() as u16;
                let x = cx.saturating_sub(w / 2);
                if x >= area.x && x + w < area.x + area.width {
                    let style = if self.hit_flash[i] > 0 {
                        Style::default().fg(Color::White).bold()
                    } else {
                        Style::default().fg(Color::Rgb(120, 120, 120))
                    };
                    buf.set_string(x, hit_y, &receptor, style);
                }
            }
        }
    }
}
