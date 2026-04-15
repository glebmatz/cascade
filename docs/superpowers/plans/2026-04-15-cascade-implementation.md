# Cascade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a terminal-based rhythm game with Guitar Hero-style perspective highway, 5 lanes, audio playback, FFT visualizer, and YouTube import with auto-generated beatmaps.

**Architecture:** Game loop at ~60 FPS on main thread. Audio playback on separate thread via rodio, exposes position through `Arc<AtomicU64>`. FFT analysis on third thread feeds visualizer. Screens (menu, song select, gameplay, results, settings) managed by an App state machine. Beatmaps are JSON files generated from audio onset detection.

**Tech Stack:** Rust, ratatui, crossterm, rodio, symphonia, rustfft, serde, toml, tokio

---

## File Map

```
Cargo.toml
src/
├── main.rs                  — entry point, terminal setup/teardown, app loop
├── app.rs                   — App struct, Screen enum, routing between screens
├── input.rs                 — KeyEvent → Action mapping
├── config.rs                — Config struct, TOML read/write, defaults
│
├── beatmap/
│   ├── mod.rs
│   ├── types.rs             — Beatmap, Note, Difficulty, SongMeta structs
│   ├── loader.rs            — JSON read/write for beatmaps
│   └── generator.rs         — onset detection, BPM, note placement
│
├── audio/
│   ├── mod.rs
│   ├── player.rs            — AudioPlayer: play/pause/stop, position tracking
│   ├── analyzer.rs          — FFT analyzer: spectrum data for visualizer
│   └── import.rs            — yt-dlp wrapper: download audio from YouTube
│
├── game/
│   ├── mod.rs
│   ├── state.rs             — GameState: score, combo, judgements, accuracy
│   ├── hit_judge.rs         — HitJudge: timing windows, judgement logic
│   └── highway.rs           — Highway: note positions, scroll speed, lane mapping
│
├── ui/
│   ├── mod.rs
│   ├── highway_render.rs    — trapezoid perspective rendering, note glyphs
│   ├── visualizer.rs        — FFT → ▁▂▃▅▇█ waves and ░▒▓█ side blocks
│   ├── hud.rs               — combo, score, accuracy, judgement feedback
│   └── widgets.rs           — reusable: centered list, progress bar, text input
│
└── screens/
    ├── mod.rs
    ├── menu.rs              — Main Menu screen
    ├── song_select.rs       — Song Select screen with import
    ├── gameplay.rs          — Gameplay screen orchestration
    ├── results.rs           — Results screen
    └── settings.rs          — Settings screen
```

---

### Task 1: Project Scaffold + App Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/app.rs`
- Create: `src/input.rs`

- [ ] **Step 1: Initialize Cargo project**

```bash
cd /Users/gleb/Desktop/work/cascade_game
cargo init --name cascade
```

- [ ] **Step 2: Set up Cargo.toml with all dependencies**

Replace `Cargo.toml`:

```toml
[package]
name = "cascade"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
rodio = { version = "0.20", features = ["mp3", "vorbis", "flac"] }
symphonia = { version = "0.5", features = ["mp3", "ogg", "flac"] }
rustfft = "6.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
dirs = "6"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Write src/app.rs — App struct and Screen enum**

```rust
use anyhow::Result;

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
    GameKey(usize), // lane 0..4
    Pause,
    Back,
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
```

- [ ] **Step 4: Write src/input.rs — key mapping**

```rust
use crossterm::event::{KeyCode, KeyEvent};
use crate::app::Action;

pub fn map_key(key: KeyEvent, lanes: &[char; 5]) -> Action {
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Up | KeyCode::Char('k') => Action::MenuUp,
        KeyCode::Down | KeyCode::Char('j') => Action::MenuDown,
        KeyCode::Enter => Action::MenuSelect,
        KeyCode::Esc => Action::Pause,
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
```

- [ ] **Step 5: Write src/main.rs — terminal setup, basic loop, teardown**

```rust
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
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    // Terminal teardown
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

        // Input
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

        // Render
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

        // Frame timing
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}
```

- [ ] **Step 6: Build and run to verify**

```bash
cargo build
cargo run
```

Expected: terminal shows "CASCADE — Press Q to quit", Q exits cleanly.

- [ ] **Step 7: Commit**

```bash
git init
echo -e "target/\n.superpowers/" > .gitignore
git add Cargo.toml Cargo.lock src/ .gitignore
git commit -m "feat: project scaffold with app skeleton and game loop"
```

---

### Task 2: Config System

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs` — load config at startup
- Test: `tests/config_test.rs`

- [ ] **Step 1: Write failing test for config defaults and TOML round-trip**

Create `tests/config_test.rs`:

```rust
use cascade::config::Config;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_default_config() {
    let config = Config::default();
    assert_eq!(config.gameplay.scroll_speed, 1.0);
    assert_eq!(config.gameplay.difficulty, "hard");
    assert_eq!(config.keys.lanes, ['d', 'f', ' ', 'j', 'k']);
    assert_eq!(config.audio.volume, 0.8);
    assert_eq!(config.audio.offset_ms, 0);
    assert_eq!(config.display.fps, 60);
}

#[test]
fn test_config_save_and_load() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let mut config = Config::default();
    config.gameplay.scroll_speed = 1.5;
    config.audio.offset_ms = -30;

    config.save(&path).unwrap();
    let loaded = Config::load(&path).unwrap();

    assert_eq!(loaded.gameplay.scroll_speed, 1.5);
    assert_eq!(loaded.audio.offset_ms, -30);
    assert_eq!(loaded.keys.lanes, ['d', 'f', ' ', 'j', 'k']);
}

#[test]
fn test_config_load_missing_file_returns_default() {
    let config = Config::load(Path::new("/nonexistent/config.toml")).unwrap();
    assert_eq!(config.gameplay.scroll_speed, 1.0);
}
```

Add `lib.rs` so tests can import:

Create `src/lib.rs`:

```rust
pub mod config;
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test config_test
```

Expected: FAIL — module `config` not found.

