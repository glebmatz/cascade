use crate::app::{Action, Screen};
use crate::config::Config;
use crate::ui::chrome::{render_bottom_bar, render_top_bar};
use crate::ui::theme;
use ratatui::prelude::*;

const SETTINGS_ITEMS: &[&str] = &[
    "Scroll Speed",
    "Volume",
    "Audio Offset (ms)",
    "Health",
    "Hold Notes",
    "Theme",
    "Calibrate Audio",
    "Back",
];

// Index constants so the match arms in handle_action stay readable as items shift.
const IDX_SCROLL_SPEED: usize = 0;
const IDX_VOLUME: usize = 1;
const IDX_OFFSET: usize = 2;
const IDX_HEALTH: usize = 3;
const IDX_HOLDS: usize = 4;
const IDX_THEME: usize = 5;
const IDX_CALIBRATE: usize = 6;
const IDX_BACK: usize = 7;

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
            Action::MenuSelect => match self.selected {
                IDX_BACK => {
                    self.save_if_modified();
                    Some(Action::Navigate(Screen::Menu))
                }
                IDX_HEALTH => {
                    self.config.gameplay.health_enabled = !self.config.gameplay.health_enabled;
                    self.modified = true;
                    None
                }
                IDX_HOLDS => {
                    self.config.gameplay.holds_enabled = !self.config.gameplay.holds_enabled;
                    self.modified = true;
                    None
                }
                IDX_THEME => {
                    // Enter advances to the next theme, same as the right lane keys.
                    self.advance_theme(true);
                    None
                }
                IDX_CALIBRATE => {
                    self.save_if_modified();
                    Some(Action::Navigate(Screen::Calibrate))
                }
                _ => None,
            },
            Action::Back | Action::Pause => {
                self.save_if_modified();
                Some(Action::Navigate(Screen::Menu))
            }
            Action::GameKey(lane) => {
                let increase = lane == 3 || lane == 4;
                let decrease = lane == 0 || lane == 1;
                match self.selected {
                    IDX_SCROLL_SPEED => {
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
                    IDX_VOLUME => {
                        if increase {
                            self.config.audio.volume = (self.config.audio.volume + 0.05).min(1.0);
                        }
                        if decrease {
                            self.config.audio.volume = (self.config.audio.volume - 0.05).max(0.0);
                        }
                        self.modified = true;
                    }
                    IDX_OFFSET => {
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
                    IDX_HEALTH => {
                        self.config.gameplay.health_enabled = !self.config.gameplay.health_enabled;
                        self.modified = true;
                    }
                    IDX_HOLDS => {
                        self.config.gameplay.holds_enabled = !self.config.gameplay.holds_enabled;
                        self.modified = true;
                    }
                    IDX_THEME => {
                        if increase {
                            self.advance_theme(true);
                        }
                        if decrease {
                            self.advance_theme(false);
                        }
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

    /// Cycle the active theme and persist the new slug to config. `forward =
    /// false` cycles backward. The global active palette is updated
    /// immediately so the next frame already uses it.
    fn advance_theme(&mut self, forward: bool) {
        let current = self.config.display.theme.as_str();
        let next = if forward {
            theme::cycle_next(current)
        } else {
            theme::cycle_prev(current)
        };
        self.config.display.theme = next.slug.to_string();
        theme::set_active(next);
        self.modified = true;
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
        let theme_name = theme::resolve_or_default(&self.config.display.theme)
            .name
            .to_string();
        let values = [
            format!("{:.1}", self.config.gameplay.scroll_speed),
            format!("{:.0}%", self.config.audio.volume * 100.0),
            format!("{:+}ms", self.config.audio.offset_ms),
            health_str.to_string(),
            holds_str.to_string(),
            theme_name,
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
            } else if i == IDX_HEALTH || i == IDX_HOLDS {
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

        // Render a small palette preview under the Theme row so users can see
        // the lane colors change as they cycle.
        if self.selected == IDX_THEME {
            let current = theme::resolve_or_default(&self.config.display.theme);
            render_palette_preview(buf, cx, y, &current);
        }

        let _ = cx;
    }
}

fn render_palette_preview(buf: &mut Buffer, cx: u16, y: u16, t: &theme::Theme) {
    // 5 colored blocks, 4 cells wide each, with a 1-cell gap.
    let block_w: u16 = 4;
    let gap: u16 = 1;
    let total_w = 5 * block_w + 4 * gap;
    let start_x = cx.saturating_sub(total_w / 2);
    for (i, rgb) in t.lane_colors.iter().enumerate() {
        let bx = start_x + i as u16 * (block_w + gap);
        let color = Color::Rgb(rgb.0, rgb.1, rgb.2);
        for dx in 0..block_w {
            buf.set_string(bx + dx, y, " ", Style::default().bg(color));
        }
    }
}
