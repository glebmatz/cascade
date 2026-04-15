use crossterm::event::{KeyCode, KeyEvent};
use crate::app::Action;

pub fn map_key(key: KeyEvent, lanes: &[char; 5]) -> Action {
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Up | KeyCode::Char('k') => Action::MenuUp,
        KeyCode::Down | KeyCode::Char('j') => Action::MenuDown,
        KeyCode::Enter => Action::MenuSelect,
        KeyCode::Esc => Action::Pause,
        KeyCode::Tab => Action::Tab,
        KeyCode::Char('i') => Action::Import,
        KeyCode::Char(c) => {
            for (i, &lane_key) in lanes.iter().enumerate() {
                if c == lane_key {
                    return Action::GameKey(i);
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}
