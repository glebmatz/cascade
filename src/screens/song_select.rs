use ratatui::prelude::*;
use crate::app::{Action, Screen};
use crate::beatmap::types::Difficulty;
use std::path::{Path, PathBuf};

pub struct SongEntry {
    pub title: String,
    pub artist: String,
    pub dir: PathBuf,
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

    pub fn scan_songs(&mut self, songs_dir: &Path) {
        self.songs.clear();
        if !songs_dir.exists() { return; }

        let Ok(entries) = std::fs::read_dir(songs_dir) else { return };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }

            let meta_path = path.join("metadata.json");
            let (title, artist) = if meta_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                        (
                            meta["title"].as_str().unwrap_or("Unknown").to_string(),
                            meta["artist"].as_str().unwrap_or("").to_string(),
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

            // Check if at least one beatmap AND audio file exist
            let has_beatmap = Difficulty::all().iter().any(|d| path.join(d.filename()).exists());
            let has_audio = find_audio_file(&path).is_some();
            if !has_beatmap || !has_audio { continue; }

            self.songs.push(SongEntry { title, artist, dir: path });
        }

        self.songs.sort_by(|a, b| a.title.cmp(&b.title));
        if self.selected >= self.songs.len() {
            self.selected = 0;
        }
    }

    pub fn selected_beatmap_path(&self) -> Option<PathBuf> {
        self.songs.get(self.selected).map(|s| s.dir.join(self.difficulty.filename()))
    }

    pub fn selected_audio_path(&self) -> Option<PathBuf> {
        self.songs.get(self.selected).and_then(|s| find_audio_file(&s.dir))
    }

    pub fn selected_song_title(&self) -> String {
        self.songs.get(self.selected).map(|s| {
            if s.artist.is_empty() {
                s.title.clone()
            } else {
                format!("{} — {}", s.title, s.artist)
            }
        }).unwrap_or_default()
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        if self.import_mode {
            return None; // text input handled separately
        }
        match action {
            Action::MenuUp => {
                if self.selected > 0 { self.selected -= 1; }
                None
            }
            Action::MenuDown => {
                if self.selected < self.songs.len().saturating_sub(1) { self.selected += 1; }
                None
            }
            Action::MenuSelect => {
                if !self.songs.is_empty() {
                    Some(Action::Navigate(Screen::Gameplay))
                } else {
                    None
                }
            }
            Action::Tab => {
                self.cycle_difficulty();
                None
            }
            Action::Back | Action::Pause => Some(Action::Navigate(Screen::Menu)),
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

        let title = "SELECT SONG";
        let x = area.x + (area.width.saturating_sub(title.len() as u16)) / 2;
        buf.set_string(x, area.y + 1, title, Style::default().fg(Color::White).bold());

        let diff_text = format!("Difficulty: {} (Tab to change)", self.difficulty);
        let x = area.x + (area.width.saturating_sub(diff_text.len() as u16)) / 2;
        buf.set_string(x, area.y + 3, &diff_text, Style::default().fg(Color::Rgb(140, 140, 140)));

        if self.import_mode {
            let prompt = format!("YouTube URL: {}_", self.import_input);
            let x = area.x + 4;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, &prompt, Style::default().fg(Color::White));
            buf.set_string(x, y + 2, "Enter: Import   ESC: Cancel", Style::default().fg(Color::Rgb(80, 80, 80)));
        } else if self.songs.is_empty() {
            let msg = "No songs found. Press 'i' to import from YouTube.";
            let x = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            buf.set_string(x, area.y + area.height / 2, msg, Style::default().fg(Color::Rgb(100, 100, 100)));
        } else {
            let list_y = area.y + 5;
            let max_visible = (area.height.saturating_sub(8)) as usize;
            let start = if self.selected >= max_visible { self.selected - max_visible + 1 } else { 0 };

            for (i, song) in self.songs.iter().skip(start).take(max_visible).enumerate() {
                let idx = start + i;
                let y = list_y + i as u16;
                if y >= area.y + area.height - 2 { break; }

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

        if let Some(status) = &self.import_status {
            let y = area.y + area.height - 2;
            buf.set_string(area.x + 2, y, status, Style::default().fg(Color::Rgb(120, 120, 120)));
        }

        let footer = "Enter: Play  Tab: Difficulty  i: Import  ESC: Back";
        let x = area.x + (area.width.saturating_sub(footer.len() as u16)) / 2;
        buf.set_string(x, area.y + area.height - 1, footer, Style::default().fg(Color::Rgb(60, 60, 60)));
    }
}

/// Find audio file in a song directory. Supports mp3, m4a, webm, opus, ogg, wav, flac.
pub fn find_audio_file(dir: &Path) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return None };
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
