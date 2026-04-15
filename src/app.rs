#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    SongSelect,
    Gameplay,
    Results,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    Navigate(Screen),
    MenuUp,
    MenuDown,
    MenuSelect,
    GameKey(usize),
    Pause,
    Back,
    Tab,
    Import,
}

pub struct App {
    pub screen: Screen,
    pub running: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Menu,
            running: true,
        }
    }

    pub fn navigate(&mut self, screen: Screen) {
        self.screen = screen;
    }

    pub fn quit(&mut self) {
        self.running = false;
    }
}
