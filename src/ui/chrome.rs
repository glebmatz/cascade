use crate::beatmap::types::Difficulty;
use ratatui::prelude::*;

pub fn difficulty_color(d: Difficulty) -> Color {
    match d {
        Difficulty::Easy => Color::Rgb(110, 220, 110), // green
        Difficulty::Medium => Color::Rgb(230, 210, 80), // yellow
        Difficulty::Hard => Color::Rgb(240, 150, 60),  // orange
        Difficulty::Expert => Color::Rgb(230, 80, 100), // red
    }
}

#[allow(dead_code)]
pub fn difficulty_color_from_name(name: &str) -> Color {
    match name.to_lowercase().as_str() {
        "easy" => Color::Rgb(110, 220, 110),
        "medium" => Color::Rgb(230, 210, 80),
        "hard" => Color::Rgb(240, 150, 60),
        "expert" => Color::Rgb(230, 80, 100),
        _ => Color::Rgb(160, 160, 160),
    }
}

#[allow(dead_code)]
pub fn render_difficulty_pill(buf: &mut Buffer, x: u16, y: u16, d: Difficulty) -> u16 {
    let name = d.to_string().to_uppercase();
    let pill = format!(" {} ", name);
    let color = difficulty_color(d);
    buf.set_string(x, y, "[", Style::default().fg(Color::Rgb(70, 70, 70)));
    buf.set_string(x + 1, y, &pill, Style::default().fg(color).bold());
    let end_x = x + 1 + pill.chars().count() as u16;
    buf.set_string(end_x, y, "]", Style::default().fg(Color::Rgb(70, 70, 70)));
    end_x + 1
}

pub fn render_difficulty_dots(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    present: [bool; 4],
    highlight: Option<Difficulty>,
) {
    let diffs = Difficulty::all();
    for (i, d) in diffs.iter().enumerate() {
        let ch = if present[i] { "●" } else { "·" };
        let mut color = if present[i] {
            difficulty_color(*d)
        } else {
            Color::Rgb(60, 60, 60)
        };
        let mut bold = false;
        if Some(*d) == highlight {
            bold = true;
            if !present[i] {
                color = Color::Rgb(100, 100, 100);
            }
        }
        let mut style = Style::default().fg(color);
        if bold {
            style = style.bold();
        }
        buf.set_string(x + i as u16 * 2, y, ch, style);
    }
}

pub fn render_top_bar(buf: &mut Buffer, area: Rect, crumbs: &[&str]) {
    if area.height == 0 {
        return;
    }
    let y = area.y;
    let bg = Color::Rgb(18, 18, 22);
    for x in area.x..area.x + area.width {
        buf.set_string(x, y, " ", Style::default().bg(bg));
    }

    let mut cx = area.x + 2;
    for (i, crumb) in crumbs.iter().enumerate() {
        let is_last = i == crumbs.len() - 1;
        let style = if is_last {
            Style::default().fg(Color::White).bg(bg).bold()
        } else {
            Style::default().fg(Color::Rgb(110, 110, 120)).bg(bg)
        };
        buf.set_string(cx, y, crumb, style);
        cx += crumb.chars().count() as u16;
        if !is_last {
            buf.set_string(
                cx,
                y,
                "  ›  ",
                Style::default().fg(Color::Rgb(70, 70, 80)).bg(bg),
            );
            cx += 5;
        }
    }
}

pub fn render_bottom_bar(buf: &mut Buffer, area: Rect, hints: &[(&str, &str)]) {
    if area.height == 0 {
        return;
    }
    let y = area.y + area.height - 1;
    for x in area.x..area.x + area.width {
        buf.set_string(x, y, " ", Style::default().bg(Color::Rgb(14, 14, 18)));
    }
    let mut cx = area.x + 2;
    let key_style = Style::default()
        .fg(Color::Rgb(40, 40, 45))
        .bg(Color::Rgb(180, 180, 190))
        .bold();
    let desc_style = Style::default()
        .fg(Color::Rgb(170, 170, 180))
        .bg(Color::Rgb(14, 14, 18));
    let gap_style = Style::default().bg(Color::Rgb(14, 14, 18));
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            buf.set_string(cx, y, "   ", gap_style);
            cx += 3;
        }
        let kstr = format!(" {} ", key);
        buf.set_string(cx, y, &kstr, key_style);
        cx += kstr.chars().count() as u16;
        buf.set_string(cx, y, " ", gap_style);
        cx += 1;
        buf.set_string(cx, y, desc, desc_style);
        cx += desc.chars().count() as u16;
        if cx >= area.x + area.width {
            break;
        }
    }
}
