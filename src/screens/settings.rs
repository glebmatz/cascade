use crate::app::{Action, Screen};
use crate::config::Config;
use crate::ui::chrome::{render_bottom_bar, render_top_bar};
use ratatui::prelude::*;

const SETTINGS_ITEMS: &[&str] = &[
    "Scroll Speed",
    "Volume",
    "Audio Offset (ms)",
    "Health",
    "Hold Notes",
    "Calibrate Audio",
    "Back",
];

pub struct SettingsScreen {
    pub config: Config,
    pub selected: usize,
    pub modified: bool,
}

impl SettingsScreen {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            selected: 0,
            modified: false,
        }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::MenuUp => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            Action::MenuDown => {
                if self.selected < SETTINGS_ITEMS.len() - 1 {
                    self.selected += 1;
                }
                None
            }
            Action::MenuSelect => {
                if self.selected == SETTINGS_ITEMS.len() - 1 {
                    self.save_if_modified();
                    Some(Action::Navigate(Screen::Menu))
                } else if self.selected == 3 {
                    // Toggle health
                    self.config.gameplay.health_enabled = !self.config.gameplay.health_enabled;
                    self.modified = true;
                    None
                } else if self.selected == 4 {
                    // Toggle hold notes
                    self.config.gameplay.holds_enabled = !self.config.gameplay.holds_enabled;
                    self.modified = true;
                    None
                } else if self.selected == 5 {
                    // Launch calibration
                    self.save_if_modified();
                    Some(Action::Navigate(Screen::Calibrate))
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
                        if increase {
                            self.config.gameplay.scroll_speed =
                                (self.config.gameplay.scroll_speed + 0.1).min(2.0);
                        }
                        if decrease {
                            self.config.gameplay.scroll_speed =
                                (self.config.gameplay.scroll_speed - 0.1).max(0.5);
                        }
                        self.modified = true;
                    }
                    1 => {
                        if increase {
                            self.config.audio.volume = (self.config.audio.volume + 0.05).min(1.0);
                        }
                        if decrease {
                            self.config.audio.volume = (self.config.audio.volume - 0.05).max(0.0);
                        }
                        self.modified = true;
                    }
                    2 => {
                        if increase {
                            self.config.audio.offset_ms =
                                (self.config.audio.offset_ms + 5).min(200);
                        }
                        if decrease {
                            self.config.audio.offset_ms =
                                (self.config.audio.offset_ms - 5).max(-200);
                        }
                        self.modified = true;
                    }
                    3 => {
                        // Toggle health with any lane key
                        self.config.gameplay.health_enabled = !self.config.gameplay.health_enabled;
                        self.modified = true;
                    }
                    4 => {
                        // Toggle hold notes with any lane key
                        self.config.gameplay.holds_enabled = !self.config.gameplay.holds_enabled;
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

        // Top + bottom chrome.
        let top = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        render_top_bar(buf, top, &["MENU", "SETTINGS"]);
        let bot = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        render_bottom_bar(
            buf,
            bot,
            &[
                ("↑↓", "select"),
                ("D F J K", "adjust"),
                ("↵", "confirm"),
                ("Esc", "back"),
            ],
        );

        let mut y = area.y + 3;

        let title = "SETTINGS";
        let title_w = title.chars().count() as u16;
        buf.set_string(
            cx.saturating_sub(title_w / 2),
            y,
            title,
            Style::default().fg(Color::White).bold(),
        );
        y += 3;

        let health_str = if self.config.gameplay.health_enabled {
            "ON"
        } else {
            "OFF"
        };
        let holds_str = if self.config.gameplay.holds_enabled {
            "ON"
        } else {
            "OFF"
        };
        let values = [
            format!("{:.1}", self.config.gameplay.scroll_speed),
            format!("{:.0}%", self.config.audio.volume * 100.0),
            format!("{:+}ms", self.config.audio.offset_ms),
            health_str.to_string(),
            holds_str.to_string(),
            String::new(),
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
            } else if i == 3 || i == 4 {
                // Toggle items — show differently
                format!("{}{}:  [ {} ]", prefix, item, value)
            } else {
                format!("{}{}:  < {} >", prefix, item, value)
            };

            let text_w = text.chars().count() as u16;
            let x = cx.saturating_sub(text_w / 2);
            buf.set_string(x, y, &text, style);
            y += 2;
        }

        let _ = cx;
    }
}
