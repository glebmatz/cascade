use ratatui::prelude::*;
use ratatui::widgets::Widget;

const BUTTON_WIDTH: u16 = 20;
const BUTTON_HEIGHT: u16 = 3; // top border + text + bottom border

pub struct MenuList<'a> {
    pub items: &'a [&'a str],
    pub selected: usize,
}

impl<'a> Widget for MenuList<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let total_height = self.items.len() as u16 * BUTTON_HEIGHT + (self.items.len() as u16 - 1); // buttons + gaps
        let start_y = area.y + area.height.saturating_sub(total_height) / 2;
        let cx = area.x + area.width / 2;

        for (i, item) in self.items.iter().enumerate() {
            let y = start_y + i as u16 * (BUTTON_HEIGHT + 1);
            if y + BUTTON_HEIGHT > area.y + area.height { break; }

            let is_selected = i == self.selected;
            let bx = cx.saturating_sub(BUTTON_WIDTH / 2);

            if is_selected {
                // Selected: bright bg, white text, "pressed" look
                let bg = Color::Rgb(50, 50, 60);
                let fg = Color::Rgb(255, 255, 255);
                let border_fg = Color::Rgb(120, 120, 140);

                // Top border
                let top = format!("+{}+", "-".repeat(BUTTON_WIDTH as usize - 2));
                buf.set_string(bx, y, &top, Style::default().fg(border_fg));

                // Middle: text centered with bg fill
                for x in bx..bx + BUTTON_WIDTH {
                    if x < area.x + area.width {
                        buf.set_string(x, y + 1, " ", Style::default().bg(bg));
                    }
                }
                let text_x = cx.saturating_sub(item.chars().count() as u16 / 2);
                buf.set_string(bx, y + 1, "|", Style::default().fg(border_fg).bg(bg));
                buf.set_string(text_x, y + 1, item, Style::default().fg(fg).bg(bg).bold());
                if bx + BUTTON_WIDTH - 1 < area.x + area.width {
                    buf.set_string(bx + BUTTON_WIDTH - 1, y + 1, "|", Style::default().fg(border_fg).bg(bg));
                }

                // Bottom border
                let bot = format!("+{}+", "-".repeat(BUTTON_WIDTH as usize - 2));
                buf.set_string(bx, y + 2, &bot, Style::default().fg(border_fg));

                // Selection indicator
                let arrow_x = bx.saturating_sub(2);
                buf.set_string(arrow_x, y + 1, ">", Style::default().fg(Color::Rgb(200, 200, 220)).bold());
            } else {
                // Unselected: dim, no fill
                let border_fg = Color::Rgb(40, 40, 45);
                let fg = Color::Rgb(80, 80, 80);

                let top = format!("+{}+", "-".repeat(BUTTON_WIDTH as usize - 2));
                buf.set_string(bx, y, &top, Style::default().fg(border_fg));

                buf.set_string(bx, y + 1, "|", Style::default().fg(border_fg));
                let text_x = cx.saturating_sub(item.chars().count() as u16 / 2);
                buf.set_string(text_x, y + 1, item, Style::default().fg(fg));
                if bx + BUTTON_WIDTH - 1 < area.x + area.width {
                    buf.set_string(bx + BUTTON_WIDTH - 1, y + 1, "|", Style::default().fg(border_fg));
                }

                let bot = format!("+{}+", "-".repeat(BUTTON_WIDTH as usize - 2));
                buf.set_string(bx, y + 2, &bot, Style::default().fg(border_fg));
            }
        }
    }
}
