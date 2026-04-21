use crate::app::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

/// Map key in menu context (j/k = navigate). Releases and repeats are ignored.
pub fn map_key_menu(key: KeyEvent) -> Action {
    if !matches!(key.kind, KeyEventKind::Press) {
        return Action::None;
    }
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Up | KeyCode::Char('k') => Action::MenuUp,
        KeyCode::Down | KeyCode::Char('j') => Action::MenuDown,
        KeyCode::Enter => Action::MenuSelect,
        KeyCode::Esc => Action::Pause,
        KeyCode::Tab => Action::Tab,
        KeyCode::Char('i') => Action::Import,
        KeyCode::Char('x') | KeyCode::Delete => Action::Delete,
        KeyCode::Char('r') => Action::Rename,
        KeyCode::Char('s') => Action::Sort,
        KeyCode::Char('m') => Action::Mods,
        KeyCode::Char('p') => Action::Practice,
        _ => Action::None,
    }
}

/// Map key in gameplay context (d/f/space/j/k = lanes). Emits GameKey on press,
/// GameKeyRelease on release (if the terminal reports release events), and
/// GameKeyHeld on OS auto-repeat — the gameplay layer uses repeats as proof
/// that the key is still physically held on terminals without real release
/// events.
pub fn map_key_gameplay(key: KeyEvent, lanes: &[char; 5]) -> Action {
    let is_release = matches!(key.kind, KeyEventKind::Release);
    let is_repeat = matches!(key.kind, KeyEventKind::Repeat);

    // Menu-only keys still act on press.
    if !is_release {
        match key.code {
            KeyCode::Esc => return Action::Pause,
            KeyCode::Char('q') => return Action::Quit,
            _ => {}
        }
    } else if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
        return Action::None;
    }

    let lane_idx = match key.code {
        KeyCode::Char(c) => lanes.iter().position(|&k| k == c),
        KeyCode::Backspace => Some(2),
        _ => None,
    };

    match (lane_idx, is_release, is_repeat) {
        (Some(i), false, false) => Action::GameKey(i),
        (Some(i), true, _) => Action::GameKeyRelease(i),
        (Some(i), false, true) => Action::GameKeyHeld(i),
        (None, _, _) => Action::None,
    }
}