- [ ] **Step 3: Implement src/config.rs**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub gameplay: GameplayConfig,
    pub keys: KeysConfig,
    pub audio: AudioConfig,
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameplayConfig {
    pub scroll_speed: f64,
    pub difficulty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysConfig {
    pub lanes: [char; 5],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub volume: f64,
    pub offset_ms: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub fps: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gameplay: GameplayConfig {
                scroll_speed: 1.0,
                difficulty: "hard".to_string(),
            },
            keys: KeysConfig {
                lanes: ['d', 'f', ' ', 'j', 'k'],
            },
            audio: AudioConfig {
                volume: 0.8,
                offset_ms: 0,
            },
            display: DisplayConfig { fps: 60 },
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn cascade_dir() -> std::path::PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".cascade")
    }

    pub fn default_path() -> std::path::PathBuf {
        Self::cascade_dir().join("config.toml")
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --test config_test
```

Expected: 3 tests PASS.

- [ ] **Step 5: Wire config into main.rs**

Add to `src/lib.rs`:

```rust
pub mod config;
```

In `src/main.rs`, add `mod config;` and load config at startup:

```rust
mod config;
// ... in run():
let config = config::Config::load(&config::Config::default_path())?;
let lanes = config.keys.lanes;
```

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/lib.rs tests/config_test.rs src/main.rs
git commit -m "feat: config system with TOML persistence"
```

---

### Task 3: Beatmap Types + Loader

**Files:**
- Create: `src/beatmap/mod.rs`
- Create: `src/beatmap/types.rs`
- Create: `src/beatmap/loader.rs`
- Test: `tests/beatmap_test.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/beatmap_test.rs`:

```rust
use cascade::beatmap::types::{Beatmap, Note, SongMeta, Difficulty};
use cascade::beatmap::loader;
use tempfile::TempDir;

#[test]
fn test_beatmap_serialization_roundtrip() {
    let beatmap = Beatmap {
        version: 1,
        song: SongMeta {
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            audio_file: "audio.mp3".to_string(),
            bpm: 120,
            duration_ms: 180000,
        },
        difficulty: Difficulty::Hard,
        notes: vec![
            Note { time_ms: 1000, lane: 0 },
            Note { time_ms: 1200, lane: 2 },
            Note { time_ms: 1200, lane: 4 },
            Note { time_ms: 1500, lane: 1 },
        ],
    };

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("hard.json");

    loader::save(&beatmap, &path).unwrap();
    let loaded = loader::load(&path).unwrap();

    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.song.title, "Test Song");
    assert_eq!(loaded.song.bpm, 120);
    assert_eq!(loaded.difficulty, Difficulty::Hard);
    assert_eq!(loaded.notes.len(), 4);
    assert_eq!(loaded.notes[0].time_ms, 1000);
    assert_eq!(loaded.notes[0].lane, 0);
    assert_eq!(loaded.notes[2].lane, 4);
}

#[test]
fn test_difficulty_ordering() {
    assert!(Difficulty::Easy < Difficulty::Medium);
    assert!(Difficulty::Medium < Difficulty::Hard);
    assert!(Difficulty::Hard < Difficulty::Expert);
}

#[test]
fn test_difficulty_filename() {
    assert_eq!(Difficulty::Easy.filename(), "easy.json");
    assert_eq!(Difficulty::Expert.filename(), "expert.json");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test beatmap_test
```

Expected: FAIL — module not found.

- [ ] **Step 3: Implement beatmap types**

Create `src/beatmap/mod.rs`:

```rust
pub mod types;
pub mod loader;
```

Create `src/beatmap/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beatmap {
    pub version: u32,
    pub song: SongMeta,
    pub difficulty: Difficulty,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongMeta {
    pub title: String,
    pub artist: String,
    pub audio_file: String,
    pub bpm: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
}

impl Difficulty {
    pub fn filename(&self) -> String {
        format!("{}.json", match self {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
            Difficulty::Expert => "expert",
        })
    }

    pub fn all() -> &'static [Difficulty] {
        &[Difficulty::Easy, Difficulty::Medium, Difficulty::Hard, Difficulty::Expert]
    }
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "Easy"),
            Difficulty::Medium => write!(f, "Medium"),
            Difficulty::Hard => write!(f, "Hard"),
            Difficulty::Expert => write!(f, "Expert"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub time_ms: u64,
    pub lane: u8,
}
```

- [ ] **Step 4: Implement loader**

Create `src/beatmap/loader.rs`:

```rust
use anyhow::Result;
use std::path::Path;
use super::types::Beatmap;

pub fn load(path: &Path) -> Result<Beatmap> {
    let content = std::fs::read_to_string(path)?;
    let beatmap: Beatmap = serde_json::from_str(&content)?;
    Ok(beatmap)
}

pub fn save(beatmap: &Beatmap, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(beatmap)?;
    std::fs::write(path, content)?;
    Ok(())
}
```

- [ ] **Step 5: Export from lib.rs and run tests**

Add to `src/lib.rs`:

```rust
pub mod beatmap;
```

```bash
cargo test --test beatmap_test
```

Expected: 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/beatmap/ src/lib.rs tests/beatmap_test.rs
git commit -m "feat: beatmap types and JSON loader"
```

---

### Task 4: Game State + Hit Detection

**Files:**
- Create: `src/game/mod.rs`
- Create: `src/game/state.rs`
- Create: `src/game/hit_judge.rs`
- Create: `src/game/highway.rs`
- Test: `tests/game_test.rs`

- [ ] **Step 1: Write failing tests for hit judge and game state**

Create `tests/game_test.rs`:

```rust
use cascade::game::hit_judge::{HitJudge, Judgement};
use cascade::game::state::GameState;

#[test]
fn test_perfect_hit() {
    let judge = HitJudge::new(0); // no offset
    assert_eq!(judge.judge(1000, 1000), Judgement::Perfect);
    assert_eq!(judge.judge(1000, 1025), Judgement::Perfect);
    assert_eq!(judge.judge(1000, 975), Judgement::Perfect);
}

#[test]
fn test_great_hit() {
    let judge = HitJudge::new(0);
    assert_eq!(judge.judge(1000, 1045), Judgement::Great);
    assert_eq!(judge.judge(1000, 955), Judgement::Great);
}

#[test]
fn test_good_hit() {
    let judge = HitJudge::new(0);
    assert_eq!(judge.judge(1000, 1080), Judgement::Good);
    assert_eq!(judge.judge(1000, 920), Judgement::Good);
}

#[test]
fn test_miss() {
    let judge = HitJudge::new(0);
    assert_eq!(judge.judge(1000, 1150), Judgement::Miss);
    assert_eq!(judge.judge(1000, 800), Judgement::Miss);
}

#[test]
fn test_offset_shifts_window() {
    let judge = HitJudge::new(50); // +50ms offset
    // note at 1000ms, pressed at 1050ms — with offset this is effectively 1000ms
    assert_eq!(judge.judge(1000, 1050), Judgement::Perfect);
    // without offset this would be Great, but with +50ms it's still Perfect
    assert_eq!(judge.judge(1000, 1075), Judgement::Perfect);
}

#[test]
fn test_game_state_combo() {
    let mut state = GameState::new();

    state.register_judgement(Judgement::Perfect);
    assert_eq!(state.combo, 1);
    assert_eq!(state.max_combo, 1);
    assert_eq!(state.score, 300); // 300 * (1 + 0/50) at combo 0 when hit

    state.register_judgement(Judgement::Great);
    assert_eq!(state.combo, 2);

    state.register_judgement(Judgement::Miss);
    assert_eq!(state.combo, 0);
    assert_eq!(state.max_combo, 2);
}

#[test]
fn test_game_state_score_multiplier() {
    let mut state = GameState::new();
    // Build up combo to 50
    for _ in 0..50 {
        state.register_judgement(Judgement::Perfect);
    }
    assert_eq!(state.combo, 50);
    // Next perfect: 300 * (1 + 50/50) = 300 * 2 = 600
    let score_before = state.score;
    state.register_judgement(Judgement::Perfect);
    assert_eq!(state.score - score_before, 600);
}

#[test]
fn test_game_state_accuracy() {
    let mut state = GameState::new();
    state.register_judgement(Judgement::Perfect); // 300/300
    state.register_judgement(Judgement::Miss);    // 0/300
    let acc = state.accuracy();
    assert!((acc - 50.0).abs() < 0.1);
}

#[test]
fn test_game_state_grade() {
    let mut state = GameState::new();
    // All perfects → S
    for _ in 0..10 {
        state.register_judgement(Judgement::Perfect);
    }
    assert_eq!(state.grade(), "S");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test game_test
```

Expected: FAIL — modules not found.

- [ ] **Step 3: Implement hit_judge.rs**

Create `src/game/mod.rs`:

```rust
pub mod state;
pub mod hit_judge;
pub mod highway;
```

Create `src/game/hit_judge.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judgement {
    Perfect,
    Great,
    Good,
    Miss,
}

impl Judgement {
    pub fn base_points(&self) -> u64 {
        match self {
            Judgement::Perfect => 300,
            Judgement::Great => 200,
            Judgement::Good => 100,
            Judgement::Miss => 0,
        }
    }

    pub fn max_points() -> u64 {
        300
    }

    pub fn label(&self) -> &'static str {
        match self {
            Judgement::Perfect => "PERFECT",
            Judgement::Great => "GREAT",
            Judgement::Good => "GOOD",
            Judgement::Miss => "MISS",
        }
    }
}

pub struct HitJudge {
    offset_ms: i64,
}

impl HitJudge {
    pub fn new(offset_ms: i32) -> Self {
        Self {
            offset_ms: offset_ms as i64,
        }
    }

    /// Judge a hit. `note_time_ms` is when the note should be hit,
    /// `press_time_ms` is when the player pressed.
    pub fn judge(&self, note_time_ms: u64, press_time_ms: u64) -> Judgement {
        let adjusted_press = press_time_ms as i64 - self.offset_ms;
        let diff = (adjusted_press - note_time_ms as i64).unsigned_abs();

        if diff <= 30 {
            Judgement::Perfect
        } else if diff <= 60 {
            Judgement::Great
        } else if diff <= 100 {
            Judgement::Good
        } else {
            Judgement::Miss
        }
    }

    /// Check if a note is past the miss window (should auto-miss)
    pub fn is_expired(&self, note_time_ms: u64, current_time_ms: u64) -> bool {
        current_time_ms as i64 - self.offset_ms > note_time_ms as i64 + 100
    }
}
```

- [ ] **Step 4: Implement state.rs**

Create `src/game/state.rs`:

```rust
use super::hit_judge::Judgement;

pub struct GameState {
    pub score: u64,
    pub combo: u32,
    pub max_combo: u32,
    pub total_notes: u32,
    pub earned_points: u64,
    pub max_possible_points: u64,
    pub last_judgement: Option<Judgement>,
    pub judgement_counts: [u32; 4], // Perfect, Great, Good, Miss
}

impl GameState {
    pub fn new() -> Self {
        Self {
            score: 0,
            combo: 0,
            max_combo: 0,
            total_notes: 0,
            earned_points: 0,
            max_possible_points: 0,
            last_judgement: None,
            judgement_counts: [0; 4],
        }
    }

    pub fn register_judgement(&mut self, judgement: Judgement) {
        self.total_notes += 1;
        self.max_possible_points += Judgement::max_points();
        self.last_judgement = Some(judgement);

        let idx = match judgement {
            Judgement::Perfect => 0,
            Judgement::Great => 1,
            Judgement::Good => 2,
            Judgement::Miss => 3,
        };
        self.judgement_counts[idx] += 1;

        if judgement == Judgement::Miss {
            self.combo = 0;
        } else {
            let multiplier = 1.0 + (self.combo as f64 / 50.0);
            let multiplier = multiplier.min(5.0);
            let points = (judgement.base_points() as f64 * multiplier) as u64;
            self.score += points;
            self.earned_points += judgement.base_points();
            self.combo += 1;
            if self.combo > self.max_combo {
                self.max_combo = self.combo;
            }
        }
    }

    pub fn accuracy(&self) -> f64 {
        if self.max_possible_points == 0 {
            return 100.0;
        }
        (self.earned_points as f64 / self.max_possible_points as f64) * 100.0
    }

    pub fn grade(&self) -> &'static str {
        let acc = self.accuracy();
        if acc >= 95.0 {
            "S"
        } else if acc >= 90.0 {
            "A"
        } else if acc >= 80.0 {
            "B"
        } else if acc >= 70.0 {
            "C"
        } else {
            "D"
        }
    }
}
```

- [ ] **Step 5: Create highway.rs stub**

Create `src/game/highway.rs`:

```rust
use crate::beatmap::types::Note;

pub struct Highway {
    pub scroll_speed: f64,
    pub visible_notes: Vec<VisibleNote>,
}

pub struct VisibleNote {
    pub note_index: usize,
    pub lane: u8,
    pub time_ms: u64,
    /// 0.0 = at hit zone, 1.0 = top of screen, negative = past hit zone
    pub position: f64,
    pub hit: bool,
}

impl Highway {
    pub fn new(scroll_speed: f64) -> Self {
        Self {
            scroll_speed,
            visible_notes: Vec::new(),
        }
    }

    /// Update visible notes based on current audio position.
    /// `look_ahead_ms` determines how far ahead to show notes.
    pub fn update(&mut self, notes: &[Note], current_time_ms: u64, look_ahead_ms: u64, hit_notes: &[bool]) {
        self.visible_notes.clear();
        let look_ahead = (look_ahead_ms as f64 / self.scroll_speed) as u64;

        for (i, note) in notes.iter().enumerate() {
            if hit_notes.get(i).copied().unwrap_or(false) {
                continue;
            }
            if note.time_ms > current_time_ms + look_ahead {
                continue;
            }
            if (current_time_ms as i64 - note.time_ms as i64) > 500 {
                continue; // way past, skip
            }

            let time_diff = note.time_ms as f64 - current_time_ms as f64;
            let position = time_diff / look_ahead as f64;

            self.visible_notes.push(VisibleNote {
                note_index: i,
                lane: note.lane,
                time_ms: note.time_ms,
                position,
                hit: false,
            });
        }
    }
}
```

- [ ] **Step 6: Export from lib.rs, run all tests**

Add to `src/lib.rs`:

```rust
pub mod game;
```

```bash
cargo test --test game_test
```

Expected: all 9 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/game/ src/lib.rs tests/game_test.rs
git commit -m "feat: game state, hit judge, and highway logic"
```

---

### Task 5: Audio Player

**Files:**
- Create: `src/audio/mod.rs`
- Create: `src/audio/player.rs`

- [ ] **Step 1: Implement audio player with position tracking**

Create `src/audio/mod.rs`:

```rust
pub mod player;
pub mod analyzer;
pub mod import;
```

Create `src/audio/player.rs`:

