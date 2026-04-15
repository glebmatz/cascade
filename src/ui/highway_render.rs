use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::highway::VisibleNote;

const LANE_COUNT: usize = 5;
const NOTE_FAR: &str = "◇";
const NOTE_MID: &str = "◈";
const NOTE_NEAR: &str = "◆";

pub struct HighwayWidget<'a> {
    pub notes: &'a [VisibleNote],
    pub lane_labels: [&'a str; LANE_COUNT],
    pub hit_flash: [u8; LANE_COUNT],
}

impl<'a> HighwayWidget<'a> {
    pub fn new(notes: &'a [VisibleNote]) -> Self {
        Self {
            notes,
            lane_labels: ["D", "F", "▽", "J", "K"],
            hit_flash: [0; LANE_COUNT],
        }
    }

    pub fn with_hit_flash(mut self, flash: [u8; LANE_COUNT]) -> Self {
        self.hit_flash = flash;
        self
    }

    fn lane_x(&self, lane: usize, position: f64, area: Rect) -> u16 {
        let center_x = area.x + area.width / 2;
        let lane_offset = lane as f64 - 2.0;

        let bottom_spacing = (area.width as f64 / 8.0).max(4.0);
        let top_spacing = bottom_spacing * 0.3;

        let spacing = bottom_spacing + (top_spacing - bottom_spacing) * position.clamp(0.0, 1.0);
        (center_x as f64 + lane_offset * spacing) as u16
    }

    fn note_char(position: f64) -> &'static str {
        if position > 0.7 { NOTE_FAR }
        else if position > 0.3 { NOTE_MID }
        else { NOTE_NEAR }
    }

    fn note_style(position: f64) -> Style {
        let brightness = ((1.0 - position) * 255.0).clamp(80.0, 255.0) as u8;
        Style::default().fg(Color::Rgb(brightness, brightness, brightness))
    }
}

impl<'a> Widget for HighwayWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 5 {
            return;
        }

        let highway_height = area.height.saturating_sub(2);

        // Draw lane dividers and walls
        for row in 0..highway_height {
            let position = 1.0 - (row as f64 / highway_height as f64);
            let y = area.y + row;

            // Left wall
            let left_x = self.lane_x(0, position, area).saturating_sub(2);
            if left_x >= area.x && left_x < area.x + area.width {
                let brightness = ((1.0 - position) * 150.0).clamp(30.0, 150.0) as u8;
                buf.set_string(left_x, y, "╲", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
            }

            // Right wall
            let right_x = self.lane_x(4, position, area) + 2;
            if right_x >= area.x && right_x < area.x + area.width {
                let brightness = ((1.0 - position) * 150.0).clamp(30.0, 150.0) as u8;
                buf.set_string(right_x, y, "╱", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
            }

            // Lane dots
            for lane in 0..LANE_COUNT {
                let x = self.lane_x(lane, position, area);
                if x >= area.x && x < area.x + area.width {
                    let brightness = ((1.0 - position) * 80.0).clamp(20.0, 80.0) as u8;
                    buf.set_string(x, y, "·", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
                }
            }
        }

        // Draw notes
        for note in self.notes {
            if note.position < -0.05 || note.position > 1.0 {
                continue;
            }
            let row = ((1.0 - note.position) * highway_height as f64) as u16;
            let y = area.y + row.min(highway_height.saturating_sub(1));
            let x = self.lane_x(note.lane as usize, note.position, area);

            if x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height {
                buf.set_string(x, y, Self::note_char(note.position), Self::note_style(note.position));
            }
        }

        // Hit zone line
        let hit_y = area.y + highway_height;
        if hit_y < area.y + area.height {
            for x in area.x..area.x + area.width {
                buf.set_string(x, hit_y, "━", Style::default().fg(Color::Rgb(60, 60, 60)));
            }

            for (i, label) in self.lane_labels.iter().enumerate() {
                let x = self.lane_x(i, 0.0, area);
                if x >= area.x + 1 && x + 2 < area.x + area.width {
                    let style = if self.hit_flash[i] > 0 {
                        Style::default().fg(Color::White).bold()
                    } else {
                        Style::default().fg(Color::Rgb(140, 140, 140))
                    };
                    buf.set_string(x - 1, hit_y, &format!("[{}]", label), style);
                }
            }
        }
    }
}
