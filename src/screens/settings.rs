use ratatui::prelude::*;
use crate::app::{Action, Screen};
use crate::config::Config;

const SETTINGS_ITEMS: &[&str] = &["Scroll Speed", "Volume", "Audio Offset (ms)", "Back"];

pub struct SettingsScreen {
    pub config: Config,
    pub selected: usize,
    pub modified: bool,
}

impl SettingsScreen {
    pub fn new(config: Config) -> Self {
        Self { config, selected: 0, modified: false }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::MenuUp => {
                if self.selected > 0 { self.selected -= 1; }
                None
            }
            Action::MenuDown => {
                if self.selected < SETTINGS_ITEMS.len() - 1 { self.selected += 1; }
                None
            }
            Action::MenuSelect => {
                if self.selected == SETTINGS_ITEMS.len() - 1 {
                    self.save_if_modified();
                    Some(Action::Navigate(Screen::Menu))
                } else {
                    None
                }
            }
            Action::Back | Action::Pause => {
                self.save_if_modified();
                Some(Action::Navigate(Screen::Menu))
            }
            Action::GameKey(lane) => {
                let increase = lane == 3 || lane == 4;
                let decrease = lane == 0 || lane == 1;
                match self.selected {
                    0 => {
                        if increase { self.config.gameplay.scroll_speed = (self.config.gameplay.scroll_speed + 0.1).min(2.0); }
                        if decrease { self.config.gameplay.scroll_speed = (self.config.gameplay.scroll_speed - 0.1).max(0.5); }
                        self.modified = true;
                    }
                    1 => {
                        if increase { self.config.audio.volume = (self.config.audio.volume + 0.05).min(1.0); }
                        if decrease { self.config.audio.volume = (self.config.audio.volume - 0.05).max(0.0); }
                        self.modified = true;
                    }
                    2 => {
                        if increase { self.config.audio.offset_ms = (self.config.audio.offset_ms + 5).min(200); }
                        if decrease { self.config.audio.offset_ms = (self.config.audio.offset_ms - 5).max(-200); }
                        self.modified = true;
                    }
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    fn save_if_modified(&self) {
        if self.modified {
            let _ = self.config.save(&Config::default_path());
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();
        let cx = area.x + area.width / 2;
        let mut y = area.y + 3;

        let title = "SETTINGS";
        let title_w = title.chars().count() as u16;
        buf.set_string(cx.saturating_sub(title_w / 2), y, title, Style::default().fg(Color::White).bold());
        y += 3;

        let values = [
            format!("{:.1}", self.config.gameplay.scroll_speed),
            format!("{:.0}%", self.config.audio.volume * 100.0),
            format!("{:+}ms", self.config.audio.offset_ms),
            String::new(),
        ];

        for (i, (item, value)) in SETTINGS_ITEMS.iter().zip(values.iter()).enumerate() {
            let (prefix, style) = if i == self.selected {
                ("> ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
            };

            let text = if value.is_empty() {
                format!("{}{}", prefix, item)
            } else {
                format!("{}{}:  < {} >", prefix, item, value)
            };

            let text_w = text.chars().count() as u16;
            let x = cx.saturating_sub(text_w / 2);
            buf.set_string(x, y, &text, style);
            y += 2;
        }

        y += 2;
        let hint = "D/F: Decrease    J/K: Increase    ESC: Back";
        let hint_w = hint.chars().count() as u16;
        let x = cx.saturating_sub(hint_w / 2);
        buf.set_string(x, y, hint, Style::default().fg(Color::Rgb(60, 60, 60)));
    }
}