```rust
use anyhow::Result;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct AudioPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    start_time: Option<Instant>,
    pause_elapsed_ms: u64,
    position_ms: Arc<AtomicU64>,
    is_playing: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        sink.pause();

        Ok(Self {
            _stream: stream,
            _stream_handle: stream_handle,
            sink,
            start_time: None,
            pause_elapsed_ms: 0,
            position_ms: Arc::new(AtomicU64::new(0)),
            is_playing: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let file = BufReader::new(File::open(path)?);
        let source = Decoder::new(file)?;
        self.sink.append(source);
        self.sink.pause();
        self.start_time = None;
        self.pause_elapsed_ms = 0;
        self.position_ms.store(0, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn play(&mut self) {
        self.sink.play();
        self.start_time = Some(Instant::now());
        self.is_playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&mut self) {
        if self.is_playing.load(Ordering::Relaxed) {
            self.pause_elapsed_ms = self.position_ms();
            self.sink.pause();
            self.start_time = None;
            self.is_playing.store(false, Ordering::Relaxed);
        }
    }

    pub fn resume(&mut self) {
        if !self.is_playing.load(Ordering::Relaxed) {
            self.sink.play();
            self.start_time = Some(Instant::now());
            self.is_playing.store(true, Ordering::Relaxed);
        }
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        self.start_time = None;
        self.pause_elapsed_ms = 0;
        self.position_ms.store(0, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
    }

    pub fn position_ms(&self) -> u64 {
        if let Some(start) = self.start_time {
            self.pause_elapsed_ms + start.elapsed().as_millis() as u64
        } else {
            self.pause_elapsed_ms
        }
    }

    /// Call this each frame to update the shared atomic position
    pub fn update_position(&self) {
        self.position_ms.store(self.position_ms(), Ordering::Relaxed);
    }

    pub fn shared_position(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.position_ms)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.sink.set_volume(volume);
    }
}
```

- [ ] **Step 2: Create stubs for analyzer and import**

Create `src/audio/analyzer.rs`:

```rust
use rustfft::{FftPlanner, num_complex::Complex};

pub struct SpectrumData {
    /// Normalized magnitudes for frequency bands (0.0 - 1.0)
    pub bands: Vec<f32>,
    /// Overall energy (0.0 - 1.0)
    pub energy: f32,
}

impl SpectrumData {
    pub fn empty(num_bands: usize) -> Self {
        Self {
            bands: vec![0.0; num_bands],
            energy: 0.0,
        }
    }
}
```

Create `src/audio/import.rs`:

```rust
// YouTube import — implemented in Task 11
```

- [ ] **Step 3: Export from lib.rs, build to verify**

Add to `src/lib.rs`:

```rust
pub mod audio;
```

```bash
cargo build
```

Expected: builds successfully.

- [ ] **Step 4: Commit**

```bash
git add src/audio/ src/lib.rs
git commit -m "feat: audio player with rodio and position tracking"
```

---

### Task 6: UI — Highway Renderer (Trapezoid Perspective)

**Files:**
- Create: `src/ui/mod.rs`
- Create: `src/ui/highway_render.rs`
- Create: `src/ui/hud.rs`
- Create: `src/ui/visualizer.rs`
- Create: `src/ui/widgets.rs`

This is visual rendering code — tested manually by running the app.

- [ ] **Step 1: Create ui module structure**

Create `src/ui/mod.rs`:

```rust
pub mod highway_render;
pub mod visualizer;
pub mod hud;
pub mod widgets;
```

- [ ] **Step 2: Implement highway_render.rs — trapezoid perspective**

Create `src/ui/highway_render.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::game::highway::VisibleNote;

const LANE_COUNT: usize = 5;

/// Characters for notes at different distances
const NOTE_FAR: &str = "◇";
const NOTE_MID: &str = "◈";
const NOTE_NEAR: &str = "◆";

/// Trapezoid highway widget
pub struct HighwayWidget<'a> {
    pub notes: &'a [VisibleNote],
    pub lane_labels: [&'a str; LANE_COUNT],
    pub hit_flash: [u8; LANE_COUNT], // 0 = no flash, >0 = frames remaining
}

impl<'a> HighwayWidget<'a> {
    pub fn new(notes: &'a [VisibleNote]) -> Self {
        Self {
            notes,
            lane_labels: ["D", "F", "▽", "J", "K"],
            hit_flash: [0; LANE_COUNT],
        }
    }

    pub fn with_hit_flash(mut self, flash: [u8; LANE_COUNT]) -> Self {
        self.hit_flash = flash;
        self
    }

    /// Map a lane (0..4) and vertical position (0.0=hit zone, 1.0=top) to terminal coords
    fn lane_x(&self, lane: usize, position: f64, area: Rect) -> u16 {
        let center_x = area.x + area.width / 2;
        let lane_offset = lane as f64 - 2.0; // -2, -1, 0, 1, 2

        // At hit zone (position=0): wide spacing
        let bottom_spacing = (area.width as f64 / 8.0).max(4.0);
        // At top (position=1): narrow spacing (perspective)
        let top_spacing = bottom_spacing * 0.3;

        let spacing = bottom_spacing + (top_spacing - bottom_spacing) * position.clamp(0.0, 1.0);
        (center_x as f64 + lane_offset * spacing) as u16
    }

    fn note_char(position: f64) -> &'static str {
        if position > 0.7 {
            NOTE_FAR
        } else if position > 0.3 {
            NOTE_MID
        } else {
            NOTE_NEAR
        }
    }

    fn note_style(position: f64) -> Style {
        let brightness = ((1.0 - position) * 255.0).clamp(80.0, 255.0) as u8;
        Style::default().fg(Color::Rgb(brightness, brightness, brightness))
    }

    fn wall_char(position: f64) -> &'static str {
        if position > 0.7 { "╲" } else if position > 0.3 { "╲" } else { "╲" }
    }
}

impl<'a> Widget for HighwayWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 5 {
            return;
        }

        let highway_height = area.height.saturating_sub(2); // leave room for hit zone + labels

        // Draw lane dividers and walls
        for row in 0..highway_height {
            let position = 1.0 - (row as f64 / highway_height as f64);
            let y = area.y + row;

            // Left wall
            let left_x = self.lane_x(0, position, area).saturating_sub(2);
            if left_x >= area.x && left_x < area.x + area.width {
                let brightness = ((1.0 - position) * 150.0).clamp(30.0, 150.0) as u8;
                buf.set_string(left_x, y, "╲", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
            }

            // Right wall
            let right_x = self.lane_x(4, position, area) + 2;
            if right_x >= area.x && right_x < area.x + area.width {
                let brightness = ((1.0 - position) * 150.0).clamp(30.0, 150.0) as u8;
                buf.set_string(right_x, y, "╱", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
            }

            // Lane dots
            for lane in 0..LANE_COUNT {
                let x = self.lane_x(lane, position, area);
                if x >= area.x && x < area.x + area.width {
                    let brightness = ((1.0 - position) * 80.0).clamp(20.0, 80.0) as u8;
                    buf.set_string(x, y, "·", Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
                }
            }
        }

        // Draw notes
        for note in self.notes {
            if note.position < -0.05 || note.position > 1.0 {
                continue;
            }
            let row = ((1.0 - note.position) * highway_height as f64) as u16;
            let y = area.y + row.min(highway_height.saturating_sub(1));
            let x = self.lane_x(note.lane as usize, note.position, area);

            if x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height {
                buf.set_string(x, y, Self::note_char(note.position), Self::note_style(note.position));
            }
        }

        // Draw hit zone line
        let hit_y = area.y + highway_height;
        if hit_y < area.y + area.height {
            for x in area.x..area.x + area.width {
                buf.set_string(x, hit_y, "━", Style::default().fg(Color::Rgb(100, 100, 100)));
            }

            // Lane receptors
            for (i, label) in self.lane_labels.iter().enumerate() {
                let x = self.lane_x(i, 0.0, area);
                if x >= area.x + 1 && x + 2 < area.x + area.width {
                    let style = if self.hit_flash[i] > 0 {
                        Style::default().fg(Color::White).bold()
                    } else {
                        Style::default().fg(Color::Rgb(180, 180, 180))
                    };
                    buf.set_string(x - 1, hit_y, &format!("[{}]", label), style);
                }
            }
        }
    }
}
```

- [ ] **Step 3: Implement hud.rs**

Create `src/ui/hud.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Widget};
use crate::game::state::GameState;
use crate::game::hit_judge::Judgement;

pub struct HudTop<'a> {
    pub state: &'a GameState,
}

impl<'a> Widget for HudTop<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 30 {
            return;
        }

        // Combo left
        let combo_text = if self.state.combo > 1 {
            format!("x{} COMBO", self.state.combo)
        } else {
            String::new()
        };
        buf.set_string(
            area.x + 2,
            area.y,
            &combo_text,
            Style::default().fg(Color::Rgb(180, 180, 180)),
        );

        // Score right
        let score_text = format!("SCORE {:>8}", self.state.score);
        let score_x = area.x + area.width - score_text.len() as u16 - 2;
        buf.set_string(
            score_x,
            area.y,
            &score_text,
            Style::default().fg(Color::Rgb(180, 180, 180)),
        );
    }
}

pub struct HudBottom<'a> {
    pub state: &'a GameState,
    pub song_title: &'a str,
    pub progress: f64, // 0.0 - 1.0
    pub difficulty: &'a str,
}

impl<'a> Widget for HudBottom<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 30 {
            return;
        }

        // Judgement feedback (row 0)
        if let Some(judgement) = self.state.last_judgement {
            let (label, style) = match judgement {
                Judgement::Perfect => ("✦ PERFECT ✦", Style::default().fg(Color::White).bold()),
                Judgement::Great => ("GREAT", Style::default().fg(Color::Rgb(200, 200, 200))),
                Judgement::Good => ("GOOD", Style::default().fg(Color::Rgb(140, 140, 140))),
                Judgement::Miss => ("MISS", Style::default().fg(Color::Rgb(80, 80, 80))),
            };
            let x = area.x + (area.width - label.len() as u16) / 2;
            buf.set_string(x, area.y, label, style);
        }

        // Bottom info (row 1): song title, accuracy, difficulty
        if area.height >= 2 {
            let y = area.y + 1;
            let info = format!(
                " ▸▸ {}    ACC {:.1}%    {}",
                self.song_title,
                self.state.accuracy(),
                self.difficulty,
            );
            buf.set_string(
                area.x + 1,
                y,
                &info,
                Style::default().fg(Color::Rgb(100, 100, 100)),
            );

            // Progress bar right side
            let bar_width = 12u16;
            let bar_x = area.x + area.width - bar_width - 2;
            let filled = (self.progress * bar_width as f64) as u16;
            for i in 0..bar_width {
                let ch = if i < filled { "━" } else { "─" };
                let style = if i < filled {
                    Style::default().fg(Color::Rgb(150, 150, 150))
                } else {
                    Style::default().fg(Color::Rgb(50, 50, 50))
                };
                buf.set_string(bar_x + i, y, ch, style);
            }
        }
    }
}
```

- [ ] **Step 4: Implement visualizer.rs**

