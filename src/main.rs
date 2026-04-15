mod app;
mod config;
mod input;
mod beatmap;
mod game;
mod audio;
mod ui;
mod screens;

use std::io;
use std::time::{Duration, Instant};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, Action, Screen};
use config::Config;
use screens::menu::MenuScreen;
use screens::song_select::SongSelectScreen;
use screens::gameplay::GameplayScreen;
use screens::results::ResultsScreen;
use screens::settings::SettingsScreen;
use beatmap::types::Difficulty;

const TARGET_FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

fn main() -> Result<()> {
    // CLI: cascade add <path>
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "add" {
        let file_path = std::path::PathBuf::from(&args[2]);
        let songs_dir = Config::cascade_dir().join("songs");
        let _ = std::fs::create_dir_all(&songs_dir);

        println!("Importing {}...", file_path.display());
        let song = audio::import::import_local_file(&file_path, &songs_dir)?;
        println!("Generating beatmaps for {}...", song.title);

        let (samples, sample_rate) = audio::analyzer::decode_audio(&song.audio_path)?;
        let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u64;
        let audio_filename = song.audio_path.file_name()
            .unwrap_or_default().to_string_lossy().to_string();
        let meta = beatmap::types::SongMeta {
            title: song.title.clone(),
            artist: String::new(),
            audio_file: audio_filename,
            bpm: 120,
            duration_ms,
        };
        let beatmaps = beatmap::generator::generate_all_beatmaps(&samples, sample_rate, meta);
        for bm in &beatmaps {
            let path = song.dir.join(bm.difficulty.filename());
            let _ = beatmap::loader::save(bm, &path);
        }
        println!("Successfully imported: {}", song.title);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(ref e) = result {
        eprintln!("Error: {}", e);
    }

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let config = Config::load(&Config::default_path())?;
    let songs_dir = Config::cascade_dir().join("songs");
    let _ = std::fs::create_dir_all(&songs_dir);

    let mut app = App::new();

    let mut menu = MenuScreen::new();
    let mut song_select = SongSelectScreen::new(
        match config.gameplay.difficulty.as_str() {
            "easy" => Difficulty::Easy,
            "medium" => Difficulty::Medium,
            "expert" => Difficulty::Expert,
            _ => Difficulty::Hard,
        }
    );
    let mut gameplay: Option<GameplayScreen> = None;
    let mut results: Option<ResultsScreen> = None;
    let mut settings: Option<SettingsScreen> = None;

    // For retry
    let mut last_beatmap_path: Option<std::path::PathBuf> = None;
    let mut last_audio_path: Option<std::path::PathBuf> = None;
    let mut last_song_title = String::new();

    while app.running {
        let frame_start = Instant::now();

        // Input
        if event::poll(Duration::from_millis(1))? {
            if let Event::Key(key) = event::read()? {
                // Handle import mode text input
                if app.screen == Screen::SongSelect && song_select.import_mode {
                    match key.code {
                        KeyCode::Char(c) => { song_select.import_input.push(c); }
                        KeyCode::Backspace => { song_select.import_input.pop(); }
                        KeyCode::Enter => {
                            let url = song_select.import_input.clone();
                            song_select.import_mode = false;
                            song_select.import_input.clear();

                            if !url.is_empty() {
                                let file_path = std::path::PathBuf::from(url.trim());

                                song_select.import_status = Some("Importing...".to_string());
                                terminal.draw(|frame| {
                                    song_select.render(frame, frame.area());
                                })?;

                                match audio::import::import_local_file(&file_path, &songs_dir) {
                                    Ok(song) => {
                                        song_select.import_status = Some(format!("Generating beatmaps for {}...", song.title));
                                        terminal.draw(|frame| {
                                            song_select.render(frame, frame.area());
                                        })?;

                                        match audio::analyzer::decode_audio(&song.audio_path) {
                                            Ok((samples, sample_rate)) => {
                                                let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u64;
                                                let audio_filename = song.audio_path.file_name()
                                                    .unwrap_or_default().to_string_lossy().to_string();
                                                let meta = beatmap::types::SongMeta {
                                                    title: song.title.clone(),
                                                    artist: String::new(),
                                                    audio_file: audio_filename,
                                                    bpm: 120,
                                                    duration_ms,
                                                };
                                                let beatmaps = beatmap::generator::generate_all_beatmaps(&samples, sample_rate, meta);
                                                for bm in &beatmaps {
                                                    let path = song.dir.join(bm.difficulty.filename());
                                                    let _ = beatmap::loader::save(bm, &path);
                                                }
                                                song_select.import_status = Some(format!("Imported: {}", song.title));
                                            }
                                            Err(e) => {
                                                song_select.import_status = Some(format!("Decode error: {}", e));
                                            }
                                        }
                                        song_select.scan_songs(&songs_dir);
                                    }
                                    Err(e) => {
                                        song_select.import_status = Some(format!("Error: {}", e));
                                    }
                                }
                            }
                        }
                        KeyCode::Esc => {
                            song_select.import_mode = false;
                            song_select.import_input.clear();
                        }
                        _ => {}
                    }
                } else {
                    // Use context-aware key mapping
                    let action = match app.screen {
                        Screen::Gameplay => input::map_key_gameplay(key, &config.keys.lanes),
                        Screen::Settings => {
                            // Settings needs both menu nav AND game keys for value adjustment
                            let menu_action = input::map_key_menu(key);
                            if menu_action == Action::None {
                                input::map_key_gameplay(key, &config.keys.lanes)
                            } else {
                                menu_action
                            }
                        }
                        _ => input::map_key_menu(key),
                    };

                    let result_action = match app.screen {
                        Screen::Menu => {
                            if action == Action::Quit {
                                Some(Action::Quit)
                            } else {
                                menu.handle_action(action)
                            }
                        }
                        Screen::SongSelect => {
                            if action == Action::Import {
                                song_select.import_mode = true;
                                song_select.import_input.clear();
                                None
                            } else if action == Action::Quit {
                                Some(Action::Navigate(Screen::Menu))
                            } else {
                                song_select.handle_action(action)
                            }
                        }
                        Screen::Gameplay => {
                            if let Some(gp) = &mut gameplay {
                                gp.handle_action(action)
                            } else {
                                None
                            }
                        }
                        Screen::Results => {
                            if let Some(rs) = &mut results {
                                rs.handle_action(action)
                            } else {
                                None
                            }
                        }
                        Screen::Settings => {
                            if let Some(st) = &mut settings {
                                if action == Action::Quit {
                                    Some(Action::Navigate(Screen::Menu))
                                } else {
                                    st.handle_action(action)
                                }
                            } else {
                                None
                            }
                        }
                    };

                    if let Some(ra) = result_action {
                        match ra {
                            Action::Quit => app.quit(),
                            Action::Navigate(screen) => {
                                // Transition logic
                                match screen {
                                    Screen::SongSelect => {
                                        song_select.scan_songs(&songs_dir);
                                        gameplay = None;
                                    }
                                    Screen::Gameplay => {
                                        // Load beatmap: fresh selection from song_select, or retry from last
                                        let from_song_select = app.screen == Screen::SongSelect;
                                        let beatmap_path = if from_song_select {
                                            song_select.selected_beatmap_path()
                                        } else {
                                            last_beatmap_path.clone().or_else(|| song_select.selected_beatmap_path())
                                        };
                                        let audio_path = if from_song_select {
                                            song_select.selected_audio_path()
                                        } else {
                                            last_audio_path.clone().or_else(|| song_select.selected_audio_path())
                                        };

                                        if let (Some(bp), Some(ap)) = (beatmap_path, audio_path) {
                                            if bp.exists() && ap.exists() {
                                                // Show loading screen
                                                terminal.draw(|frame| {
                                                    let area = frame.area();
                                                    let buf = frame.buffer_mut();
                                                    let msg = "Loading...";
                                                    let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
                                                    let y = area.y + area.height / 2;
                                                    buf.set_string(x, y, msg, Style::default().fg(Color::Rgb(140, 140, 140)));
                                                })?;

                                                match beatmap::loader::load(&bp) {
                                                    Ok(bm) => {
                                                        last_song_title = song_select.selected_song_title();
                                                        last_beatmap_path = Some(bp);
                                                        last_audio_path = Some(ap.clone());

                                                        // Decode for visualizer
                                                        let (samples, sample_rate) = audio::analyzer::decode_audio(&ap)
                                                            .unwrap_or_else(|_| (vec![], 44100));

                                                        match GameplayScreen::new(
                                                            bm,
                                                            &ap,
                                                            samples,
                                                            sample_rate,
                                                            config.audio.offset_ms,
                                                            config.gameplay.scroll_speed,
                                                            config.audio.volume,
                                                        ) {
                                                            Ok(mut gp) => {
                                                                gp.start();
                                                                gameplay = Some(gp);
                                                            }
                                                            Err(e) => {
                                                                song_select.import_status = Some(format!("Audio error: {}", e));
                                                                app.navigate(Screen::SongSelect);
                                                                continue;
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        song_select.import_status = Some(format!("Beatmap error: {}", e));
                                                        app.navigate(Screen::SongSelect);
                                                        continue;
                                                    }
                                                }
                                            } else {
                                                song_select.import_status = Some("Beatmap or audio file not found".to_string());
                                                app.navigate(Screen::SongSelect);
                                                continue;
                                            }
                                        } else {
                                            app.navigate(Screen::SongSelect);
                                            continue;
                                        }
                                    }
                                    Screen::Results => {
                                        if let Some(gp) = gameplay.take() {
                                            results = Some(ResultsScreen::new(
                                                gp.state,
                                                last_song_title.clone(),
                                                gp.beatmap.difficulty.to_string(),
                                            ));
                                        }
                                    }
                                    Screen::Settings => {
                                        let cfg = Config::load(&Config::default_path()).unwrap_or_default();
                                        settings = Some(SettingsScreen::new(cfg));
                                    }
                                    Screen::Menu => {
                                        last_beatmap_path = None;
                                        last_audio_path = None;
                                    }
                                }
                                app.navigate(screen);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Update
        if app.screen == Screen::Gameplay {
            if let Some(gp) = &mut gameplay {
                gp.update();
                if gp.finished {
                    let gp = gameplay.take().unwrap();
                    results = Some(ResultsScreen::new(
                        gp.state,
                        last_song_title.clone(),
                        gp.beatmap.difficulty.to_string(),
                    ));
                    app.navigate(Screen::Results);
                }
            }
        }

        // Render
        terminal.draw(|frame| {
            let area = frame.area();
            match app.screen {
                Screen::Menu => menu.render(frame, area),
                Screen::SongSelect => song_select.render(frame, area),
                Screen::Gameplay => {
                    if let Some(gp) = &mut gameplay {
                        gp.render(frame, area);
                    }
                }
                Screen::Results => {
                    if let Some(rs) = &results {
                        rs.render(frame, area);
                    }
                }
                Screen::Settings => {
                    if let Some(st) = &settings {
                        st.render(frame, area);
                    }
                }
            }
        })?;

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}
