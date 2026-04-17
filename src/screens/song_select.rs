use crate::app::{Action, Screen};
use crate::beatmap::types::{Beatmap, Difficulty};
use crate::score_store::ScoreStore;
use crate::ui::chrome::{
    difficulty_color, render_bottom_bar, render_difficulty_dots, render_top_bar,
};
use crossterm::event::KeyCode;
use ratatui::prelude::*;
use std::path::{Path, PathBuf};

pub struct SongEntry {
    pub title: String,
    pub artist: String,
    pub dir: PathBuf,
    pub slug: String,
    pub bpm: u32,
    pub duration_ms: u64,
    /// Filesystem mtime in seconds since epoch — used for "Recently added" sort.
    pub added_secs: u64,
    /// Note counts per Difficulty (Easy, Medium, Hard, Expert).
    pub note_counts: [u32; 4],
    pub present: [bool; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Title,
    Artist,
    Added,
    Bpm,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Title => "Title",
            SortMode::Artist => "Artist",
            SortMode::Added => "Recently added",
            SortMode::Bpm => "BPM",
        }
    }

    pub fn cycle(self) -> SortMode {
        match self {
            SortMode::Title => SortMode::Artist,
            SortMode::Artist => SortMode::Added,
            SortMode::Added => SortMode::Bpm,
            SortMode::Bpm => SortMode::Title,
        }
    }
}

