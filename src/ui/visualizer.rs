use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::audio::analyzer::SpectrumData;

const WAVE_CHARS: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
const BLOCK_CHARS: &[&str] = &["░", "▒", "▓", "█"];

pub struct WaveVisualizer<'a> {
    pub spectrum: &'a SpectrumData,
}

impl<'a> Widget for WaveVisualizer<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        for x in 0..area.width {
            let band_idx = if self.spectrum.bands.is_empty() {
                0
            } else {
                (x as usize * self.spectrum.bands.len() / area.width as usize)
                    .min(self.spectrum.bands.len() - 1)
            };
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
            let band_idx = if self.spectrum.bands.is_empty() {
                0
            } else {
                (y as usize * self.spectrum.bands.len() / area.height as usize)
                    .min(self.spectrum.bands.len() - 1)
            };
            let value = self.spectrum.bands.get(band_idx).copied().unwrap_or(0.0);

            for x in 0..area.width {
                let dist = match self.side {
                    Side::Left => (area.width - 1 - x) as f32 / area.width.max(1) as f32,
                    Side::Right => x as f32 / area.width.max(1) as f32,
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
