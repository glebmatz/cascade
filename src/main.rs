#![allow(clippy::needless_range_loop)]
#![allow(clippy::question_mark)]
#![allow(clippy::while_let_loop)]

mod achievements;
mod app;
mod audio;
mod beatmap;
mod cli;
mod config;
mod game;
mod input;
mod score_store;
mod screens;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{
        self, Event, KeyCode, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;

use achievements::AchievementStore;
use app::{Action, App, Screen};
use audio::sfx::{self, SfxPlayer};
use beatmap::types::Difficulty;
use config::Config;
use score_store::{BestScore, ScoreStore};
use screens::calibrate::CalibrateScreen;
use screens::gameplay::GameplayScreen;
use screens::menu::MenuScreen;
use screens::results::ResultsScreen;
use screens::settings::SettingsScreen;
use screens::song_select::SongSelectScreen;

const TARGET_FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 {
        match args[1].as_str() {
            "add" if args.len() >= 3 => return cli::add(&args[2]),
            "list" | "ls" => return cli::list(),
            "song" if args.len() >= 3 => return cli::song(&args[2]),
            "achievements" => return cli::achievements_list(),
            "regen" => return cli::regen(),
            "rename" if args.len() >= 3 => {
                let slug = &args[2];
                let title = cli::extract_flag(&args[3..], "--title");
                let artist = cli::extract_flag(&args[3..], "--artist");
                if title.is_none() && artist.is_none() {
                    eprintln!("Usage: cascade rename <slug> [--title NAME] [--artist NAME]");
                    return Ok(());
                }
                return cli::rename(slug, title.as_deref(), artist.as_deref());
            }
            "play" if args.len() >= 3 => {
                let slug = args[2].clone();
                let difficulty = cli::parse_difficulty_flag(&args[3..]);
                let mods = cli::extract_flag(&args[3..], "--mods")
                    .map(|s| game::modifiers::Mods::from_codes(&s))
                    .unwrap_or_default();
                return run_interactive(Some((slug, difficulty, mods)));
            }
            "help" | "--help" | "-h" => return cli::print_help(),
            _ => {}
        }
    }

    run_interactive(None)
}

fn run_interactive(
    start_song: Option<(String, Option<Difficulty>, game::modifiers::Mods)>,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let kb_enhanced = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    )
    .is_ok();

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, start_song);

    if kb_enhanced {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(ref e) = result {
        eprintln!("Error: {}", e);
    }
    result
}

struct Session {
    config: Config,
    songs_dir: PathBuf,
    scores_path: PathBuf,
    scores: ScoreStore,
    achievements_path: PathBuf,
    achievements: AchievementStore,
    sfx: Option<SfxPlayer>,

    app: App,
    menu: MenuScreen,
    song_select: SongSelectScreen,
    gameplay: Option<GameplayScreen>,
    results: Option<ResultsScreen>,
    settings: Option<SettingsScreen>,
    calibrate: Option<CalibrateScreen>,

    last_beatmap_path: Option<PathBuf>,
    last_audio_path: Option<PathBuf>,
    last_song_title: String,
}

impl Session {
    fn load() -> Result<Self> {
        let config = Config::load(&Config::default_path())?;
        let songs_dir = Config::cascade_dir().join("songs");
        std::fs::create_dir_all(&songs_dir)?;
        let scores_path = Config::cascade_dir().join("scores.json");

        let sfx = SfxPlayer::new((config.audio.volume as f32 * 0.6).clamp(0.0, 1.0)).ok();
        let scores = ScoreStore::load(&scores_path);
        let achievements_path = Config::cascade_dir().join("achievements.json");
        let achievements = AchievementStore::load(&achievements_path);

        let initial_difficulty = match config.gameplay.difficulty.as_str() {
            "easy" => Difficulty::Easy,
            "medium" => Difficulty::Medium,
            "expert" => Difficulty::Expert,
            _ => Difficulty::Hard,
        };
        let mut song_select = SongSelectScreen::new(initial_difficulty);
        song_select.load_scores(&scores_path);
        song_select.scan_songs(&songs_dir);

        Ok(Self {
            config,
            songs_dir,
            scores_path,
            scores,
            achievements_path,
            achievements,
            sfx,
            app: App::new(),
            menu: MenuScreen::new(),
            song_select,
            gameplay: None,
            results: None,
            settings: None,
            calibrate: None,
            last_beatmap_path: None,
            last_audio_path: None,
            last_song_title: String::new(),
        })
    }

