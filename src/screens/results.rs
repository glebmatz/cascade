use crate::achievements::AchievementId;
use crate::app::{Action, Screen};
use crate::game::modifiers::Mods;
use crate::game::state::GameState;
use crate::score_store::BestScore;
use crate::ui::chrome::{render_bottom_bar, render_top_bar};
use ratatui::prelude::*;

pub struct ResultsScreen {
    pub state: GameState,
    pub song_title: String,
    pub difficulty: String,
    pub selected: usize,
    pub prev_best: Option<BestScore>,
    pub new_best: bool,
    pub anim_score: f64,
    pub anim_combo: f64,
    pub anim_acc: f64,
    pub anim_ticks: u32,
    pub unlocked: Vec<AchievementId>,
    pub mods: Mods,
}

impl ResultsScreen {
    pub fn new(
        state: GameState,
        song_title: String,
        difficulty: String,
        prev_best: Option<BestScore>,
        new_best: bool,
        unlocked: Vec<AchievementId>,
        mods: Mods,
    ) -> Self {
        Self {
            state,
            song_title,
            difficulty,
            selected: 0,
            prev_best,
            new_best,
            anim_score: 0.0,
            anim_combo: 0.0,
            anim_acc: 0.0,
            anim_ticks: 0,
            unlocked,
            mods,
        }
    }

    /// Animate numbers toward final values. Call every frame.
    pub fn update(&mut self) {
        self.anim_ticks = self.anim_ticks.saturating_add(1);
        // ease-out cubic, ~90 frames (1.5s at 60fps).
        let target = 90.0_f64;
        let t = (self.anim_ticks as f64 / target).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        self.anim_score = self.state.score as f64 * eased;
        self.anim_combo = self.state.max_combo as f64 * eased;
        self.anim_acc = self.state.accuracy() * eased;
    }

