use ratatui::prelude::*;
use ratatui::widgets::Widget;
use crate::audio::analyzer::SpectrumData;

/// Top rhythm bar — bouncing spectrum
pub struct WaveVisualizer<'a> {
    pub spectrum: &'a SpectrumData,
}

impl<'a> Widget for WaveVisualizer<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.spectrum.bands.is_empty() {
            return;
        }

        // Multi-row bar chart from top
        for x in 0..area.width {
            let band_idx = (x as usize * self.spectrum.bands.len() / area.width as usize)
                .min(self.spectrum.bands.len() - 1);
            let value = self.spectrum.bands[band_idx];

            let max_rows = area.height;
            let filled = (value * max_rows as f32) as u16;

            for row in 0..filled.min(max_rows) {
                let y = area.y + row;
                // Brighter at top, dimmer further down
                let fade = 1.0 - (row as f32 / max_rows as f32) * 0.5;
                let brightness = (value * 80.0 * fade + 15.0).min(95.0) as u8;
                let ch = if row == 0 { "_" } else { "." };
                buf.set_string(
                    area.x + x, y, ch,
                    Style::default().fg(Color::Rgb(brightness, brightness, brightness)),
                );
            }
        }
    }
}

/// Side gradient background — subtle vertical gradient that pulses with music
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

        for y in 0..area.height {
            // Vertical gradient: bright at bottom, dark at top
            let vert_t = y as f32 / area.height as f32; // 0=top, 1=bottom
            let base_brightness = vert_t * 25.0 + 5.0; // 5 at top, 30 at bottom

            // Get spectrum value for this vertical position
            let band_idx = (y as usize * self.spectrum.bands.len() / area.height as usize)
                .min(self.spectrum.bands.len() - 1);
            let energy = self.spectrum.bands[band_idx];

            // Music pulse adds brightness
            let pulse = energy * 20.0;
            let brightness = (base_brightness + pulse).min(50.0) as u8;

            if brightness < 8 { continue; }

            for x in 0..area.width {
                // Horizontal fade: dim away from highway
                let horiz_t = match self.side {
                    Side::Left => x as f32 / area.width as f32,          // 0=far, 1=near highway
                    Side::Right => 1.0 - x as f32 / area.width as f32,  // 1=near highway, 0=far
                };
                let h_fade = horiz_t * 0.7 + 0.3; // 0.3 at edge, 1.0 near highway

                let final_b = (brightness as f32 * h_fade) as u8;
                if final_b < 6 { continue; }

                buf.set_string(
                    area.x + x, area.y + y, " ",
                    Style::default().bg(Color::Rgb(final_b, final_b, final_b)),
                );
            }
        }
    }
}