    fn record_score(&mut self, gp: &GameplayScreen) -> (Option<BestScore>, bool) {
        let slug = cli::song_slug_from_path(&self.last_beatmap_path);
        let diff_name = gp.beatmap.difficulty.to_string();
        let mods_key = gp.mods.storage_key();
        let prev = self
            .scores
            .get_with_mods(&slug, &diff_name, &mods_key)
            .cloned();
        let new_record = BestScore {
            score: gp.state.score,
            max_combo: gp.state.max_combo,
            accuracy: gp.state.accuracy(),
            grade: gp.state.grade().to_string(),
        };
        let is_best = self
            .scores
            .update_if_best_with_mods(&slug, &diff_name, &mods_key, new_record);
        if is_best {
            let _ = self.scores.save(&self.scores_path);
        }
        (prev, is_best)
    }

    fn finalize_to_results(&mut self) {
        let Some(gp) = self.gameplay.take() else {
            return;
        };
        let (prev_best, is_best) = self.record_score(&gp);
        let diff_name = gp.beatmap.difficulty.to_string();
        let unlocked = self.achievements.check(&gp.state, &diff_name, &gp.mods);
        if !unlocked.is_empty() {
            let _ = self.achievements.save(&self.achievements_path);
        }
        self.results = Some(ResultsScreen::new(
            gp.state,
            self.last_song_title.clone(),
            diff_name,
            prev_best,
            is_best,
            unlocked,
            gp.mods,
        ));
    }

    fn play_nav_sfx(&self, action: Action) {
        let Some(sfx) = &self.sfx else { return };
        if self.app.screen == Screen::Gameplay {
            return;
        }
        match action {
            Action::MenuUp | Action::MenuDown | Action::Tab => sfx.play(sfx::nav_tick()),
            Action::MenuSelect => sfx.play(sfx::nav_select()),
            Action::Back | Action::Pause | Action::Quit => sfx.play(sfx::nav_back()),
            _ => {}
        }
    }

    fn launch_gameplay_from_song_select(&mut self) -> Result<bool> {
        let from_song_select = self.app.screen == Screen::SongSelect;
        let beatmap_path = if from_song_select {
            self.song_select.selected_beatmap_path()
        } else {
            self.last_beatmap_path
                .clone()
                .or_else(|| self.song_select.selected_beatmap_path())
        };
        let audio_path = if from_song_select {
            self.song_select.selected_audio_path()
        } else {
            self.last_audio_path
                .clone()
                .or_else(|| self.song_select.selected_audio_path())
        };

        let (Some(bp), Some(ap)) = (beatmap_path, audio_path) else {
            return Ok(false);
        };
        if !bp.exists() || !ap.exists() {
            self.song_select.import_status = Some("Beatmap or audio file not found".to_string());
            return Ok(false);
        }

        let bm = match beatmap::loader::load(&bp) {
            Ok(bm) => bm,
            Err(e) => {
                self.song_select.import_status = Some(format!("Beatmap error: {}", e));
                return Ok(false);
            }
        };

        self.last_song_title = self.song_select.selected_song_title();
        self.last_beatmap_path = Some(bp);
        self.last_audio_path = Some(ap.clone());

        let (samples, sample_rate) =
            audio::analyzer::decode_audio(&ap).unwrap_or_else(|_| (vec![], 44100));

        let mut gp = GameplayScreen::new(
            bm,
            &ap,
            samples,
            sample_rate,
            self.config.audio.offset_ms,
            self.config.gameplay.scroll_speed,
            self.config.audio.volume,
            self.config.gameplay.health_enabled,
            self.config.gameplay.holds_enabled,
            self.song_select.mods.clone(),
        )?;
        gp.start();
        self.gameplay = Some(gp);
        Ok(true)
    }
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    start_song: Option<(String, Option<Difficulty>, game::modifiers::Mods)>,
) -> Result<()> {
    let mut session = Session::load()?;

    if let Some((slug, diff, mods)) = start_song {
        match session
            .song_select
            .songs
            .iter()
            .position(|s| s.slug == slug)
        {
            Some(i) => {
                if let Some(d) = diff {
                    session.song_select.difficulty = d;
                }
                session.song_select.mods = mods;
                session.song_select.selected = session
                    .song_select
                    .filtered_indices
                    .iter()
                    .position(|&fi| fi == i)
                    .unwrap_or(0);
                session.app.navigate(Screen::SongSelect);
                if !session.launch_gameplay_from_song_select()? {
                    eprintln!(
                        "Beatmap file missing for difficulty {}.",
                        session.song_select.difficulty
                    );
                    return Ok(());
                }
                session.app.navigate(Screen::Gameplay);
            }
            None => {
                eprintln!("Song '{}' not found. Use `cascade list`.", slug);
                return Ok(());
            }
        }
    }

    while session.app.running {
        let frame_start = Instant::now();
        process_input(&mut session, terminal)?;
        update(&mut session, terminal);
        draw(&mut session, terminal)?;

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }
    Ok(())
}

