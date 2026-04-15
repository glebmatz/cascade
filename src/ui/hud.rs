use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::state::GameState;
use crate::game::hit_judge::Judgement;

pub struct HudTop<'a> {
    pub state: &'a GameState,
}

impl<'a> Widget for HudTop<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 {
            return;
        }

        let combo_text = if self.state.combo > 1 {
            format!("x{} COMBO", self.state.combo)
        } else {
            String::new()
        };
        buf.set_string(
            area.x + 2,
            area.y,
            &combo_text,
            Style::default().fg(Color::Rgb(180, 180, 180)),
        );

        let score_text = format!("SCORE {:>8}", self.state.score);
        let score_x = area.x + area.width.saturating_sub(score_text.len() as u16 + 2);
        buf.set_string(score_x, area.y, &score_text, Style::default().fg(Color::Rgb(180, 180, 180)));
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
        if area.height < 2 || area.width < 30 {
            return;
        }

        // Judgement feedback
        if let Some(judgement) = self.state.last_judgement {
            let (label, style) = match judgement {
                Judgement::Perfect => ("✦ PERFECT ✦", Style::default().fg(Color::White).bold()),
                Judgement::Great => ("GREAT", Style::default().fg(Color::Rgb(200, 200, 200))),
                Judgement::Good => ("GOOD", Style::default().fg(Color::Rgb(140, 140, 140))),
                Judgement::Miss => ("MISS", Style::default().fg(Color::Rgb(80, 80, 80))),
            };
            let x = area.x + (area.width.saturating_sub(label.len() as u16)) / 2;
            buf.set_string(x, area.y, label, style);
        }

        if area.height >= 2 {
            let y = area.y + 1;
            let info = format!(
                " ▸▸ {}    ACC {:.1}%    {}",
                self.song_title,
                self.state.accuracy(),
                self.difficulty,
            );
            let truncated: String = info.chars().take(area.width as usize - 2).collect();
            buf.set_string(area.x + 1, y, &truncated, Style::default().fg(Color::Rgb(100, 100, 100)));

            let bar_width = 12u16.min(area.width / 4);
            let bar_x = area.x + area.width.saturating_sub(bar_width + 2);
            let filled = (self.progress * bar_width as f64) as u16;
            for i in 0..bar_width {
                let (ch, style) = if i < filled {
                    ("━", Style::default().fg(Color::Rgb(150, 150, 150)))
                } else {
                    ("─", Style::default().fg(Color::Rgb(50, 50, 50)))
                };
                buf.set_string(bar_x + i, y, ch, style);
            }
        }
    }
}
