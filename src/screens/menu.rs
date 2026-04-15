use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::app::{Action, Screen};
use crate::ui::widgets::MenuList;

const LOGO: &str = r#"
   ▄████▄   ▄▄▄        ██████  ▄████▄   ▄▄▄      ▓█████▄ ▓█████
  ▒██▀ ▀█  ▒████▄    ▒██    ▒ ▒██▀ ▀█  ▒████▄    ▒██▀ ██▌▓█   ▀
  ▒▓█    ▄ ▒██  ▀█▄  ░ ▓██▄   ▒▓█    ▄ ▒██  ▀█▄  ░██   █▌▒███
  ▒▓▓▄ ▄██▒░██▄▄▄▄██   ▒   ██▒▒▓▓▄ ▄██▒░██▄▄▄▄██ ░▓█▄   ▌▒▓█  ▄
  ▒ ▓███▀ ░ ▓█   ▓██▒▒██████▒▒▒ ▓███▀ ░ ▓█   ▓██▒░▒████▓ ░▒████▒
  ░ ░▒ ▒  ░ ▒▒   ▓▒█░▒ ▒▓▒ ▒ ░░ ░▒ ▒  ░ ▒▒   ▓▒█░ ▒▒▓  ▒ ░░ ▒░ ░
"#;

const MENU_ITEMS: &[&str] = &["Play", "Settings", "Quit"];

pub struct MenuScreen {
    pub selected: usize,
}

impl MenuScreen {
    pub fn new() -> Self {
        Self { selected: 0 }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::MenuUp => {
                if self.selected > 0 { self.selected -= 1; }
                None
            }
            Action::MenuDown => {
                if self.selected < MENU_ITEMS.len() - 1 { self.selected += 1; }
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

        let logo_height = LOGO.lines().count() as u16;
        let logo_y = area.y + 2;
        for (i, line) in LOGO.lines().enumerate() {
            let y = logo_y + i as u16;
            if y >= area.y + area.height { break; }
            let x = area.x + area.width.saturating_sub(line.len() as u16) / 2;
            buf.set_string(x, y, line, Style::default().fg(Color::Rgb(160, 160, 160)));
        }

        let menu_area = Rect {
            x: area.x,
            y: logo_y + logo_height + 2,
            width: area.width,
            height: area.height.saturating_sub(logo_height + 6),
        };

        MenuList { items: MENU_ITEMS, selected: self.selected }.render(menu_area, buf);

        let footer = "Terminal Rhythm Game";
        let footer_y = area.y + area.height - 1;
        let x = area.x + (area.width.saturating_sub(footer.len() as u16)) / 2;
        buf.set_string(x, footer_y, footer, Style::default().fg(Color::Rgb(60, 60, 60)));
    }
}
