use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::audio::analyzer::SpectrumData;

/// Thin top bar — single row of varying height bar characters
pub struct WaveVisualizer<'a> {
    pub spectrum: &'a SpectrumData,
}

impl<'a> Widget for WaveVisualizer<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.spectrum.bands.is_empty() {
            return;
        }

        let chars = &[' ', ' ', '_', '.', '-', '=', '#'];

        for x in 0..area.width {
            let band_idx = (x as usize * self.spectrum.bands.len() / area.width as usize)
                .min(self.spectrum.bands.len() - 1);
            let value = self.spectrum.bands[band_idx];
            let idx = (value * (chars.len() - 1) as f32) as usize;
            let idx = idx.min(chars.len() - 1);

            if idx > 1 {
                let brightness = (value * 50.0 + 20.0).min(70.0) as u8;
                buf.set_string(
                    area.x + x,
                    area.y,
                    &chars[idx].to_string(),
                    Style::default().fg(Color::Rgb(brightness, brightness, brightness)),
                );
            }
        }
    }
}

/// Side glow — just a thin 1-2 column accent strip
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
        if area.width == 0 || area.height == 0 || self.spectrum.bands.is_empty() {
            return;
        }

        // Only draw 1 column right next to the highway
        for y in 0..area.height {
            let band_idx = (y as usize * self.spectrum.bands.len() / area.height as usize)
                .min(self.spectrum.bands.len() - 1);
            let value = self.spectrum.bands[band_idx];

            if value < 0.2 { continue; }

            let x = match self.side {
                Side::Left => area.x + area.width - 1,
                Side::Right => area.x,
            };

            let brightness = (value * 40.0 + 10.0).min(50.0) as u8;
            let ch = if value > 0.6 { "|" } else { ":" };
            buf.set_string(x, area.y + y, ch, Style::default().fg(Color::Rgb(brightness, brightness, brightness)));
        }
    }
}
