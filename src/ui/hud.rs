use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::state::GameState;

pub struct HudTop<'a> {
    pub state: &'a GameState,
}

impl<'a> Widget for HudTop<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 { return; }

        // Combo left
        if self.state.combo > 1 {
            let combo_text = format!("x{} COMBO", self.state.combo);
            buf.set_string(area.x + 2, area.y, &combo_text,
                Style::default().fg(Color::Rgb(180, 180, 180)));
        }

        // Score right
        let score_text = format!("SCORE {:>8}", self.state.score);
        let score_x = area.x + area.width.saturating_sub(score_text.len() as u16 + 2);
        buf.set_string(score_x, area.y, &score_text,
            Style::default().fg(Color::Rgb(180, 180, 180)));
    }
}

pub struct HudBottom<'a> {
    pub state: &'a GameState,
    pub song_title: &'a str,
    pub progress: f64,
    pub difficulty: &'a str,
}

impl<'a> Widget for HudBottom<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 { return; }

        // Single line: song title left, accuracy + difficulty center-right, progress bar far right
        let info = format!(" {} ", self.song_title);
        let truncated: String = info.chars().take((area.width / 2) as usize).collect();
        buf.set_string(area.x, area.y, &truncated,
            Style::default().fg(Color::Rgb(60, 60, 60)));

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
