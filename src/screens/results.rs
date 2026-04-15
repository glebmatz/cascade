use ratatui::prelude::*;
use crate::app::{Action, Screen};
use crate::game::state::GameState;

pub struct ResultsScreen {
    pub state: GameState,
    pub song_title: String,
    pub difficulty: String,
    pub selected: usize,
}

impl ResultsScreen {
    pub fn new(state: GameState, song_title: String, difficulty: String) -> Self {
        Self { state, song_title, difficulty, selected: 0 }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::MenuUp | Action::MenuDown => {
                self.selected = 1 - self.selected;
                None
            }
            Action::MenuSelect => {
                if self.selected == 0 {
                    Some(Action::Navigate(Screen::Gameplay)) // retry
                } else {
                    Some(Action::Navigate(Screen::SongSelect))
                }
            }
            Action::Back | Action::Pause => Some(Action::Navigate(Screen::SongSelect)),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();
        let cx = area.x + area.width / 2;
        let mut y = area.y + 2;

        let title = "RESULTS";
        buf.set_string(cx.saturating_sub(title.len() as u16 / 2), y, title, Style::default().fg(Color::White).bold());
        y += 2;

        let song_info = format!("{} [{}]", self.song_title, self.difficulty);
        buf.set_string(cx.saturating_sub(song_info.len() as u16 / 2), y, &song_info, Style::default().fg(Color::Rgb(140, 140, 140)));
        y += 3;

        let grade = self.state.grade();
        buf.set_string(cx.saturating_sub(1), y, grade, Style::default().fg(Color::White).bold());
        y += 3;

        let stats = [
            format!("Score:     {:>10}", self.state.score),
            format!("Accuracy:  {:>9.1}%", self.state.accuracy()),
            format!("Max Combo: {:>10}", self.state.max_combo),
            String::new(),
            format!("Perfect:   {:>10}", self.state.judgement_counts[0]),
            format!("Great:     {:>10}", self.state.judgement_counts[1]),
            format!("Good:      {:>10}", self.state.judgement_counts[2]),
            format!("Miss:      {:>10}", self.state.judgement_counts[3]),
        ];

        for line in &stats {
            let w = line.chars().count() as u16;
            buf.set_string(cx.saturating_sub(w / 2), y, line, Style::default().fg(Color::Rgb(160, 160, 160)));
            y += 1;
        }
        y += 2;

        let options = ["Retry", "Back to songs"];
        for (i, option) in options.iter().enumerate() {
            let (prefix, style) = if i == self.selected {
                ("> ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
            };
            let text = format!("{}{}", prefix, option);
            let w = text.chars().count() as u16;
            buf.set_string(cx.saturating_sub(w / 2), y, &text, style);
            y += 1;
        }
    }
}