pub struct SongSelectScreen {
    pub songs: Vec<SongEntry>,
    pub selected: usize,
    pub difficulty: Difficulty,
    pub import_mode: bool,
    pub import_input: String,
    pub import_status: Option<String>,
    pub search_mode: bool,
    pub search_query: String,
    /// Indices of songs matching the query (or all songs if no query).
    pub filtered_indices: Vec<usize>,
    pub scores: ScoreStore,
    pub sort_mode: SortMode,
    pub rename_mode: bool,
    pub rename_buf: String,
    pub rename_field: RenameField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameField {
    Title,
    Artist,
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
            search_mode: false,
            search_query: String::new(),
            filtered_indices: Vec::new(),
            scores: ScoreStore::default(),
            sort_mode: SortMode::Title,
            rename_mode: false,
            rename_buf: String::new(),
            rename_field: RenameField::Title,
        }
    }

    pub fn load_scores(&mut self, path: &Path) {
        self.scores = ScoreStore::load(path);
    }

    pub fn scan_songs(&mut self, songs_dir: &Path) {
        self.songs.clear();
        if !songs_dir.exists() {
            self.rebuild_filter();
            return;
        }

        let Ok(entries) = std::fs::read_dir(songs_dir) else {
            self.rebuild_filter();
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Presence of each difficulty file.
            let diffs = Difficulty::all();
            let mut present = [false; 4];
            let mut note_counts = [0u32; 4];
            let mut bpm = 0u32;
            let mut duration_ms = 0u64;

            for (i, d) in diffs.iter().enumerate() {
                let p = path.join(d.filename());
                if !p.exists() {
                    continue;
                }
                present[i] = true;
                if let Ok(s) = std::fs::read_to_string(&p)
                    && let Ok(bm) = serde_json::from_str::<Beatmap>(&s)
                {
                    note_counts[i] = bm.notes.len() as u32;
                    if bpm == 0 {
                        bpm = bm.song.bpm;
                    }
                    if duration_ms == 0 {
                        duration_ms = bm.song.duration_ms;
                    }
                }
            }
            if !present.iter().any(|&p| p) {
                continue;
            }
            if find_audio_file(&path).is_none() {
                continue;
            }

            // Metadata
            let meta_path = path.join("metadata.json");
            let (title, artist) = if let Ok(c) = std::fs::read_to_string(&meta_path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&c) {
                    (
                        v["title"].as_str().unwrap_or("Unknown").to_string(),
                        v["artist"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        String::new(),
                    )
                }
            } else {
                (
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    String::new(),
                )
            };

            let slug = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let added_secs = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            self.songs.push(SongEntry {
                title,
                artist,
                dir: path,
                slug,
                bpm,
                duration_ms,
                added_secs,
                note_counts,
                present,
            });
        }

        self.apply_sort();
        self.rebuild_filter();
    }

    fn apply_sort(&mut self) {
        match self.sort_mode {
            SortMode::Title => self.songs.sort_by_key(|a| a.title.to_lowercase()),
            SortMode::Artist => self.songs.sort_by(|a, b| {
                let ka = (a.artist.to_lowercase(), a.title.to_lowercase());
                let kb = (b.artist.to_lowercase(), b.title.to_lowercase());
                ka.cmp(&kb)
            }),
            SortMode::Added => self.songs.sort_by_key(|a| std::cmp::Reverse(a.added_secs)),
            SortMode::Bpm => self.songs.sort_by_key(|a| a.bpm),
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort_mode = self.sort_mode.cycle();
        self.apply_sort();
        self.rebuild_filter();
    }

    fn rebuild_filter(&mut self) {
        self.filtered_indices.clear();
        let q = self.search_query.to_lowercase();
        for (i, s) in self.songs.iter().enumerate() {
            if q.is_empty()
                || s.title.to_lowercase().contains(&q)
                || s.artist.to_lowercase().contains(&q)
            {
                self.filtered_indices.push(i);
            }
        }
        if self.selected >= self.filtered_indices.len() {
            self.selected = 0;
        }
    }

    fn real_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.selected).copied()
    }

    pub fn selected_beatmap_path(&self) -> Option<PathBuf> {
        self.real_index()
            .and_then(|i| self.songs.get(i))
            .map(|s| s.dir.join(self.difficulty.filename()))
    }

    pub fn selected_audio_path(&self) -> Option<PathBuf> {
        self.real_index()
            .and_then(|i| self.songs.get(i))
            .and_then(|s| find_audio_file(&s.dir))
    }

    pub fn selected_song_title(&self) -> String {
        self.real_index()
            .and_then(|i| self.songs.get(i))
            .map(|s| {
                if s.artist.is_empty() {
                    s.title.clone()
                } else {
                    format!("{} — {}", s.title, s.artist)
                }
            })
            .unwrap_or_default()
    }

    /// Handle raw key for search-mode typing. Returns true if the key was consumed.
    pub fn handle_search_key(&mut self, code: KeyCode) -> bool {
        if !self.search_mode {
            return false;
        }
        match code {
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.rebuild_filter();
                true
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.rebuild_filter();
                true
            }
            KeyCode::Esc => {
                self.search_mode = false;
                self.search_query.clear();
                self.rebuild_filter();
                true
            }
            KeyCode::Enter => {
                self.search_mode = false;
                true
            }
            _ => false,
        }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        if self.import_mode || self.search_mode || self.rename_mode {
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
                if self.selected + 1 < self.filtered_indices.len() {
                    self.selected += 1;
                }
                None
            }
            Action::MenuSelect => {
                if !self.filtered_indices.is_empty() {
                    Some(Action::Navigate(Screen::Gameplay))
                } else {
                    None
                }
            }
            Action::Tab => {
                self.cycle_difficulty();
                None
            }
            Action::Sort => {
                self.cycle_sort();
                None
            }
            Action::Rename => {
                self.start_rename();
                None
            }
            Action::Delete => {
                if let Some(idx) = self.real_index() {
                    let song = &self.songs[idx];
                    let _ = std::fs::remove_dir_all(&song.dir);
                    self.songs.remove(idx);
                    self.rebuild_filter();
                    self.import_status = Some("Song deleted".to_string());
                }
                None
            }
            Action::Back | Action::Pause => Some(Action::Navigate(Screen::Menu)),
            _ => None,
        }
    }

    fn start_rename(&mut self) {
        let Some(idx) = self.real_index() else { return };
        let song = &self.songs[idx];
        self.rename_field = RenameField::Title;
        self.rename_buf = song.title.clone();
        self.rename_mode = true;
    }

    pub fn handle_rename_key(&mut self, code: KeyCode) -> bool {
        if !self.rename_mode {
            return false;
        }
        match code {
            KeyCode::Char(c) => {
                self.rename_buf.push(c);
                true
            }
            KeyCode::Backspace => {
                self.rename_buf.pop();
                true
            }
            KeyCode::Tab => {
                self.commit_rename_field();
                self.rename_field = match self.rename_field {
                    RenameField::Title => RenameField::Artist,
                    RenameField::Artist => RenameField::Title,
                };
                self.rename_buf = self
                    .real_index()
                    .and_then(|i| self.songs.get(i))
                    .map(|s| match self.rename_field {
                        RenameField::Title => s.title.clone(),
                        RenameField::Artist => s.artist.clone(),
                    })
                    .unwrap_or_default();
                true
            }
            KeyCode::Enter => {
                self.commit_rename_field();
                self.persist_rename();
                self.rename_mode = false;
                true
            }
            KeyCode::Esc => {
                self.rename_mode = false;
                self.rename_buf.clear();
                true
            }
            _ => false,
        }
    }

    fn commit_rename_field(&mut self) {
        let Some(idx) = self.real_index() else { return };
        let song = &mut self.songs[idx];
        match self.rename_field {
            RenameField::Title => song.title = self.rename_buf.clone(),
            RenameField::Artist => song.artist = self.rename_buf.clone(),
        }
    }

    fn persist_rename(&mut self) {
        let Some(idx) = self.real_index() else { return };
        let song = &self.songs[idx];
        let _ = crate::audio::import::rename_song(&song.dir, &song.title, &song.artist);
        self.import_status = Some(format!("Renamed: {} — {}", song.artist, song.title));
        self.apply_sort();
        self.rebuild_filter();
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

        // Top bar
        let top = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        render_top_bar(buf, top, &["MENU", "SONGS"]);

        // Bottom bar
        let bot = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        let bot_hints: &[(&str, &str)] = if self.import_mode {
            &[("Enter", "import"), ("Esc", "cancel")]
        } else if self.search_mode {
            &[("Enter", "confirm"), ("Esc", "clear")]
        } else if self.rename_mode {
            &[
                ("Tab", "title↔artist"),
                ("Enter", "save"),
                ("Esc", "cancel"),
            ]
        } else {
            &[
                ("↵", "play"),
                ("Tab", "difficulty"),
                ("s", "sort"),
                ("/", "search"),
                ("i", "import"),
                ("r", "rename"),
                ("x", "delete"),
                ("Esc", "back"),
            ]
        };
        render_bottom_bar(buf, bot, bot_hints);

        // Header title (inside content area)
        let cx = area.x + area.width / 2;
        let title = "SELECT SONG";
        buf.set_string(
            cx.saturating_sub(title.len() as u16 / 2),
            area.y + 2,
            title,
            Style::default().fg(Color::White).bold(),
        );

        // Difficulty + search status strip
        let diff_name = self.difficulty.to_string().to_uppercase();
        let diff_color = difficulty_color(self.difficulty);
        let diff_prefix = "Difficulty: ";
        let diff_total_w = (diff_prefix.len() + diff_name.len()) as u16;
        let dx = cx.saturating_sub(diff_total_w / 2);
        buf.set_string(
            dx,
            area.y + 3,
            diff_prefix,
            Style::default().fg(Color::Rgb(120, 120, 120)),
        );
        buf.set_string(
            dx + diff_prefix.len() as u16,
            area.y + 3,
            &diff_name,
            Style::default().fg(diff_color).bold(),
        );

        if self.search_mode || !self.search_query.is_empty() {
            let cursor = if self.search_mode { "_" } else { "" };
            let search_txt = format!("/ {}{}", self.search_query, cursor);
            let sw = search_txt.chars().count() as u16;
            buf.set_string(
                cx.saturating_sub(sw / 2),
                area.y + 4,
                &search_txt,
                Style::default().fg(Color::Rgb(200, 200, 100)),
            );
        }

        // Sort indicator
        let sort_txt = format!("Sort: {}", self.sort_mode.label());
        let sw = sort_txt.chars().count() as u16;
        buf.set_string(
            area.x + area.width.saturating_sub(sw + 2),
            area.y + 3,
            &sort_txt,
            Style::default().fg(Color::Rgb(110, 110, 110)),
        );

        if self.import_mode {
            let prompt = format!("Path to audio file: {}_", self.import_input);
            let x = area.x + 4;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, &prompt, Style::default().fg(Color::White));
            buf.set_string(
                x,
                y + 2,
                "Supports: mp3, wav, flac, ogg, m4a, opus, webm",
                Style::default().fg(Color::Rgb(80, 80, 80)),
            );
        } else if self.filtered_indices.is_empty() {
            let msg = if self.search_query.is_empty() {
                "No songs found. Press 'i' to import an audio file."
            } else {
                "No songs match the query."
            };
            let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            buf.set_string(
                x,
                area.y + area.height / 2,
                msg,
                Style::default().fg(Color::Rgb(100, 100, 100)),
            );
        } else {
            self.render_song_list(buf, area);
        }

        if let Some(status) = &self.import_status {
            let y = area.y + area.height - 3;
            buf.set_string(
                area.x + 2,
                y,
                status,
                Style::default().fg(Color::Rgb(120, 120, 120)),
            );
        }

        if self.rename_mode {
            self.render_rename_overlay(buf, area);
        }
    }

    fn render_rename_overlay(&self, buf: &mut Buffer, area: Rect) {
        let cx = area.x + area.width / 2;
        let cy = area.y + area.height / 2;
        let w: u16 = 60;
        let h: u16 = 7;
        let x = cx.saturating_sub(w / 2);
        let y = cy.saturating_sub(h / 2);

        for dy in 0..h {
            for dx in 0..w {
                buf.set_string(
                    x + dx,
                    y + dy,
                    " ",
                    Style::default().bg(Color::Rgb(20, 20, 28)),
                );
            }
        }
        for dx in 0..w {
            buf.set_string(
                x + dx,
                y,
                "─",
                Style::default()
                    .fg(Color::Rgb(80, 80, 100))
                    .bg(Color::Rgb(20, 20, 28)),
            );
            buf.set_string(
                x + dx,
                y + h - 1,
                "─",
                Style::default()
                    .fg(Color::Rgb(80, 80, 100))
                    .bg(Color::Rgb(20, 20, 28)),
            );
        }

        let title_focused = matches!(self.rename_field, RenameField::Title);
        let song = self.real_index().and_then(|i| self.songs.get(i));
        let (cur_title, cur_artist) = song
            .map(|s| (s.title.clone(), s.artist.clone()))
            .unwrap_or_default();

        let header = "RENAME SONG";
        buf.set_string(
            cx - header.len() as u16 / 2,
            y + 1,
            header,
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(20, 20, 28))
                .bold(),
        );

        let title_value = if title_focused {
            format!("{}_", self.rename_buf)
        } else {
            cur_title
        };
        let artist_value = if !title_focused {
            format!("{}_", self.rename_buf)
        } else {
            cur_artist
        };

        let label = |buf: &mut Buffer, focused: bool, label: &str, value: &str, row: u16| {
            let label_color = if focused {
                Color::Rgb(255, 240, 150)
            } else {
                Color::Rgb(120, 120, 120)
            };
            let value_color = if focused {
                Color::White
            } else {
                Color::Rgb(160, 160, 160)
            };
            buf.set_string(
                x + 2,
                row,
                label,
                Style::default()
                    .fg(label_color)
                    .bg(Color::Rgb(20, 20, 28))
                    .bold(),
            );
            buf.set_string(
                x + 12,
                row,
                value,
                Style::default().fg(value_color).bg(Color::Rgb(20, 20, 28)),
            );
        };

        label(buf, title_focused, "Title:", &title_value, y + 3);
        label(buf, !title_focused, "Artist:", &artist_value, y + 4);
    }

    fn render_song_list(&self, buf: &mut Buffer, area: Rect) {
        let list_y = area.y + 6;
        let list_h = area.height.saturating_sub(8) as usize;
        let max_visible = list_h.max(1);
        let start = if self.selected >= max_visible {
            self.selected - max_visible + 1
        } else {
            0
        };
        let diff_idx = Difficulty::all()
            .iter()
            .position(|d| *d == self.difficulty)
            .unwrap_or(2);
        let cx = area.x + area.width / 2;

        // Each row: 2 lines — title+dots+best, then meta strip (bpm/dur/notes).
        let row_h: u16 = 2;
        let mut drawn: u16 = 0;
        for (i, real_idx) in self.filtered_indices.iter().skip(start).enumerate() {
            let y = list_y + drawn;
            if y + row_h > area.y + area.height - 2 {
                break;
            }
            let is_sel = start + i == self.selected;
            let song = &self.songs[*real_idx];

            let (prefix, title_style) = if is_sel {
                ("▸ ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(130, 130, 130)))
            };

            // Accent bar on the left for selected row.
            if is_sel {
                for dy in 0..row_h {
                    buf.set_string(
                        area.x + 2,
                        y + dy,
                        "│",
                        Style::default()
                            .fg(difficulty_color(self.difficulty))
                            .bold(),
                    );
                }
            }

            // Row 1: title + artist
            let title_text = if song.artist.is_empty() {
                format!("{}{}", prefix, song.title)
            } else {
                format!("{}{} — {}", prefix, song.title, song.artist)
            };
            let row_x = area.x + 5;
            let max_title_w = (area.width.saturating_sub(26 + 12)) as usize;
            let truncated: String = title_text.chars().take(max_title_w).collect();
            buf.set_string(row_x, y, &truncated, title_style);

            // Best score on the far right of row 1.
            let best_txt = self
                .scores
                .get(&song.slug, &self.difficulty.to_string())
                .map(|b| format!("★ {} ({})", b.score, b.grade))
                .unwrap_or_else(|| String::from("—"));
            let bw = best_txt.chars().count() as u16;
            let bx = area.x + area.width.saturating_sub(bw + 3);
            let best_style = if best_txt == "—" {
                Style::default().fg(Color::Rgb(60, 60, 60))
            } else {
                Style::default().fg(Color::Rgb(255, 215, 0))
            };
            buf.set_string(bx, y, &best_txt, best_style);

            // Row 2: meta
            let y2 = y + 1;
            let bpm_txt = format!("{} BPM", song.bpm);
            let dur_txt = format!(
                "{}:{:02}",
                song.duration_ms / 60_000,
                (song.duration_ms / 1000) % 60
            );
            let notes_txt = format!("{} notes", song.note_counts[diff_idx]);
            let meta = format!("  {}  •  {}  •  {}", bpm_txt, dur_txt, notes_txt);
            buf.set_string(
                row_x,
                y2,
                &meta,
                Style::default().fg(Color::Rgb(110, 110, 110)),
            );

            // Difficulty dots (far right of row 2).
            let _ = cx;
            let dots_x = area.x + area.width.saturating_sub(12);
            render_difficulty_dots(buf, dots_x, y2, song.present, Some(self.difficulty));

            drawn += row_h;
        }
    }
}

/// Find audio file in a song directory. Supports mp3, m4a, webm, opus, ogg, wav, flac.
pub fn find_audio_file(dir: &Path) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("audio.") {
            let ext = name_str.rsplit('.').next().unwrap_or("");
            match ext {
                "mp3" | "m4a" | "webm" | "opus" | "ogg" | "wav" | "flac" => {
                    return Some(entry.path());
                }
                _ => {}
            }
        }
    }
    None
}
