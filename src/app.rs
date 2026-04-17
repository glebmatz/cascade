#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    SongSelect,
    Gameplay,
    Results,
    Settings,
    Calibrate,
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
    GameKeyRelease(usize),
    Pause,
    #[allow(dead_code)]
    Back,
    Tab,
    Import,
    Delete,
    Rename,
    Sort,
    Mods,
    Practice,
}

pub struct App {
    pub screen: Screen,
    pub running: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
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