    #[allow(dead_code)]
    pub fn anim_done(&self) -> bool {
        self.anim_ticks >= 90
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
            Action::Back | Action::Pause => Some(Action::Navigate(Screen::SongSelect)),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();
        let cx = area.x + area.width / 2;

        // Chrome
        let top = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        render_top_bar(buf, top, &["MENU", "SONGS", "PLAY", "RESULTS"]);
        let bot = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        render_bottom_bar(
            buf,
            bot,
            &[("↑↓", "select"), ("↵", "confirm"), ("Esc", "back")],
        );

        let mut y = area.y + 2;

        // Title
        let title = "RESULTS";
        buf.set_string(
            cx.saturating_sub(title.len() as u16 / 2),
            y,
            title,
            Style::default().fg(Color::White).bold(),
        );
        y += 2;

        // Song + difficulty pill
        let song_info = format!("{} [{}]", self.song_title, self.difficulty);
        let dw = song_info.chars().count() as u16;
        buf.set_string(
            cx.saturating_sub(dw / 2),
            y,
            &song_info,
            Style::default().fg(Color::Rgb(140, 140, 140)),
        );
        y += 2;

        // Big ASCII grade letter
        let grade = self.state.grade();
        let grade_color = match grade {
            "SS" => Color::Rgb(255, 235, 80), // bright gold
            "S" => Color::Rgb(255, 215, 0),   // gold
            "A" => Color::Rgb(110, 220, 110), // green
            "B" => Color::Rgb(110, 170, 240), // blue
            "C" => Color::Rgb(200, 200, 200), // grey
            _ => Color::Rgb(220, 90, 90),     // red
        };
        render_big_letter(buf, cx, y, grade, grade_color);
        y += 7;

        if self.new_best {
            let nb = "★  NEW BEST  ★";
            let w = nb.chars().count() as u16;
            buf.set_string(
                cx.saturating_sub(w / 2),
                y,
                nb,
                Style::default().fg(Color::Rgb(255, 215, 0)).bold(),
            );
            y += 2;
        } else if let Some(prev) = &self.prev_best {
            let txt = format!("best: {}   (grade {})", prev.score, prev.grade);
            let w = txt.chars().count() as u16;
            buf.set_string(
                cx.saturating_sub(w / 2),
                y,
                &txt,
                Style::default().fg(Color::Rgb(100, 100, 100)),
            );
            y += 2;
        } else {
            y += 1;
        }

        // Animated headline stats
        let score_line = format!("SCORE  {:>8}", self.anim_score as u64);
        let acc_line = format!("ACC    {:>7.1}%", self.anim_acc);
        let combo_line = format!("COMBO  {:>7}x", self.anim_combo as u32);

        buf.set_string(
            cx.saturating_sub(score_line.len() as u16 / 2),
            y,
            &score_line,
            Style::default().fg(Color::White).bold(),
        );
        y += 1;
        buf.set_string(
            cx.saturating_sub(acc_line.len() as u16 / 2),
            y,
            &acc_line,
            Style::default().fg(Color::Rgb(190, 190, 190)),
        );
        y += 1;
        buf.set_string(
            cx.saturating_sub(combo_line.len() as u16 / 2),
            y,
            &combo_line,
            Style::default().fg(Color::Rgb(190, 190, 190)),
        );
        y += 2;

        // Judgement histogram
        let counts = &self.state.judgement_counts;
        let max_count = (*counts.iter().max().unwrap_or(&1)).max(1) as f64;
        let bar_max_w = (area.width / 2).min(30) as f64;
        let labels = ["PERFECT", "GREAT", "GOOD", "MISS"];
        let colors: [(u8, u8, u8); 4] = [
            (255, 220, 120),
            (130, 220, 140),
            (170, 170, 170),
            (220, 90, 90),
        ];
        let bar_x_start = cx.saturating_sub((bar_max_w as u16 + 18) / 2);
        for (i, label) in labels.iter().enumerate() {
            let cnt = counts[i] as f64;
            let w = ((cnt / max_count) * bar_max_w) as u16;
            let (cr, cg, cb) = colors[i];
            // label
            buf.set_string(
                bar_x_start,
                y,
                format!("{:<8}", label),
                Style::default().fg(Color::Rgb(cr, cg, cb)).bold(),
            );
            // bar
            for dx in 0..bar_max_w as u16 {
                let style = if dx < w {
                    Style::default().bg(Color::Rgb(cr, cg, cb))
                } else {
                    Style::default().bg(Color::Rgb(30, 30, 32))
                };
                buf.set_string(bar_x_start + 9 + dx, y, " ", style);
            }
            // count
            buf.set_string(
                bar_x_start + 10 + bar_max_w as u16,
                y,
                format!("{:>5}", counts[i]),
                Style::default().fg(Color::Rgb(180, 180, 180)),
            );
            y += 1;
        }
        y += 1;

        // Mods badge
        if !self.mods.is_empty() {
            let badge = format!("Played with {}", self.mods.badge());
            let w = badge.chars().count() as u16;
            buf.set_string(
                cx.saturating_sub(w / 2),
                y,
                &badge,
                Style::default().fg(Color::Rgb(255, 200, 100)),
            );
            y += 1;
        }

        // Achievements unlocked
        if !self.unlocked.is_empty() {
            y += 1;
            let header = "★ ACHIEVEMENTS UNLOCKED ★";
            let hw = header.chars().count() as u16;
            buf.set_string(
                cx.saturating_sub(hw / 2),
                y,
                header,
                Style::default().fg(Color::Rgb(255, 215, 0)).bold(),
            );
            y += 1;
            for id in &self.unlocked {
                let line = format!("  {} — {}", id.name(), id.description());
                let w = line.chars().count() as u16;
                buf.set_string(
                    cx.saturating_sub(w / 2),
                    y,
                    &line,
                    Style::default().fg(Color::Rgb(220, 220, 180)),
                );
                y += 1;
            }
            y += 1;
        } else {
            y += 1;
        }

        // Options
        let options = ["Retry", "Back to songs"];
        for (i, option) in options.iter().enumerate() {
            let (prefix, style) = if i == self.selected {
                ("▸ ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
            };
            let text = format!("{}{}", prefix, option);
            let w = text.chars().count() as u16;
            buf.set_string(cx.saturating_sub(w / 2), y, &text, style);
            y += 1;
        }
    }
}

// 6-row × 6-col ASCII glyphs for grade letters.
const GLYPH_ROWS: usize = 6;
const GLYPH_COLS: usize = 6;

fn glyph(ch: &str) -> [&'static str; GLYPH_ROWS] {
    match ch {
        "S" => [" ████ ", "█     ", " ███  ", "    █ ", "█   █ ", " ███  "],
        "A" => ["  ██  ", " █  █ ", "█    █", "██████", "█    █", "█    █"],
        "B" => ["█████ ", "█    █", "█████ ", "█    █", "█    █", "█████ "],
        "C" => [" ████ ", "█    █", "█     ", "█     ", "█    █", " ████ "],
        _ => ["████  ", "█   █ ", "█    █", "█    █", "█   █ ", "████  "],
    }
}

fn render_big_letter(buf: &mut Buffer, cx: u16, y: u16, letter: &str, color: Color) {
    let chars: Vec<char> = letter.chars().collect();
    let glyph_w = GLYPH_COLS as u16;
    let gap: u16 = 1;
    let total_w = chars.len() as u16 * glyph_w + (chars.len() as u16 - 1) * gap;
    let start_x = cx.saturating_sub(total_w / 2);

    for (idx, ch) in chars.iter().enumerate() {
        let g = glyph(&ch.to_string());
        let off_x = start_x + idx as u16 * (glyph_w + gap);
        for (row_i, row) in g.iter().enumerate() {
            let py = y + row_i as u16;
            for (ci, c) in row.chars().enumerate() {
                if c == '█' {
                    buf.set_string(off_x + ci as u16, py, "█", Style::default().fg(color));
                }
            }
        }
    }
}