Create `src/ui/visualizer.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::audio::analyzer::SpectrumData;

const WAVE_CHARS: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
const BLOCK_CHARS: &[&str] = &["░", "▒", "▓", "█"];

/// Top wave visualizer
pub struct WaveVisualizer<'a> {
    pub spectrum: &'a SpectrumData,
}

impl<'a> Widget for WaveVisualizer<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        for x in 0..area.width {
            let band_idx = (x as usize * self.spectrum.bands.len()) / area.width as usize;
            let band_idx = band_idx.min(self.spectrum.bands.len().saturating_sub(1));
            let value = self.spectrum.bands.get(band_idx).copied().unwrap_or(0.0);
            let char_idx = (value * (WAVE_CHARS.len() - 1) as f32) as usize;
            let char_idx = char_idx.min(WAVE_CHARS.len() - 1);

            let brightness = (value * 180.0 + 40.0).min(220.0) as u8;
            buf.set_string(
                area.x + x,
                area.y + area.height - 1,
                WAVE_CHARS[char_idx],
                Style::default().fg(Color::Rgb(brightness, brightness, brightness)),
            );
        }
    }
}

/// Side block visualizer
pub struct BlockVisualizer<'a> {
    pub spectrum: &'a SpectrumData,
    pub side: Side,
}

pub enum Side {
    Left,
    Right,
}

impl<'a> Widget for BlockVisualizer<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        for y in 0..area.height {
            let band_idx = (y as usize * self.spectrum.bands.len()) / area.height as usize;
            let band_idx = band_idx.min(self.spectrum.bands.len().saturating_sub(1));
            let value = self.spectrum.bands.get(band_idx).copied().unwrap_or(0.0);

            for x in 0..area.width {
                // Fade intensity based on distance from highway
                let dist = match self.side {
                    Side::Left => (area.width - 1 - x) as f32 / area.width as f32,
                    Side::Right => x as f32 / area.width as f32,
                };
                let effective = value * (1.0 - dist * 0.7);
                let char_idx = (effective * (BLOCK_CHARS.len() - 1) as f32) as usize;
                let char_idx = char_idx.min(BLOCK_CHARS.len() - 1);

                let brightness = (effective * 120.0 + 20.0).min(140.0) as u8;
                buf.set_string(
                    area.x + x,
                    area.y + y,
                    BLOCK_CHARS[char_idx],
                    Style::default().fg(Color::Rgb(brightness, brightness, brightness)),
                );
            }
        }
    }
}
```

- [ ] **Step 5: Create widgets.rs with shared components**

Create `src/ui/widgets.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::Widget;

/// Centered selectable list for menus
pub struct MenuList<'a> {
    pub items: &'a [&'a str],
    pub selected: usize,
}

impl<'a> Widget for MenuList<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let start_y = area.y + (area.height.saturating_sub(self.items.len() as u16)) / 2;

        for (i, item) in self.items.iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.y + area.height {
                break;
            }

            let (prefix, style) = if i == self.selected {
                ("▸ ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
            };

            let text = format!("{}{}", prefix, item);
            let x = area.x + (area.width.saturating_sub(text.len() as u16)) / 2;
            buf.set_string(x, y, &text, style);
        }
    }
}

/// Simple progress bar
pub struct ProgressBar {
    pub progress: f64, // 0.0 - 1.0
    pub label: String,
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height == 0 {
            return;
        }

        let bar_width = area.width - 2;
        let filled = (self.progress * bar_width as f64) as u16;

        buf.set_string(area.x, area.y, "[", Style::default().fg(Color::Rgb(100, 100, 100)));
        for i in 0..bar_width {
            let ch = if i < filled { "━" } else { "─" };
            let style = if i < filled {
                Style::default().fg(Color::Rgb(180, 180, 180))
            } else {
                Style::default().fg(Color::Rgb(40, 40, 40))
            };
            buf.set_string(area.x + 1 + i, area.y, ch, style);
        }
        buf.set_string(area.x + bar_width + 1, area.y, "]", Style::default().fg(Color::Rgb(100, 100, 100)));

        if area.height >= 2 {
            let x = area.x + (area.width.saturating_sub(self.label.len() as u16)) / 2;
            buf.set_string(x, area.y + 1, &self.label, Style::default().fg(Color::Rgb(120, 120, 120)));
        }
    }
}
```

- [ ] **Step 6: Export from lib.rs, build**

Add to `src/lib.rs`:

```rust
pub mod ui;
```

```bash
cargo build
```

Expected: builds successfully.

- [ ] **Step 7: Commit**

```bash
git add src/ui/ src/lib.rs
git commit -m "feat: UI widgets — highway renderer, visualizer, HUD"
```

---

### Task 7: Main Menu Screen

**Files:**
- Create: `src/screens/mod.rs`
- Create: `src/screens/menu.rs`
- Modify: `src/main.rs` — wire screen rendering

- [ ] **Step 1: Create screens module**

Create `src/screens/mod.rs`:

```rust
pub mod menu;
pub mod song_select;
pub mod gameplay;
pub mod results;
pub mod settings;
```

- [ ] **Step 2: Implement menu.rs**

Create `src/screens/menu.rs`:

```rust
use ratatui::prelude::*;
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
        // Logo
        let logo_height = LOGO.lines().count() as u16;
        let logo_y = area.y + 2;
        for (i, line) in LOGO.lines().enumerate() {
            let y = logo_y + i as u16;
            if y >= area.y + area.height {
                break;
            }
            let x = area.x + area.width.saturating_sub(line.len() as u16) / 2;
            frame.buffer_mut().set_string(
                x,
                y,
                line,
                Style::default().fg(Color::Rgb(160, 160, 160)),
            );
        }

        // Menu items below logo
        let menu_area = Rect {
            x: area.x,
            y: logo_y + logo_height + 2,
            width: area.width,
            height: area.height.saturating_sub(logo_height + 4),
        };

        MenuList {
            items: MENU_ITEMS,
            selected: self.selected,
        }
        .render(menu_area, frame.buffer_mut());

        // Footer
        let footer_y = area.y + area.height - 1;
        let footer = "Terminal Rhythm Game";
        let x = area.x + (area.width.saturating_sub(footer.len() as u16)) / 2;
        frame.buffer_mut().set_string(
            x,
            footer_y,
            footer,
            Style::default().fg(Color::Rgb(60, 60, 60)),
        );
    }
}
```

- [ ] **Step 3: Create stubs for other screens**

Create `src/screens/song_select.rs`:

```rust
// Implemented in Task 8
```

Create `src/screens/gameplay.rs`:

```rust
// Implemented in Task 9
```

Create `src/screens/results.rs`:

```rust
// Implemented in Task 10
```

Create `src/screens/settings.rs`:

```rust
// Implemented in Task 12
```

- [ ] **Step 4: Wire menu into main.rs**

Update `src/main.rs` — replace the placeholder render with actual menu screen. Add `mod screens;` and update the run function to create `MenuScreen`, route actions through it, and call `menu.render(frame, area)` when `app.screen == Screen::Menu`.

The full updated `run()`:

```rust
fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let config = config::Config::load(&config::Config::default_path())?;
    let mut app = App::new();
    let lanes = config.keys.lanes;

    let mut menu = screens::menu::MenuScreen::new();

    while app.running {
        let frame_start = Instant::now();

        // Input
        if event::poll(Duration::from_millis(1))? {
            if let Event::Key(key) = event::read()? {
                let action = map_key(key, &lanes);
                match action {
                    Action::Quit => app.quit(),
                    Action::Navigate(screen) => app.navigate(screen),
                    _ => {
                        if app.screen == Screen::Menu {
                            if let Some(result) = menu.handle_action(action) {
                                match result {
                                    Action::Quit => app.quit(),
                                    Action::Navigate(screen) => app.navigate(screen),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        // Render
        terminal.draw(|frame| {
            let area = frame.area();
            match app.screen {
                Screen::Menu => menu.render(frame, area),
                _ => {
                    frame.render_widget(
                        ratatui::widgets::Paragraph::new("Not implemented yet — ESC to go back")
                            .alignment(Alignment::Center),
                        area,
                    );
                }
            }
        })?;

        // Frame timing
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}
```

- [ ] **Step 5: Build and run, verify menu renders**

```bash
cargo run
```

Expected: ASCII "CASCADE" logo, menu items (Play/Settings/Quit), arrow keys navigate, Enter selects, Q quits.

- [ ] **Step 6: Commit**

```bash
git add src/screens/ src/main.rs src/lib.rs
git commit -m "feat: main menu screen with ASCII logo"
```

---

### Task 8: Song Select Screen

**Files:**
- Modify: `src/screens/song_select.rs`
- Modify: `src/main.rs` — wire song select

- [ ] **Step 1: Implement song_select.rs**