fn process_input(
    session: &mut Session,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    let mut event_budget = 16;
    while event_budget > 0 && event::poll(Duration::from_millis(0))? {
        event_budget -= 1;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !matches!(key.kind, KeyEventKind::Press) && session.app.screen != Screen::Gameplay {
            continue;
        }

        if session.app.screen == Screen::SongSelect
            && session.song_select.search_mode
            && session.song_select.handle_search_key(key.code)
        {
            continue;
        }
        if session.app.screen == Screen::SongSelect
            && session.song_select.rename_mode
            && session.song_select.handle_rename_key(key.code)
        {
            continue;
        }
        if session.app.screen == Screen::SongSelect
            && session.song_select.mods_overlay
            && session.song_select.handle_mods_key(key.code)
        {
            continue;
        }
        if session.app.screen == Screen::SongSelect
            && !session.song_select.import_mode
            && !session.song_select.search_mode
            && !session.song_select.rename_mode
            && key.code == KeyCode::Char('/')
        {
            session.song_select.search_mode = true;
            continue;
        }
        if session.app.screen == Screen::SongSelect && session.song_select.import_mode {
            handle_import_input(session, terminal, key.code)?;
            continue;
        }

        let action = derive_action(session, key);
        session.play_nav_sfx(action);
        let result_action = dispatch_action(session, action);

        if let Some(ra) = result_action {
            apply_outcome(session, terminal, ra)?;
        }
    }
    Ok(())
}

fn derive_action(session: &Session, key: crossterm::event::KeyEvent) -> Action {
    match session.app.screen {
        Screen::Gameplay | Screen::Calibrate => {
            input::map_key_gameplay(key, &session.config.keys.lanes)
        }
        Screen::Settings => {
            let menu_action = input::map_key_menu(key);
            if menu_action == Action::None {
                input::map_key_gameplay(key, &session.config.keys.lanes)
            } else {
                menu_action
            }
        }
        _ => input::map_key_menu(key),
    }
}

fn dispatch_action(session: &mut Session, action: Action) -> Option<Action> {
    match session.app.screen {
        Screen::Menu => {
            if action == Action::Quit {
                Some(Action::Quit)
            } else {
                session.menu.handle_action(action)
            }
        }
        Screen::SongSelect => match action {
            Action::Import => {
                session.song_select.import_mode = true;
                session.song_select.import_input.clear();
                None
            }
            Action::Quit => Some(Action::Navigate(Screen::Menu)),
            _ => session.song_select.handle_action(action),
        },
        Screen::Gameplay => session
            .gameplay
            .as_mut()
            .and_then(|gp| gp.handle_action(action)),
        Screen::Results => session
            .results
            .as_mut()
            .and_then(|rs| rs.handle_action(action)),
        Screen::Settings => session.settings.as_mut().and_then(|st| {
            if action == Action::Quit {
                Some(Action::Navigate(Screen::Menu))
            } else {
                st.handle_action(action)
            }
        }),
        Screen::Calibrate => session
            .calibrate
            .as_mut()
            .and_then(|cl| cl.handle_action(action)),
    }
}

fn apply_outcome(
    session: &mut Session,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    outcome: Action,
) -> Result<()> {
    match outcome {
        Action::Quit => session.app.quit(),
        Action::Navigate(screen) => transition_to(session, terminal, screen)?,
        _ => {}
    }
    Ok(())
}

fn transition_to(
    session: &mut Session,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    screen: Screen,
) -> Result<()> {
    match screen {
        Screen::SongSelect => {
            session.song_select.scan_songs(&session.songs_dir);
            session.gameplay = None;
        }
        Screen::Gameplay => {
            draw_loading(terminal)?;
            match session.launch_gameplay_from_song_select() {
                Ok(true) => {}
                Ok(false) => {
                    session.app.navigate(Screen::SongSelect);
                    return Ok(());
                }
                Err(e) => {
                    session.song_select.import_status = Some(format!("Audio error: {}", e));
                    session.app.navigate(Screen::SongSelect);
                    return Ok(());
                }
            }
        }
        Screen::Results => session.finalize_to_results(),
        Screen::Settings => {
            let cfg = Config::load(&Config::default_path()).unwrap_or_default();
            session.settings = Some(SettingsScreen::new(cfg));
        }
        Screen::Calibrate => {
            let cfg = Config::load(&Config::default_path()).unwrap_or_default();
            match CalibrateScreen::new(cfg) {
                Ok(mut cl) => {
                    cl.start();
                    session.calibrate = Some(cl);
                }
                Err(_) => {
                    session.app.navigate(Screen::Settings);
                    return Ok(());
                }
            }
        }
        Screen::Menu => {
            session.last_beatmap_path = None;
            session.last_audio_path = None;
        }
    }
    session.app.navigate(screen);
    Ok(())
}

