use ratatui::prelude::*;

use crate::app::{Action, Screen};
use crate::stats::{
    DiffRow, HEATMAP_DAYS, StatsSummary, TopSong, format_duration_ms, heatmap_glyphs, sparkline_30d,
};
use crate::ui::chrome::{render_bottom_bar, render_top_bar};

pub struct StatsScreen {
    pub summary: StatsSummary,
}

impl StatsScreen {
    pub fn new(summary: StatsSummary) -> Self {
        Self { summary }
    }

    pub fn handle_action(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::Back | Action::Pause | Action::Quit => Some(Action::Navigate(Screen::Menu)),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let buf = frame.buffer_mut();

        // Chrome.
        let top = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        render_top_bar(buf, top, &["MENU", "STATS"]);
        let bot = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        render_bottom_bar(buf, bot, &[("Esc", "back"), ("Q", "quit")]);

        // Minimum size guard.
        if area.width < 60 || area.height < 20 {
            let msg = "Terminal too small — resize to at least 60×20.";
            let x = area.x + area.width.saturating_sub(msg.chars().count() as u16) / 2;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, msg, Style::default().fg(Color::Rgb(200, 160, 100)));
            return;
        }

        let s = &self.summary;
        let empty = s.total_plays == 0;

        let pad_x = 3u16;
        let inner_x = area.x + pad_x;
        let inner_w = area.width.saturating_sub(pad_x * 2);
        let mut y = area.y + 2;

        // Title.
        let title = "STATISTICS";
        let tx = area.x + (area.width.saturating_sub(title.len() as u16)) / 2;
        buf.set_string(tx, y, title, Style::default().fg(Color::White).bold());
        y += 2;

        if empty {
            let msg = "No plays yet. Play a song to start tracking stats.";
            let mx = area.x + (area.width.saturating_sub(msg.len() as u16)) / 2;
            let my = area.y + area.height / 2;
            buf.set_string(mx, my, msg, Style::default().fg(Color::Rgb(140, 140, 140)));
            return;
        }

        // Row: totals (three "cards" side by side).
        y = render_totals_row(buf, inner_x, y, inner_w, s);
        y += 1;

        // Row: top songs + per-difficulty.
        y = render_top_and_diff(buf, inner_x, y, inner_w, &s.top_songs, &s.per_difficulty);
        y += 1;

        // Accuracy sparkline.
        y = render_accuracy_sparkline(buf, inner_x, y, inner_w, s);
        y += 1;

        // Activity heatmap.
        y = render_heatmap(buf, inner_x, y, inner_w, &s.heatmap_30d);
        y += 1;

        // Achievements progress.
        render_achievements_progress(
            buf,
            inner_x,
            y,
            inner_w,
            s.achievements_unlocked,
            s.achievements_total,
        );
    }
}

fn render_totals_row(buf: &mut Buffer, x: u16, y: u16, w: u16, s: &StatsSummary) -> u16 {
    let cards = [
        ("PLAYS", format!("{}", s.total_plays)),
        ("TIME PLAYED", format_duration_ms(s.total_time_played_ms)),
        ("NOTES HIT", format_number(s.total_notes_hit)),
    ];
    let card_w = w / cards.len() as u16;
    for (i, (label, value)) in cards.iter().enumerate() {
        let cx = x + i as u16 * card_w;
        buf.set_string(cx, y, label, Style::default().fg(Color::Rgb(120, 120, 130)));
        buf.set_string(cx, y + 1, value, Style::default().fg(Color::White).bold());
    }
    y + 2
}

fn render_top_and_diff(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    w: u16,
    top: &[TopSong],
    diffs: &[DiffRow],
) -> u16 {
    let half = w / 2;
    // Left: top songs.
    buf.set_string(
        x,
        y,
        "TOP SONGS",
        Style::default().fg(Color::Rgb(160, 200, 220)).bold(),
    );
    if top.is_empty() {
        buf.set_string(
            x,
            y + 1,
            "(none)",
            Style::default().fg(Color::Rgb(100, 100, 100)),
        );
    } else {
        for (i, ts) in top.iter().enumerate() {
            let line_y = y + 1 + i as u16;
            let title: String = ts
                .title
                .chars()
                .take((half as usize).saturating_sub(10))
                .collect();
            let text = format!(
                "{}. {:<.*}",
                i + 1,
                (half as usize).saturating_sub(10),
                title
            );
            buf.set_string(
                x,
                line_y,
                text,
                Style::default().fg(Color::Rgb(220, 220, 220)),
            );
            let count = format!("{} plays", ts.plays);
            let cw = count.chars().count() as u16;
            buf.set_string(
                x + half - cw - 2,
                line_y,
                count,
                Style::default().fg(Color::Rgb(160, 160, 160)),
            );
        }
    }

    // Right: per-difficulty.
    let rx = x + half;
    buf.set_string(
        rx,
        y,
        "BY DIFFICULTY",
        Style::default().fg(Color::Rgb(200, 180, 140)).bold(),
    );
    if diffs.is_empty() {
        buf.set_string(
            rx,
            y + 1,
            "(none)",
            Style::default().fg(Color::Rgb(100, 100, 100)),
        );
    } else {
        for (i, d) in diffs.iter().enumerate() {
            let line_y = y + 1 + i as u16;
            let color = diff_color(&d.difficulty);
            let label = format!("{:<7}", d.difficulty.to_uppercase());
            buf.set_string(rx, line_y, &label, Style::default().fg(color).bold());
            let body = format!(
                "{:>4} plays   best {:>5.1}%   avg {:>5.1}%",
                d.plays, d.best_accuracy, d.avg_accuracy
            );
            buf.set_string(
                rx + 8,
                line_y,
                &body,
                Style::default().fg(Color::Rgb(200, 200, 200)),
            );
        }
    }

    let rows = top.len().max(diffs.len()).max(1) as u16;
    y + 1 + rows
}