```rust
use ratatui::prelude::*;
use crate::app::{Action, Screen};
use crate::beatmap::types::Difficulty;
use crate::config::Config;
use std::path::PathBuf;

pub struct SongEntry {
    pub title: String,
    pub artist: String,
    pub dir: PathBuf,
    pub difficulties: Vec<Difficulty>,
}

pub struct SongSelectScreen {
    pub songs: Vec<SongEntry>,
    pub selected: usize,
    pub difficulty: Difficulty,
    pub import_mode: bool,
    pub import_input: String,
    pub import_status: Option<String>,
}

impl SongSelectScreen {
    pub fn new(difficulty: Difficulty) -> Self {
        Self {
            songs: Vec::new(),
            selected: 0,
            difficulty,
            import_mode: false,
            import_input: String::new(),
            import_status: None,
        }
    }

    pub fn scan_songs(&mut self, songs_dir: &std::path::Path) {
        self.songs.clear();
        if !songs_dir.exists() {
            return;
        }

        let Ok(entries) = std::fs::read_dir(songs_dir) else { return };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Try to read metadata.json
            let meta_path = path.join("metadata.json");
            let (title, artist) = if meta_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                        (
                            meta["title"].as_str().unwrap_or("Unknown").to_string(),
                            meta["artist"].as_str().unwrap_or("Unknown").to_string(),
                        )
                    } else {
                        (path.file_name().unwrap_or_default().to_string_lossy().to_string(), String::new())
                    }
                } else {
                    (path.file_name().unwrap_or_default().to_string_lossy().to_string(), String::new())
                }
            } else {
                (path.file_name().unwrap_or_default().to_string_lossy().to_string(), String::new())
            };

            // Check which difficulties exist
            let difficulties: Vec<Difficulty> = Difficulty::all()
                .iter()
                .filter(|d| path.join(d.filename()).exists())
                .copied()
                .collect();

            if !difficulties.is_empty() {
                self.songs.push(SongEntry {
                    title,
                    artist,
                    dir: path,
                    difficulties,
                });
            }
        }

        self.songs.sort_by(|a, b| a.title.cmp(&b.title));
    }

    pub fn selected_beatmap_path(&self) -> Option<PathBuf> {
        self.songs.get(self.selected).map(|song| {
            song.dir.join(self.difficulty.filename())
        })
    }

    pub fn selected_audio_path(&self) -> Option<PathBuf> {
        self.songs.get(self.selected).map(|song| {
            song.dir.join("audio.mp3")
        })
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        if self.import_mode {
            // Import mode handles text input separately
            return None;
        }

        match action {
            Action::MenuUp => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            Action::MenuDown => {
                if self.selected < self.songs.len().saturating_sub(1) {
                    self.selected += 1;
                }
                None
            }
            Action::MenuSelect => {
                if !self.songs.is_empty() {
                    Some(Action::Navigate(Screen::Gameplay))
                } else {
                    None
                }
            }
            Action::Back | Action::Pause => {
                Some(Action::Navigate(Screen::Menu))
            }
            _ => None,
        }
    }

    pub fn cycle_difficulty(&mut self) {
        self.difficulty = match self.difficulty {
            Difficulty::Easy => Difficulty::Medium,
            Difficulty::Medium => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Expert,
            Difficulty::Expert => Difficulty::Easy,
        };
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();

        // Title
        let title = "SELECT SONG";
        let x = area.x + (area.width.saturating_sub(title.len() as u16)) / 2;
        buf.set_string(x, area.y + 1, title, Style::default().fg(Color::White).bold());

        // Difficulty selector
        let diff_text = format!("Difficulty: {} (Tab to change)", self.difficulty);
        let x = area.x + (area.width.saturating_sub(diff_text.len() as u16)) / 2;
        buf.set_string(x, area.y + 3, &diff_text, Style::default().fg(Color::Rgb(140, 140, 140)));

        if self.songs.is_empty() {
            let msg = "No songs found. Press 'i' to import from YouTube.";
            let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            buf.set_string(x, area.y + area.height / 2, msg, Style::default().fg(Color::Rgb(100, 100, 100)));
        } else {
            // Song list
            let list_y = area.y + 5;
            let max_visible = (area.height.saturating_sub(8)) as usize;

            let start = if self.selected >= max_visible {
                self.selected - max_visible + 1
            } else {
                0
            };

            for (i, song) in self.songs.iter().skip(start).take(max_visible).enumerate() {
                let idx = start + i;
                let y = list_y + i as u16;
                if y >= area.y + area.height - 2 {
                    break;
                }

                let (prefix, style) = if idx == self.selected {
                    ("▸ ", Style::default().fg(Color::White).bold())
                } else {
                    ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
                };

                let text = if song.artist.is_empty() {
                    format!("{}{}", prefix, song.title)
                } else {
                    format!("{}{} — {}", prefix, song.title, song.artist)
                };

                buf.set_string(area.x + 4, y, &text, style);
            }
        }

        // Import status
        if let Some(status) = &self.import_status {
            let y = area.y + area.height - 2;
            buf.set_string(area.x + 2, y, status, Style::default().fg(Color::Rgb(120, 120, 120)));
        }

        // Footer
        let footer = "Enter: Play  Tab: Difficulty  i: Import  ESC: Back";
        let x = area.x + (area.width.saturating_sub(footer.len() as u16)) / 2;
        buf.set_string(
            x,
            area.y + area.height - 1,
            footer,
            Style::default().fg(Color::Rgb(60, 60, 60)),
        );
    }
}
```

- [ ] **Step 2: Wire into main.rs**

Add `SongSelectScreen` to the run function, scan songs on navigation to SongSelect, handle Tab for difficulty cycling, handle 'i' for import mode, and route actions.

- [ ] **Step 3: Build and run, verify navigation Menu → Song Select → back**

```bash
cargo run
```

Expected: Play → song select shows "No songs found", ESC goes back to menu.

- [ ] **Step 4: Commit**

```bash
git add src/screens/song_select.rs src/main.rs
git commit -m "feat: song select screen with difficulty selector"
```

---

### Task 9: Gameplay Screen

**Files:**
- Modify: `src/screens/gameplay.rs`
- Modify: `src/main.rs` — wire gameplay

This is the core screen that ties together highway, audio, hit detection, and visualizer.

- [ ] **Step 1: Implement gameplay.rs**

```rust
use ratatui::prelude::*;
use ratatui::layout::{Layout, Constraint, Direction};
use crate::app::{Action, Screen};
use crate::audio::player::AudioPlayer;
use crate::audio::analyzer::SpectrumData;
use crate::beatmap::types::{Beatmap, Difficulty};
use crate::game::state::GameState;
use crate::game::hit_judge::{HitJudge, Judgement};
use crate::game::highway::Highway;
use crate::ui::highway_render::HighwayWidget;
use crate::ui::hud::{HudTop, HudBottom};
use crate::ui::visualizer::{WaveVisualizer, BlockVisualizer, Side};
use std::path::Path;
use anyhow::Result;

pub struct GameplayScreen {
    pub beatmap: Beatmap,
    pub audio: AudioPlayer,
    pub state: GameState,
    pub highway: Highway,
    pub judge: HitJudge,
    pub hit_notes: Vec<bool>,
    pub hit_flash: [u8; 5],
    pub paused: bool,
    pub finished: bool,
    pub spectrum: SpectrumData,
    pub judgement_timer: u8, // frames to show last judgement
}

impl GameplayScreen {
    pub fn new(beatmap: Beatmap, audio_path: &Path, offset_ms: i32, scroll_speed: f64, volume: f64) -> Result<Self> {
        let mut audio = AudioPlayer::new()?;
        audio.load(audio_path)?;
        audio.set_volume(volume as f32);

        let hit_notes = vec![false; beatmap.notes.len()];

        Ok(Self {
            highway: Highway::new(scroll_speed),
            judge: HitJudge::new(offset_ms),
            state: GameState::new(),
            hit_notes,
            hit_flash: [0; 5],
            paused: false,
            finished: false,
            spectrum: SpectrumData::empty(32),
            judgement_timer: 0,
            beatmap,
            audio,
        })
    }

    pub fn start(&mut self) {
        self.audio.play();
    }

    pub fn update(&mut self) {
        if self.paused || self.finished {
            return;
        }

        self.audio.update_position();
        let current_ms = self.audio.position_ms();

        // Update highway
        self.highway.update(&self.beatmap.notes, current_ms, 2000, &self.hit_notes);

        // Check for expired notes (auto-miss)
        for (i, note) in self.beatmap.notes.iter().enumerate() {
            if !self.hit_notes[i] && self.judge.is_expired(note.time_ms, current_ms) {
                self.hit_notes[i] = true;
                self.state.register_judgement(Judgement::Miss);
                self.judgement_timer = 30;
            }
        }

        // Decay hit flash
        for flash in &mut self.hit_flash {
            *flash = flash.saturating_sub(1);
        }

        // Decay judgement display
        if self.judgement_timer > 0 {
            self.judgement_timer -= 1;
            if self.judgement_timer == 0 {
                self.state.last_judgement = None;
            }
        }

        // Check if song finished
        if self.audio.is_finished() || (current_ms > self.beatmap.song.duration_ms + 2000) {
            self.finished = true;
        }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::Pause => {
                if self.finished {
                    return Some(Action::Navigate(Screen::Results));
                }
                self.paused = !self.paused;
                if self.paused {
                    self.audio.pause();
                } else {
                    self.audio.resume();
                }
                None
            }
            Action::GameKey(lane) if !self.paused && !self.finished => {
                self.hit_flash[lane] = 8;
                let current_ms = self.audio.position_ms();

                // Find closest unhit note in this lane within hit window
                let mut best: Option<(usize, u64)> = None;
                for (i, note) in self.beatmap.notes.iter().enumerate() {
                    if self.hit_notes[i] || note.lane as usize != lane {
                        continue;
                    }
                    let diff = (note.time_ms as i64 - current_ms as i64).unsigned_abs();
                    if diff <= 100 {
                        if best.is_none() || diff < best.unwrap().1 {
                            best = Some((i, diff));
                        }
                    }
                }

                if let Some((note_idx, _)) = best {
                    let note_time = self.beatmap.notes[note_idx].time_ms;
                    let judgement = self.judge.judge(note_time, current_ms);
                    self.hit_notes[note_idx] = true;
                    self.state.register_judgement(judgement);
                    self.judgement_timer = 30;
                }
                None
            }
            Action::Back if self.paused => {
                self.audio.stop();
                Some(Action::Navigate(Screen::SongSelect))
            }
            _ => None,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();

        // Layout: [left visualizer | highway | right visualizer]
        // Top: HUD + wave visualizer
        // Bottom: HUD bottom

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),   // wave + top HUD
                Constraint::Min(10),     // highway + side visualizers
                Constraint::Length(3),   // bottom HUD
            ])
            .split(area);

        let top_area = vertical[0];
        let mid_area = vertical[1];
        let bot_area = vertical[2];

        // Top: wave visualizer row 0, HUD row 1
        if top_area.height >= 1 {
            WaveVisualizer { spectrum: &self.spectrum }
                .render(Rect { height: 1, ..top_area }, buf);
        }
        if top_area.height >= 2 {
            HudTop { state: &self.state }
                .render(Rect { y: top_area.y + 1, height: 1, ..top_area }, buf);
        }

        // Middle: side visualizers + highway
        let side_width = (mid_area.width / 5).max(3);
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(side_width),
                Constraint::Min(20),
                Constraint::Length(side_width),
            ])
            .split(mid_area);

        BlockVisualizer { spectrum: &self.spectrum, side: Side::Left }
            .render(horizontal[0], buf);

        HighwayWidget::new(&self.highway.visible_notes)
            .with_hit_flash(self.hit_flash)
            .render(horizontal[1], buf);

        BlockVisualizer { spectrum: &self.spectrum, side: Side::Right }
            .render(horizontal[2], buf);

        // Bottom HUD
        let progress = if self.beatmap.song.duration_ms > 0 {
            self.audio.position_ms() as f64 / self.beatmap.song.duration_ms as f64
        } else {
            0.0
        };

        HudBottom {
            state: &self.state,
            song_title: &format!("{} — {}", self.beatmap.song.title, self.beatmap.song.artist),
            progress,
            difficulty: &self.beatmap.difficulty.to_string().to_uppercase(),
        }
        .render(bot_area, buf);

        // Pause overlay
        if self.paused {
            let pause_text = "PAUSED";
            let x = area.x + (area.width.saturating_sub(pause_text.len() as u16)) / 2;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, pause_text, Style::default().fg(Color::White).bold());

            let hint = "ESC: Resume   Q: Quit to menu";
            let x = area.x + (area.width.saturating_sub(hint.len() as u16)) / 2;
            buf.set_string(x, y + 1, hint, Style::default().fg(Color::Rgb(100, 100, 100)));
        }
    }
}
```

- [ ] **Step 2: Wire gameplay into main.rs**

Update `run()` to:
- Create `GameplayScreen` when navigating to Gameplay (load beatmap + audio from selected song)
- Call `gameplay.update()` each frame
- Call `gameplay.handle_action()` for input
- On `gameplay.is_finished()` → navigate to Results
- Handle `Action::Back` during pause → back to SongSelect

