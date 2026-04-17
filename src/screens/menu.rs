use crate::app::{Action, Screen};
use crate::ui::chrome::{render_bottom_bar, render_top_bar};
use crate::ui::widgets::MenuList;
use ratatui::prelude::*;
use ratatui::widgets::Widget;

const LOGO_LINES: &[&str] = &[
    r"  ____                        _      ",
    r" / ___|__ _ ___  ___ __ _  __| | ___ ",
    r"| |   / _` / __|/ __/ _` |/ _` |/ _ \",
    r"| |__| (_| \__ \ (_| (_| | (_| |  __/",
    r" \____\__,_|___/\___\__,_|\__,_|\___|",
];

const MENU_ITEMS: &[&str] = &["Play", "Settings", "Quit"];

#[derive(Clone, Copy)]
struct BgStar {
    x: f32,
    y: f32,
    speed: f32,
    bright: u8,
    glyph_idx: u8,
}

pub struct MenuScreen {
    pub selected: usize,
    tick: u32,
    stars: Vec<BgStar>,
}

impl Default for MenuScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuScreen {
    pub fn new() -> Self {
        Self {
            selected: 0,
            tick: 0,
            stars: Self::init_stars(),
        }
    }

    fn init_stars() -> Vec<BgStar> {
        let mut v = Vec::with_capacity(50);
        let mut rng: u32 = 0xBEEF1234;
        for _ in 0..50 {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = ((rng >> 8) & 0x1FFF) as f32;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let y = ((rng >> 8) & 0x1FF) as f32;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let speed = 0.08 + ((rng >> 8) & 0x7F) as f32 / 600.0;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let bright = 45 + ((rng >> 8) & 0x4F) as u8;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let glyph_idx = ((rng >> 8) & 0x3) as u8;
            v.push(BgStar {
                x,
                y,
                speed,
                bright,
                glyph_idx,
            });
        }
        v
    }

    pub fn update(&mut self, area: Rect) {
        self.tick = self.tick.wrapping_add(1);
        let h = area.height as f32;
        let w = area.width as f32;
        for s in &mut self.stars {
            s.y += s.speed;
            if s.y >= h {
                s.y = 0.0;
            }
            while s.x >= w {
                s.x -= w;
            }
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
                if self.selected < MENU_ITEMS.len() - 1 {
                    self.selected += 1;
                }
                None
            }
            Action::MenuSelect => match self.selected {
                0 => Some(Action::Navigate(Screen::SongSelect)),
                1 => Some(Action::Navigate(Screen::Settings)),
                2 => Some(Action::Quit),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();

        // 1. Starfield background.
        let glyphs = [".", "·", "•", "∘"];
        for s in &self.stars {
            let x = s.x as u16;
            let y = s.y as u16;
            if x >= area.x + area.width || y >= area.y + area.height {
                continue;
            }
            let g = glyphs[s.glyph_idx as usize % glyphs.len()];
            buf.set_string(
                area.x + x,
                area.y + y,
                g,
                Style::default().fg(Color::Rgb(
                    s.bright,
                    s.bright,
                    (s.bright as u16 + 24).min(200) as u8,
                )),
            );
        }

        // 2. Top chrome.
        let top = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        render_top_bar(buf, top, &["MENU"]);

        // 3. Logo with breathing effect.
        let phase = (self.tick as f32 * 0.04).sin() * 0.5 + 0.5; // 0..1 slow oscillation
        let base = 150.0;
        let amp = 60.0;
        let logo_brightness = (base + amp * phase) as u8;
        let accent_b = (base + amp * (1.0 - phase) * 0.6) as u8;

        let logo_height = LOGO_LINES.len() as u16;
        let logo_y = area.y + 4;
        for (i, line) in LOGO_LINES.iter().enumerate() {
            let y = logo_y + i as u16;
            if y >= area.y + area.height - 6 {
                break;
            }
            let line_width = line.chars().count() as u16;
            let x = area.x + area.width.saturating_sub(line_width) / 2;
            // Row-wise color gradient: top dim, middle bright.
            let mid_factor = 1.0 - ((i as f32 - 2.0).abs() / 2.5);
            let r = (logo_brightness as f32 * (0.55 + 0.45 * mid_factor)) as u8;
            let g = (logo_brightness as f32 * (0.55 + 0.45 * mid_factor)) as u8;
            let b = accent_b.saturating_add(20);
            buf.set_string(x, y, line, Style::default().fg(Color::Rgb(r, g, b)));
        }

        // Subtitle.
        let subtitle = "TERMINAL RHYTHM";
        let sy = logo_y + logo_height + 1;
        let sx = area.x + (area.width.saturating_sub(subtitle.len() as u16)) / 2;
        buf.set_string(
            sx,
            sy,
            subtitle,
            Style::default().fg(Color::Rgb(120, 120, 140)),
        );

        // 4. Menu items.
        let menu_area = Rect {
            x: area.x,
            y: sy + 3,
            width: area.width,
            height: area.height.saturating_sub((sy + 3 - area.y) + 2),
        };
        MenuList {
            items: MENU_ITEMS,
            selected: self.selected,
        }
        .render(menu_area, buf);

        // 5. Bottom chrome.
        let bot = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        render_bottom_bar(buf, bot, &[("↑↓", "move"), ("↵", "select"), ("Q", "quit")]);
    }
}
