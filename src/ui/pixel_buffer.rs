use crate::ui::color::Rgb;

/// Half-row pixel buffer. Two sub-pixels stack into one terminal cell rendered as `▀`.
pub struct PixelBuffer {
    pub width: u16,
    pub height_px: i32,
    pixels: Vec<Rgb>,
}

impl PixelBuffer {
    pub fn new(width: u16, height_px: i32) -> Self {
        Self {
            width,
            height_px,
            pixels: vec![(0, 0, 0); (width as usize) * (height_px as usize)],
        }
    }

    pub fn set(&mut self, x: u16, py: i32, c: Rgb) {
        if let Some(idx) = self.idx(x, py) {
            self.pixels[idx] = c;
        }
    }

    pub fn blend(&mut self, x: u16, py: i32, c: Rgb, alpha: f32) {
        let Some(idx) = self.idx(x, py) else { return };
        let cur = self.pixels[idx];
        let a = alpha.clamp(0.0, 1.0);
        let inv = 1.0 - a;
        self.pixels[idx] = (
            (cur.0 as f32 * inv + c.0 as f32 * a) as u8,
            (cur.1 as f32 * inv + c.1 as f32 * a) as u8,
            (cur.2 as f32 * inv + c.2 as f32 * a) as u8,
        );
    }

    pub fn add(&mut self, x: u16, py: i32, c: Rgb, mult: f32) {
        let Some(idx) = self.idx(x, py) else { return };
        let cur = self.pixels[idx];
        self.pixels[idx] = (
            (cur.0 as u16 + (c.0 as f32 * mult) as u16).min(255) as u8,
            (cur.1 as u16 + (c.1 as f32 * mult) as u16).min(255) as u8,
            (cur.2 as u16 + (c.2 as f32 * mult) as u16).min(255) as u8,
        );
    }

    pub fn get(&self, x: u16, py: i32) -> Rgb {
        self.idx(x, py).map(|i| self.pixels[i]).unwrap_or((0, 0, 0))
    }

    fn idx(&self, x: u16, py: i32) -> Option<usize> {
        if py < 0 || py >= self.height_px || x >= self.width {
            return None;
        }
        Some((py as usize) * (self.width as usize) + x as usize)
    }
}
