use crossterm::event::{KeyCode, KeyEvent};
use crate::app::Action;

/// Map key in menu context (j/k = navigate)
pub fn map_key_menu(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Up | KeyCode::Char('k') => Action::MenuUp,
        KeyCode::Down | KeyCode::Char('j') => Action::MenuDown,
        KeyCode::Enter => Action::MenuSelect,
        KeyCode::Esc => Action::Pause,
        KeyCode::Tab => Action::Tab,
        KeyCode::Char('i') => Action::Import,
        KeyCode::Char('x') | KeyCode::Delete => Action::Delete,
        _ => Action::None,
    }
}

/// Map key in gameplay context (d/f/space/j/k = lanes)
pub fn map_key_gameplay(key: KeyEvent, lanes: &[char; 5]) -> Action {
    match key.code {
        KeyCode::Esc => Action::Pause,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char(c) => {
            for (i, &lane_key) in lanes.iter().enumerate() {
                if c == lane_key {
                    return Action::GameKey(i);
                }
            }
            Action::None
        }
        KeyCode::Char(' ') | KeyCode::Backspace => {
            // Space might not match as Char(' ') in some terminals
            Action::GameKey(2)
        }
        _ => Action::None,
    }
}