- [ ] **Step 3: Build and test with a sample song**

To test, manually create a test song directory:

```bash
mkdir -p ~/.cascade/songs/test-song
```

Create `~/.cascade/songs/test-song/metadata.json`:
```json
{"title": "Test", "artist": "Test"}
```

Place any `.mp3` file as `~/.cascade/songs/test-song/audio.mp3` and create a simple `hard.json` beatmap manually, then run:

```bash
cargo run
```

Expected: Play → select song → gameplay screen with falling notes, keys register hits, ESC pauses.

- [ ] **Step 4: Commit**

```bash
git add src/screens/gameplay.rs src/main.rs
git commit -m "feat: gameplay screen with highway, audio, and hit detection"
```

---

### Task 10: Results Screen

**Files:**
- Modify: `src/screens/results.rs`

- [ ] **Step 1: Implement results.rs**

```rust
use ratatui::prelude::*;
use crate::app::{Action, Screen};
use crate::game::state::GameState;
use crate::game::hit_judge::Judgement;

pub struct ResultsScreen {
    pub state: GameState,
    pub song_title: String,
    pub difficulty: String,
    pub selected: usize, // 0 = retry, 1 = back
}

impl ResultsScreen {
    pub fn new(state: GameState, song_title: String, difficulty: String) -> Self {
        Self {
            state,
            song_title,
            difficulty,
            selected: 0,
        }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::MenuUp | Action::MenuDown => {
                self.selected = 1 - self.selected;
                None
            }
            Action::MenuSelect => {
                if self.selected == 0 {
                    Some(Action::Navigate(Screen::Gameplay)) // retry
                } else {
                    Some(Action::Navigate(Screen::SongSelect))
                }
            }
            Action::Back | Action::Pause => {
                Some(Action::Navigate(Screen::SongSelect))
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();
        let center_x = area.x + area.width / 2;
        let mut y = area.y + 2;

        // Title
        let title = "RESULTS";
        buf.set_string(
            center_x - title.len() as u16 / 2,
            y,
            title,
            Style::default().fg(Color::White).bold(),
        );
        y += 2;

        // Song info
        let song_info = format!("{} [{}]", self.song_title, self.difficulty);
        buf.set_string(
            center_x - song_info.len() as u16 / 2,
            y,
            &song_info,
            Style::default().fg(Color::Rgb(140, 140, 140)),
        );
        y += 3;

        // Grade — large
        let grade = self.state.grade();
        buf.set_string(
            center_x - 1,
            y,
            grade,
            Style::default().fg(Color::White).bold(),
        );
        y += 3;

        // Stats
        let stats = [
            format!("Score:     {:>10}", self.state.score),
            format!("Accuracy:  {:>9.1}%", self.state.accuracy()),
            format!("Max Combo: {:>10}", self.state.max_combo),
            String::new(),
            format!("Perfect:   {:>10}", self.state.judgement_counts[0]),
            format!("Great:     {:>10}", self.state.judgement_counts[1]),
            format!("Good:      {:>10}", self.state.judgement_counts[2]),
            format!("Miss:      {:>10}", self.state.judgement_counts[3]),
        ];

        for line in &stats {
            buf.set_string(
                center_x - line.len() as u16 / 2,
                y,
                line,
                Style::default().fg(Color::Rgb(160, 160, 160)),
            );
            y += 1;
        }

        y += 2;

        // Options
        let options = ["Retry", "Back to songs"];
        for (i, option) in options.iter().enumerate() {
            let (prefix, style) = if i == self.selected {
                ("▸ ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
            };
            let text = format!("{}{}", prefix, option);
            buf.set_string(center_x - text.len() as u16 / 2, y, &text, style);
            y += 1;
        }
    }
}
```

- [ ] **Step 2: Wire results into main.rs**

When gameplay finishes, create `ResultsScreen` from the game state. Handle retry → recreate gameplay, back → song select.

- [ ] **Step 3: Build and verify flow: gameplay → results → retry/back**

```bash
cargo run
```

Expected: after song ends, results screen shows grade/score/accuracy, Retry restarts, Back goes to song select.

- [ ] **Step 4: Commit**

```bash
git add src/screens/results.rs src/main.rs
git commit -m "feat: results screen with grade, score, and accuracy"
```

---

### Task 11: YouTube Import (yt-dlp)

**Files:**
- Modify: `src/audio/import.rs`
- Test: `tests/import_test.rs`

- [ ] **Step 1: Write failing test for URL validation and slug generation**

Create `tests/import_test.rs`:

```rust
use cascade::audio::import::{is_youtube_url, slug_from_title};

#[test]
fn test_youtube_url_detection() {
    assert!(is_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
    assert!(is_youtube_url("https://youtu.be/dQw4w9WgXcQ"));
    assert!(is_youtube_url("https://youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf"));
    assert!(!is_youtube_url("https://example.com/video"));
    assert!(!is_youtube_url("not a url"));
}

#[test]
fn test_slug_generation() {
    assert_eq!(slug_from_title("Neon Dreams - The Midnight"), "neon-dreams-the-midnight");
    assert_eq!(slug_from_title("Song (Official Video)"), "song-official-video");
    assert_eq!(slug_from_title("  Lots   of   spaces  "), "lots-of-spaces");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test import_test
```

- [ ] **Step 3: Implement import.rs**

```rust
use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/") || url.contains("youtu.be/") || url.contains("youtube.com/playlist")
}

pub fn slug_from_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Download audio from a YouTube URL using yt-dlp.
/// Returns the path to the downloaded audio and the title.
pub async fn download_audio(url: &str, songs_dir: &Path) -> Result<Vec<ImportedSong>> {
    // First get metadata (title, playlist entries)
    let output = tokio::process::Command::new("yt-dlp")
        .args(["--flat-playlist", "--dump-json", url])
        .output()
        .await
        .context("Failed to run yt-dlp. Is it installed?")?;

    if !output.status.success() {
        anyhow::bail!("yt-dlp metadata fetch failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    let mut results = Vec::new();

    for entry in &entries {
        let title = entry["title"].as_str().unwrap_or("unknown");
        let entry_url = entry["url"].as_str()
            .or_else(|| entry["webpage_url"].as_str())
            .unwrap_or(url);

        let slug = slug_from_title(title);
        let song_dir = songs_dir.join(&slug);
        std::fs::create_dir_all(&song_dir)?;

        let audio_path = song_dir.join("audio.mp3");

        // Download audio
        let dl_status = tokio::process::Command::new("yt-dlp")
            .args([
                "--extract-audio",
                "--audio-format", "mp3",
                "--audio-quality", "0",
                "-o", audio_path.to_str().unwrap(),
                entry_url,
            ])
            .status()
            .await
            .context("Failed to run yt-dlp download")?;

        if !dl_status.success() {
            eprintln!("Warning: failed to download {}", title);
            continue;
        }

        // Write metadata
        let meta = serde_json::json!({
            "title": title,
            "artist": entry.get("uploader").and_then(|v| v.as_str()).unwrap_or(""),
            "source_url": entry_url,
        });
        std::fs::write(
            song_dir.join("metadata.json"),
            serde_json::to_string_pretty(&meta)?,
        )?;

        results.push(ImportedSong {
            title: title.to_string(),
            dir: song_dir,
            audio_path,
        });
    }

    Ok(results)
}

pub struct ImportedSong {
    pub title: String,
    pub dir: PathBuf,
    pub audio_path: PathBuf,
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test --test import_test
```

Expected: 2 tests PASS (URL validation and slug generation — no actual yt-dlp calls in tests).

- [ ] **Step 5: Commit**

```bash
git add src/audio/import.rs tests/import_test.rs
git commit -m "feat: YouTube import via yt-dlp"
```

---

### Task 12: Beatmap Auto-Generation

**Files:**
- Modify: `src/beatmap/generator.rs`
- Test: `tests/generator_test.rs`

- [ ] **Step 1: Write failing tests for onset detection and note placement**

Create `tests/generator_test.rs`:

```rust
use cascade::beatmap::generator::{detect_onsets, place_notes, OnsetInfo};
use cascade::beatmap::types::{Difficulty, Note};

#[test]
fn test_detect_onsets_from_synthetic_signal() {
    // Create a simple signal with clear beats: silence + spike pattern
    let sample_rate = 44100;
    let duration_secs = 2.0;
    let num_samples = (sample_rate as f64 * duration_secs) as usize;
    let mut samples = vec![0.0f32; num_samples];

    // Place spikes every 0.5 seconds (120 BPM)
    let beat_interval = sample_rate / 2; // 0.5 seconds
    for i in 0..4 {
        let pos = i * beat_interval;
        for j in 0..100 {
            if pos + j < num_samples {
                samples[pos + j] = 0.8;
            }
        }
    }

    let onsets = detect_onsets(&samples, sample_rate);
    // Should detect approximately 4 onsets
    assert!(onsets.len() >= 3, "Expected at least 3 onsets, got {}", onsets.len());
    assert!(onsets.len() <= 6, "Expected at most 6 onsets, got {}", onsets.len());
}

#[test]
fn test_place_notes_easy_has_fewer_notes() {
    let onsets = vec![
        OnsetInfo { time_ms: 500, strength: 0.9, freq_band: 0 },
        OnsetInfo { time_ms: 750, strength: 0.3, freq_band: 1 },
        OnsetInfo { time_ms: 1000, strength: 0.8, freq_band: 2 },
        OnsetInfo { time_ms: 1100, strength: 0.2, freq_band: 1 },
        OnsetInfo { time_ms: 1500, strength: 0.7, freq_band: 0 },
        OnsetInfo { time_ms: 1600, strength: 0.4, freq_band: 2 },
        OnsetInfo { time_ms: 2000, strength: 0.95, freq_band: 1 },
    ];

    let easy = place_notes(&onsets, Difficulty::Easy);
    let hard = place_notes(&onsets, Difficulty::Hard);

    assert!(easy.len() < hard.len(), "Easy ({}) should have fewer notes than Hard ({})", easy.len(), hard.len());
}

#[test]
fn test_place_notes_lane_assignment_by_frequency() {
    let onsets = vec![
        OnsetInfo { time_ms: 1000, strength: 0.9, freq_band: 0 }, // low → lane 0 or 1
        OnsetInfo { time_ms: 2000, strength: 0.9, freq_band: 1 }, // mid → lane 2
        OnsetInfo { time_ms: 3000, strength: 0.9, freq_band: 2 }, // high → lane 3 or 4
    ];

    let notes = place_notes(&onsets, Difficulty::Hard);
    assert!(notes.len() >= 3);
    assert!(notes[0].lane <= 1, "Low freq should map to lane 0 or 1");
    assert_eq!(notes[1].lane, 2, "Mid freq should map to lane 2");
    assert!(notes[2].lane >= 3, "High freq should map to lane 3 or 4");
}

#[test]
fn test_no_extreme_lane_jumps() {
    let onsets: Vec<OnsetInfo> = (0..20)
        .map(|i| OnsetInfo {
            time_ms: i * 100 + 500, // 100ms apart — fast
            strength: 0.9,
            freq_band: (i % 3) as u8,
        })
        .collect();

    let notes = place_notes(&onsets, Difficulty::Expert);

    for window in notes.windows(2) {
        let gap = (window[0].time_ms as i64 - window[1].time_ms as i64).unsigned_abs();
        if gap <= 100 {
            let lane_jump = (window[0].lane as i8 - window[1].lane as i8).unsigned_abs();
            assert!(lane_jump <= 3, "Lane jump of {} between notes {}ms apart", lane_jump, gap);
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test generator_test
```

