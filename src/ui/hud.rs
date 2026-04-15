use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::state::GameState;

pub struct HudTop<'a> {
    pub state: &'a GameState,
    pub health_enabled: bool,
}

impl<'a> Widget for HudTop<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 { return; }

        let cx = area.x + area.width / 2;

        // Health bar: left side (only if enabled)
        if self.health_enabled {
            let bar_width = 12u16.min(area.width / 4);
            let bar_x = area.x + 1;
            let filled = (self.state.health * bar_width as f64) as u16;
            let hp_color = if self.state.health > 0.5 {
                Color::Rgb(80, 200, 80)
            } else if self.state.health >= 0.25 {
                Color::Rgb(200, 200, 60)
            } else {
                Color::Rgb(200, 60, 60)
            };
            buf.set_string(bar_x, area.y, "[", Style::default().fg(Color::Rgb(100, 100, 100)));
            for i in 0..bar_width {
                let style = if i < filled {
                    Style::default().bg(hp_color)
                } else {
                    Style::default().bg(Color::Rgb(30, 30, 30))
                };
                buf.set_string(bar_x + 1 + i, area.y, " ", style);
            }
            buf.set_string(bar_x + 1 + bar_width, area.y, "]", Style::default().fg(Color::Rgb(100, 100, 100)));
        }

        // Combo: left of center
        if self.state.combo > 1 {
            let combo_text = format!("x{} COMBO", self.state.combo);
            let combo_w = combo_text.chars().count() as u16;
            let combo_x = cx.saturating_sub(combo_w + 8);
            buf.set_string(combo_x, area.y, &combo_text,
                Style::default().fg(Color::Rgb(180, 180, 180)));
        }

        // Score: right of center
        let score_text = format!("SCORE {}", self.state.score);
        let score_x = cx + 4;
        buf.set_string(score_x, area.y, &score_text,
            Style::default().fg(Color::Rgb(180, 180, 180)));
    }
}

pub struct HudBottom<'a> {
    pub state: &'a GameState,
    pub song_title: &'a str,
    pub progress: f64,
    pub difficulty: &'a str,
    pub total_notes: u32,
}

impl<'a> Widget for HudBottom<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 { return; }

        // Single line: song title left, accuracy + difficulty center-right, progress bar far right
        let info = format!(" {} ", self.song_title);
        let truncated: String = info.chars().take((area.width / 2) as usize).collect();
        buf.set_string(area.x, area.y, &truncated,
            Style::default().fg(Color::Rgb(60, 60, 60)));

        // Note counter
        let note_counter = format!("{}/{}", self.state.total_notes, self.total_notes);
        let nc_x = area.x + area.width / 2 - note_counter.len() as u16 - 2;
        buf.set_string(nc_x, area.y, &note_counter,
            Style::default().fg(Color::Rgb(80, 80, 80)));

        let acc = format!("ACC {:.1}%  {}", self.state.accuracy(), self.difficulty);
        let acc_x = area.x + area.width / 2;
        buf.set_string(acc_x, area.y, &acc,
            Style::default().fg(Color::Rgb(80, 80, 80)));

        // Progress bar
        let bar_width = 10u16.min(area.width / 6);
        let bar_x = area.x + area.width.saturating_sub(bar_width + 1);
        let filled = (self.progress * bar_width as f64) as u16;
        for i in 0..bar_width {
            let (ch, style) = if i < filled {
                ("-", Style::default().fg(Color::Rgb(100, 100, 100)))
            } else {
                ("-", Style::default().fg(Color::Rgb(30, 30, 30)))
            };
            buf.set_string(bar_x + i, area.y, ch, style);
        }
    }
}
