mod app;
mod input;

use std::io;
use std::time::{Duration, Instant};
use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, Action, Screen};
use input::map_key;

const TARGET_FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    let lanes = ['d', 'f', ' ', 'j', 'k'];

    while app.running {
        let frame_start = Instant::now();

        if event::poll(Duration::from_millis(1))? {
            if let Event::Key(key) = event::read()? {
                let action = map_key(key, &lanes);
                match action {
                    Action::Quit => app.quit(),
                    Action::Navigate(screen) => app.navigate(screen),
                    _ => {}
                }
            }
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let text = match app.screen {
                Screen::Menu => "CASCADE — Press Q to quit",
                Screen::SongSelect => "Song Select",
                Screen::Gameplay => "Gameplay",
                Screen::Results => "Results",
                Screen::Settings => "Settings",
            };
            frame.render_widget(
                ratatui::widgets::Paragraph::new(text)
                    .alignment(Alignment::Center),
                area,
            );
        })?;

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}