fn render_accuracy_sparkline(buf: &mut Buffer, x: u16, y: u16, w: u16, s: &StatsSummary) -> u16 {
    buf.set_string(
        x,
        y,
        "ACCURACY (LAST 30 DAYS)",
        Style::default().fg(Color::Rgb(160, 200, 220)).bold(),
    );
    let line = sparkline_30d(&s.accuracy_30d);
    buf.set_string(
        x,
        y + 1,
        &line,
        Style::default().fg(Color::Rgb(150, 220, 150)),
    );
    // Show 30d overall mean for extra context.
    let vals: Vec<f64> = s.accuracy_30d.iter().filter_map(|v| *v).collect();
    let hint = if vals.is_empty() {
        "no plays in last 30 days".to_string()
    } else {
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        format!("avg {:.1}%   (blanks = days with no plays)", mean)
    };
    let hx = x + (w.saturating_sub(hint.chars().count() as u16)).min(w.saturating_sub(1));
    let _ = hx;
    buf.set_string(
        x,
        y + 2,
        &hint,
        Style::default().fg(Color::Rgb(120, 120, 130)),
    );
    y + 3
}

fn render_heatmap(buf: &mut Buffer, x: u16, y: u16, w: u16, counts: &[u32; HEATMAP_DAYS]) -> u16 {
    buf.set_string(
        x,
        y,
        "ACTIVITY (LAST 30 DAYS)",
        Style::default().fg(Color::Rgb(200, 180, 140)).bold(),
    );
    let glyphs = heatmap_glyphs(counts);
    // Spread glyphs with a space between for readability when width allows.
    let spaced = if w >= (HEATMAP_DAYS as u16) * 2 {
        glyphs
            .chars()
            .map(|c| format!("{} ", c))
            .collect::<String>()
    } else {
        glyphs
    };
    buf.set_string(
        x,
        y + 1,
        &spaced,
        Style::default().fg(Color::Rgb(140, 200, 160)),
    );
    let legend = "  less  · ░ ▒ ▓ █  more";
    buf.set_string(
        x,
        y + 2,
        legend,
        Style::default().fg(Color::Rgb(110, 110, 120)),
    );
    y + 3
}

fn render_achievements_progress(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    w: u16,
    unlocked: usize,
    total: usize,
) {
    buf.set_string(
        x,
        y,
        "ACHIEVEMENTS",
        Style::default().fg(Color::Rgb(255, 215, 0)).bold(),
    );
    let bar_w = (w.saturating_sub(20)).min(40);
    let filled = if total == 0 {
        0
    } else {
        ((unlocked as f64 / total as f64) * bar_w as f64).round() as u16
    };
    let y2 = y + 1;
    for i in 0..bar_w {
        let ch = if i < filled { "█" } else { "░" };
        let col = if i < filled {
            Color::Rgb(255, 215, 0)
        } else {
            Color::Rgb(60, 60, 65)
        };
        buf.set_string(x + i, y2, ch, Style::default().fg(col));
    }
    let label = format!("  {} / {} unlocked", unlocked, total);
    buf.set_string(
        x + bar_w,
        y2,
        &label,
        Style::default().fg(Color::Rgb(220, 220, 220)),
    );
}

fn diff_color(name: &str) -> Color {
    match name.to_lowercase().as_str() {
        "easy" => Color::Rgb(110, 220, 110),
        "medium" => Color::Rgb(230, 210, 80),
        "hard" => Color::Rgb(240, 150, 60),
        "expert" => Color::Rgb(230, 80, 100),
        _ => Color::Rgb(180, 180, 180),
    }
}

fn format_number(n: u64) -> String {
    // Insert thin space as a thousands separator for readability.
    let s = n.to_string();
    let bytes: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    let len = bytes.len();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(*c);
    }
    out
}
