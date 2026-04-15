use ratatui::prelude::*;
use ratatui::widgets::Widget;

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
                ("> ", Style::default().fg(Color::White).bold())
            } else {
                ("  ", Style::default().fg(Color::Rgb(100, 100, 100)))
            };

            let text = format!("{}{}", prefix, item);
            let text_w = text.chars().count() as u16;
            let x = area.x + (area.width.saturating_sub(text_w)) / 2;
            buf.set_string(x, y, &text, style);
        }
    }
}