fn draw_loading(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let msg = "Loading...";
        let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
        let y = area.y + area.height / 2;
        buf.set_string(x, y, msg, Style::default().fg(Color::Rgb(140, 140, 140)));
    })?;
    Ok(())
}

fn handle_import_input(
    session: &mut Session,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    code: KeyCode,
) -> Result<()> {
    match code {
        KeyCode::Char(c) => {
            session.song_select.import_input.push(c);
        }
        KeyCode::Backspace => {
            session.song_select.import_input.pop();
        }
        KeyCode::Esc => {
            session.song_select.import_mode = false;
            session.song_select.import_input.clear();
        }
        KeyCode::Enter => {
            let raw = session.song_select.import_input.clone();
            session.song_select.import_mode = false;
            session.song_select.import_input.clear();
            if raw.is_empty() {
                return Ok(());
            }
            run_import(session, terminal, PathBuf::from(raw.trim()))?;
        }
        _ => {}
    }
    Ok(())
}

fn run_import(
    session: &mut Session,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    file_path: PathBuf,
) -> Result<()> {
    session.song_select.import_status = Some("Importing...".to_string());
    terminal.draw(|frame| session.song_select.render(frame, frame.area()))?;

    let song = match audio::import::import_local_file(&file_path, &session.songs_dir) {
        Ok(s) => s,
        Err(e) => {
            session.song_select.import_status = Some(format!("Error: {}", e));
            return Ok(());
        }
    };

    session.song_select.import_status = Some(format!("Generating beatmaps for {}...", song.title));
    terminal.draw(|frame| session.song_select.render(frame, frame.area()))?;

    match audio::analyzer::decode_audio(&song.audio_path) {
        Ok((samples, sample_rate)) => {
            let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u64;
            let audio_filename = song
                .audio_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let meta = beatmap::types::SongMeta {
                title: song.title.clone(),
                artist: song.artist.clone(),
                audio_file: audio_filename,
                bpm: 120,
                duration_ms,
            };
            let beatmaps = beatmap::generator::generate_all_beatmaps(&samples, sample_rate, meta);
            for bm in &beatmaps {
                let path = song.dir.join(bm.difficulty.filename());
                let _ = beatmap::loader::save(bm, &path);
            }
            session.song_select.import_status = Some(format!("Imported: {}", song.title));
        }
        Err(e) => {
            session.song_select.import_status = Some(format!("Decode error: {}", e));
        }
    }
    session.song_select.scan_songs(&session.songs_dir);
    Ok(())
}

fn update(session: &mut Session, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    match session.app.screen {
        Screen::Gameplay => {
            if let Some(gp) = &mut session.gameplay {
                gp.update();
                if gp.finished {
                    session.finalize_to_results();
                    session.app.navigate(Screen::Results);
                }
            }
        }
        Screen::Calibrate => {
            if let Some(cl) = &mut session.calibrate {
                cl.update();
            }
        }
        Screen::Results => {
            if let Some(rs) = &mut session.results {
                rs.update();
            }
        }
        Screen::Menu => {
            let area = terminal.get_frame().area();
            session.menu.update(area);
        }
        _ => {}
    }
}

fn draw(
    session: &mut Session,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        match session.app.screen {
            Screen::Menu => session.menu.render(frame, area),
            Screen::SongSelect => session.song_select.render(frame, area),
            Screen::Gameplay => {
                if let Some(gp) = &mut session.gameplay {
                    gp.render(frame, area);
                }
            }
            Screen::Results => {
                if let Some(rs) = &session.results {
                    rs.render(frame, area);
                }
            }
            Screen::Settings => {
                if let Some(st) = &session.settings {
                    st.render(frame, area);
                }
            }
            Screen::Calibrate => {
                if let Some(cl) = &session.calibrate {
                    cl.render(frame, area);
                }
            }
        }
    })?;
    Ok(())
}