- [ ] **Step 3: Implement generator.rs**

```rust
use crate::beatmap::types::{Beatmap, Note, Difficulty, SongMeta};
use rustfft::{FftPlanner, num_complex::Complex};
use anyhow::Result;

pub struct OnsetInfo {
    pub time_ms: u64,
    pub strength: f32,
    pub freq_band: u8, // 0=low, 1=mid, 2=high
}

const FFT_SIZE: usize = 2048;
const HOP_SIZE: usize = 512;

/// Detect onsets in audio samples using spectral flux.
pub fn detect_onsets(samples: &[f32], sample_rate: u32) -> Vec<OnsetInfo> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let mut prev_magnitudes = vec![0.0f32; FFT_SIZE / 2];
    let mut onsets = Vec::new();

    let mut pos = 0;
    while pos + FFT_SIZE <= samples.len() {
        // Window and FFT
        let mut buffer: Vec<Complex<f32>> = samples[pos..pos + FFT_SIZE]
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                // Hann window
                let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos());
                Complex::new(s * window, 0.0)
            })
            .collect();

        fft.process(&mut buffer);

        // Compute magnitudes
        let magnitudes: Vec<f32> = buffer[..FFT_SIZE / 2]
            .iter()
            .map(|c| c.norm())
            .collect();

        // Spectral flux (positive differences only)
        let mut flux_low = 0.0f32;
        let mut flux_mid = 0.0f32;
        let mut flux_high = 0.0f32;

        let bin_freq = sample_rate as f32 / FFT_SIZE as f32;
        for (i, (curr, prev)) in magnitudes.iter().zip(prev_magnitudes.iter()).enumerate() {
            let diff = (curr - prev).max(0.0);
            let freq = i as f32 * bin_freq;

            if freq < 300.0 {
                flux_low += diff;
            } else if freq < 2000.0 {
                flux_mid += diff;
            } else {
                flux_high += diff;
            }
        }

        let total_flux = flux_low + flux_mid + flux_high;

        // Determine dominant frequency band
        let freq_band = if flux_low >= flux_mid && flux_low >= flux_high {
            0
        } else if flux_mid >= flux_high {
            1
        } else {
            2
        };

        let time_ms = (pos as f64 / sample_rate as f64 * 1000.0) as u64;

        if total_flux > 0.0 {
            onsets.push(OnsetInfo {
                time_ms,
                strength: total_flux,
                freq_band,
            });
        }

        prev_magnitudes = magnitudes;
        pos += HOP_SIZE;
    }

    // Normalize strengths
    let max_strength = onsets.iter().map(|o| o.strength).fold(0.0f32, f32::max);
    if max_strength > 0.0 {
        for onset in &mut onsets {
            onset.strength /= max_strength;
        }
    }

    // Peak picking — only keep local maxima
    let mut peaks = Vec::new();
    for i in 1..onsets.len().saturating_sub(1) {
        if onsets[i].strength > onsets[i - 1].strength
            && onsets[i].strength > onsets[i + 1].strength
            && onsets[i].strength > 0.1
        {
            peaks.push(OnsetInfo {
                time_ms: onsets[i].time_ms,
                strength: onsets[i].strength,
                freq_band: onsets[i].freq_band,
            });
        }
    }

    peaks
}

/// Place notes from onsets based on difficulty.
pub fn place_notes(onsets: &[OnsetInfo], difficulty: Difficulty) -> Vec<Note> {
    let (threshold, min_gap_ms, max_simultaneous) = match difficulty {
        Difficulty::Easy => (0.6, 400u64, 1usize),
        Difficulty::Medium => (0.4, 250, 2),
        Difficulty::Hard => (0.25, 150, 3),
        Difficulty::Expert => (0.15, 80, 4),
    };

    let mut notes = Vec::new();
    let mut last_time: Option<u64> = None;
    let mut last_lane: Option<u8> = None;

    for onset in onsets {
        if onset.strength < threshold {
            continue;
        }

        if let Some(lt) = last_time {
            if onset.time_ms.saturating_sub(lt) < min_gap_ms {
                continue;
            }
        }

        let lane = freq_band_to_lane(onset.freq_band, last_lane);

        notes.push(Note {
            time_ms: onset.time_ms,
            lane,
        });

        last_time = Some(onset.time_ms);
        last_lane = Some(lane);
    }

    notes
}

fn freq_band_to_lane(freq_band: u8, last_lane: Option<u8>) -> u8 {
    let base_lanes: &[u8] = match freq_band {
        0 => &[0, 1],     // low
        1 => &[2],         // mid
        _ => &[3, 4],     // high
    };

    // Pick lane avoiding extreme jumps
    let mut best = base_lanes[0];
    if let Some(prev) = last_lane {
        let mut min_jump = u8::MAX;
        for &lane in base_lanes {
            let jump = (lane as i8 - prev as i8).unsigned_abs();
            if jump < min_jump {
                min_jump = jump;
                best = lane;
            }
        }
    } else if base_lanes.len() > 1 {
        // Alternate between options for variety
        best = base_lanes[0];
    }

    best
}

/// Detect BPM from onset times using autocorrelation.
pub fn detect_bpm(onsets: &[OnsetInfo]) -> u32 {
    if onsets.len() < 4 {
        return 120; // default
    }

    // Compute intervals between successive onsets
    let intervals: Vec<u64> = onsets.windows(2)
        .map(|w| w[1].time_ms - w[0].time_ms)
        .filter(|&i| i > 200 && i < 2000) // 30-300 BPM range
        .collect();

    if intervals.is_empty() {
        return 120;
    }

    // Find most common interval (simple histogram)
    let mut best_interval = intervals[0];
    let mut best_count = 0;

    for &interval in &intervals {
        let count = intervals.iter()
            .filter(|&&i| (i as i64 - interval as i64).unsigned_abs() < 30)
            .count();
        if count > best_count {
            best_count = count;
            best_interval = interval;
        }
    }

    let bpm = (60000.0 / best_interval as f64).round() as u32;
    bpm.clamp(60, 300)
}

/// Full pipeline: decode audio → detect onsets → generate all difficulty beatmaps.
pub fn generate_all_beatmaps(
    samples: &[f32],
    sample_rate: u32,
    song_meta: SongMeta,
) -> Vec<Beatmap> {
    let onsets = detect_onsets(samples, sample_rate);
    let bpm = detect_bpm(&onsets);

    let mut meta = song_meta;
    meta.bpm = bpm;

    Difficulty::all()
        .iter()
        .map(|&diff| {
            let notes = place_notes(&onsets, diff);
            Beatmap {
                version: 1,
                song: meta.clone(),
                difficulty: diff,
                notes,
            }
        })
        .collect()
}
```

Also add `Clone` derive to `SongMeta` in `src/beatmap/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongMeta {
```

- [ ] **Step 4: Run tests**

```bash
cargo test --test generator_test
```

Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/beatmap/generator.rs tests/generator_test.rs src/beatmap/types.rs
git commit -m "feat: beatmap auto-generation with onset detection and difficulty scaling"
```

---

### Task 13: FFT Analyzer for Real-Time Visualizer

**Files:**
- Modify: `src/audio/analyzer.rs`

- [ ] **Step 1: Implement real-time spectrum analyzer**

Replace `src/audio/analyzer.rs`:

```rust
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::{Arc, Mutex};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::path::Path;
use anyhow::Result;

pub struct SpectrumData {
    pub bands: Vec<f32>,
    pub energy: f32,
}

impl SpectrumData {
    pub fn empty(num_bands: usize) -> Self {
        Self {
            bands: vec![0.0; num_bands],
            energy: 0.0,
        }
    }
}

const ANALYZER_FFT_SIZE: usize = 1024;
const NUM_BANDS: usize = 32;

pub struct SpectrumAnalyzer {
    planner: FftPlanner<f32>,
    buffer: Vec<Complex<f32>>,
    band_ranges: Vec<(usize, usize)>,
    pub spectrum: Arc<Mutex<SpectrumData>>,
    decay: f32,
}

impl SpectrumAnalyzer {
    pub fn new() -> Self {
        let planner = FftPlanner::new();
        let buffer = vec![Complex::new(0.0, 0.0); ANALYZER_FFT_SIZE];

        // Divide frequency range into bands (logarithmic spacing)
        let mut band_ranges = Vec::with_capacity(NUM_BANDS);
        let min_freq = 20.0f32;
        let max_freq = 16000.0;
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        for i in 0..NUM_BANDS {
            let low = ((log_min + (log_max - log_min) * i as f32 / NUM_BANDS as f32).exp()
                / 44100.0 * ANALYZER_FFT_SIZE as f32) as usize;
            let high = ((log_min + (log_max - log_min) * (i + 1) as f32 / NUM_BANDS as f32).exp()
                / 44100.0 * ANALYZER_FFT_SIZE as f32) as usize;
            band_ranges.push((low.max(1), high.max(low + 1).min(ANALYZER_FFT_SIZE / 2)));
        }

        Self {
            planner,
            buffer,
            band_ranges,
            spectrum: Arc::new(Mutex::new(SpectrumData::empty(NUM_BANDS))),
            decay: 0.85,
        }
    }

    pub fn shared_spectrum(&self) -> Arc<Mutex<SpectrumData>> {
        Arc::clone(&self.spectrum)
    }

    /// Process a chunk of audio samples and update spectrum.
    pub fn process(&mut self, samples: &[f32]) {
        let fft = self.planner.plan_fft_forward(ANALYZER_FFT_SIZE);

        // Fill buffer with latest samples (zero-pad if needed)
        let start = samples.len().saturating_sub(ANALYZER_FFT_SIZE);
        for (i, val) in self.buffer.iter_mut().enumerate() {
            let sample_idx = start + i;
            let s = if sample_idx < samples.len() {
                samples[sample_idx]
            } else {
                0.0
            };
            // Hann window
            let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / ANALYZER_FFT_SIZE as f32).cos());
            *val = Complex::new(s * window, 0.0);
        }

        fft.process(&mut self.buffer);

        // Compute band magnitudes
        let magnitudes: Vec<f32> = self.buffer[..ANALYZER_FFT_SIZE / 2]
            .iter()
            .map(|c| c.norm())
            .collect();

        let max_mag = magnitudes.iter().copied().fold(0.0f32, f32::max).max(0.001);

        let mut spectrum = self.spectrum.lock().unwrap();
        let mut total_energy = 0.0f32;

        for (i, &(low, high)) in self.band_ranges.iter().enumerate() {
            let sum: f32 = magnitudes[low..high.min(magnitudes.len())]
                .iter()
                .sum();
            let avg = sum / (high - low).max(1) as f32;
            let normalized = (avg / max_mag).clamp(0.0, 1.0);

            // Smooth with decay
            let prev = spectrum.bands.get(i).copied().unwrap_or(0.0);
            spectrum.bands[i] = if normalized > prev {
                normalized
            } else {
                prev * self.decay + normalized * (1.0 - self.decay)
            };
            total_energy += spectrum.bands[i];
        }

        spectrum.energy = (total_energy / NUM_BANDS as f32).clamp(0.0, 1.0);
    }
}

/// Decode an audio file to PCM f32 samples.
pub fn decode_audio(path: &Path) -> Result<(Vec<f32>, u32)> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;

    let mut format = probed.format;
    let track = format.default_track().unwrap();
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())?;

    let mut all_samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let samples = sample_buf.samples();
        let channels = spec.channels.count();

        // Mix to mono
        if channels == 1 {
            all_samples.extend_from_slice(samples);
        } else {
            for chunk in samples.chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                all_samples.push(mono);
            }
        }
    }

    Ok((all_samples, sample_rate))
}
```

- [ ] **Step 2: Build to verify**

```bash
cargo build
```

Expected: builds successfully.

- [ ] **Step 3: Commit**

```bash
git add src/audio/analyzer.rs
git commit -m "feat: real-time FFT spectrum analyzer and audio decoder"
```

---

### Task 14: Settings Screen

**Files:**
- Modify: `src/screens/settings.rs`

- [ ] **Step 1: Implement settings.rs**

```rust
use ratatui::prelude::*;
use crate::app::Action;
use crate::config::Config;

const SETTINGS_ITEMS: &[&str] = &[
    "Scroll Speed",
    "Volume",
    "Audio Offset (ms)",
    "Back",
];

pub struct SettingsScreen {
    pub config: Config,
    pub selected: usize,
    pub modified: bool,
}

impl SettingsScreen {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            selected: 0,
            modified: false,
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
                if self.selected < SETTINGS_ITEMS.len() - 1 {
                    self.selected += 1;
                }
                None
            }
            Action::MenuSelect | Action::Back | Action::Pause => {
                if self.selected == SETTINGS_ITEMS.len() - 1 || matches!(action, Action::Back | Action::Pause) {
                    if self.modified {
                        let _ = self.config.save(&Config::default_path());
                    }
                    Some(Action::Navigate(crate::app::Screen::Menu))
                } else {
                    None
                }
            }
            Action::GameKey(lane) => {
                // Use lane 3 (J) as right/increase, lane 1 (F) as left/decrease
                match self.selected {
                    0 => {
                        // Scroll speed: 0.5 - 2.0
                        if lane == 3 || lane == 4 {
                            self.config.gameplay.scroll_speed = (self.config.gameplay.scroll_speed + 0.1).min(2.0);
                        } else if lane == 0 || lane == 1 {
                            self.config.gameplay.scroll_speed = (self.config.gameplay.scroll_speed - 0.1).max(0.5);
                        }
                        self.modified = true;
                    }
                    1 => {
                        // Volume: 0.0 - 1.0
                        if lane == 3 || lane == 4 {
                            self.config.audio.volume = (self.config.audio.volume + 0.05).min(1.0);
                        } else if lane == 0 || lane == 1 {
                            self.config.audio.volume = (self.config.audio.volume - 0.05).max(0.0);
                        }
                        self.modified = true;
                    }
                    2 => {
                        // Offset: -200 to 200
                        if lane == 3 || lane == 4 {
                            self.config.audio.offset_ms = (self.config.audio.offset_ms + 5).min(200);
                        } else if lane == 0 || lane == 1 {
                            self.config.audio.offset_ms = (self.config.audio.offset_ms - 5).max(-200);
                        }
                        self.modified = true;
                    }
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();
        let center_x = area.x + area.width / 2;
        let mut y = area.y + 3;

        let title = "SETTINGS";
        buf.set_string(
            center_x - title.len() as u16 / 2,
            y,
            title,
            Style::default().fg(Color::White).bold(),
        );
        y += 3;

        let values = [
            format!("{:.1}", self.config.gameplay.scroll_speed),
            format!("{:.0}%", self.config.audio.volume * 100.0),
            format!("{:+}ms", self.config.audio.offset_ms),
            String::new(),
        ];

        for (i, (item, value)) in SETTINGS_ITEMS.iter().zip(values.iter()).enumerate() {
            let (prefix, style) = if i == self.selected {
                ("▸ ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
            };

            let text = if value.is_empty() {
                format!("{}{}", prefix, item)
            } else {
                format!("{}{}:  ◂ {} ▸", prefix, item, value)
            };

            let x = center_x.saturating_sub(text.len() as u16 / 2);
            buf.set_string(x, y, &text, style);
            y += 2;
        }

        y += 2;
        let hint = "D/F: Decrease    J/K: Increase    ESC: Back";
        let x = center_x.saturating_sub(hint.len() as u16 / 2);
        buf.set_string(x, y, hint, Style::default().fg(Color::Rgb(60, 60, 60)));
    }
}
```

- [ ] **Step 2: Wire into main.rs, build and test**

Add SettingsScreen creation and handling in run(). Test: Menu → Settings → adjust values → Back → verify config saved.

```bash
cargo run
```

- [ ] **Step 3: Commit**

```bash
git add src/screens/settings.rs src/main.rs
git commit -m "feat: settings screen with scroll speed, volume, and offset"
```

---

### Task 15: Integration — Wire Import + Generation into Song Select

**Files:**
- Modify: `src/screens/song_select.rs` — add import flow
- Modify: `src/main.rs` — handle async import

- [ ] **Step 1: Add import mode to song_select.rs**

Update `SongSelectScreen` to handle 'i' key → enter import mode, text input for URL, trigger async download + beatmap generation. Show progress in `import_status`.

Add to `handle_action`:

```rust
// In the match on action, add handling for 'i' key via GameKey or a new Import action
```

Add method:

```rust
pub async fn import_from_url(&mut self, url: &str, songs_dir: &std::path::Path) -> Result<()> {
    use crate::audio::import::download_audio;
    use crate::audio::analyzer::decode_audio;
    use crate::beatmap::generator::generate_all_beatmaps;
    use crate::beatmap::loader;
    use crate::beatmap::types::SongMeta;

    self.import_status = Some("Downloading...".to_string());
    let imported = download_audio(url, songs_dir).await?;

    for (i, song) in imported.iter().enumerate() {
        self.import_status = Some(format!("Generating beatmaps {}/{}...", i + 1, imported.len()));

        let (samples, sample_rate) = decode_audio(&song.audio_path)?;
        let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u64;

        let meta = SongMeta {
            title: song.title.clone(),
            artist: String::new(),
            audio_file: "audio.mp3".to_string(),
            bpm: 120, // will be detected
            duration_ms,
        };

        let beatmaps = generate_all_beatmaps(&samples, sample_rate, meta);

        for beatmap in &beatmaps {
            let path = song.dir.join(beatmap.difficulty.filename());
            loader::save(beatmap, &path)?;
        }
    }

    self.import_status = Some(format!("Imported {} song(s)!", imported.len()));
    self.scan_songs(songs_dir);
    Ok(())
}
```

- [ ] **Step 2: Handle text input for import URL in main.rs**

When in import mode, collect `KeyCode::Char` into `import_input`, Enter triggers import, ESC cancels. Use `tokio::runtime` or `block_on` for the async import since the game loop is sync.

- [ ] **Step 3: Test full flow: import URL → download → generate → play**

```bash
cargo run
```

Test: Song Select → press 'i' → paste YouTube URL → Enter → wait for download → song appears in list → play.

- [ ] **Step 4: Commit**

```bash
git add src/screens/song_select.rs src/main.rs
git commit -m "feat: YouTube import with auto beatmap generation"
```

---

### Task 16: Integration — Wire Spectrum Analyzer into Gameplay

**Files:**
- Modify: `src/screens/gameplay.rs` — feed real-time FFT to visualizer

- [ ] **Step 1: Add spectrum analyzer to gameplay**

In `GameplayScreen::new()`, decode the audio to samples, create `SpectrumAnalyzer`, spawn a thread that reads through samples in sync with audio position and calls `analyzer.process()`.

```rust
// In GameplayScreen, add:
analyzer: SpectrumAnalyzer,
samples: Arc<Vec<f32>>,
sample_rate: u32,
```

In `update()`, compute which audio samples correspond to current position, slice them, and call `analyzer.process(&slice)`. Then read `analyzer.spectrum` into `self.spectrum` for the visualizer widgets.

- [ ] **Step 2: Build and test — verify visualizer pulses with music**

```bash
cargo run
```

Expected: side blocks and top waves pulse in sync with the audio.

- [ ] **Step 3: Commit**

```bash
git add src/screens/gameplay.rs
git commit -m "feat: real-time FFT visualizer synced to audio playback"
```

---

### Task 17: Polish — Final Integration and Input Handling

**Files:**
- Modify: `src/main.rs` — complete all screen wiring
- Modify: `src/input.rs` — add Tab, 'i', Back handling

- [ ] **Step 1: Update input.rs with full key mapping**

Add `Tab` → new `Action::Tab` variant, `'i'` → `Action::Import`, `'r'` → `Action::Retry`. Update `app.rs` with new Action variants.

- [ ] **Step 2: Complete main.rs screen routing**

Ensure all transitions work:
- Menu → SongSelect, Settings
- SongSelect → Gameplay, Menu
- Gameplay → Results (auto), SongSelect (quit from pause)
- Results → Gameplay (retry), SongSelect (back)
- Settings → Menu

- [ ] **Step 3: Add tokio runtime to main**

Change `fn main()` to `#[tokio::main] async fn main()` for the import async flow. Use `tokio::task::spawn_blocking` for the game loop since it's synchronous.

- [ ] **Step 4: Full end-to-end test**

```bash
cargo run
```

Verify: Menu → Settings → adjust → back → Play → Song Select → import → play song → gameplay → results → retry → results → back → quit.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/input.rs src/app.rs
git commit -m "feat: complete screen routing and input handling"
```
