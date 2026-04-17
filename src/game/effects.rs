use crate::ui::color::Rgb;

#[derive(Clone, Copy)]
pub struct Star {
    pub x: f32,
    pub y_px: f32,
    pub speed: f32,
    pub brightness: u8,
}

#[derive(Clone, Copy)]
pub struct Particle {
    pub x: f32,
    pub y_px: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: u8,
    pub max_life: u8,
    pub color: Rgb,
}

impl Particle {
    pub fn step(&mut self) {
        self.x += self.vx;
        self.y_px += self.vy;
        self.vy += GRAVITY_PER_FRAME;
        self.vx *= DRAG_PER_FRAME;
        self.life = self.life.saturating_sub(1);
    }

    pub fn alpha(&self) -> f32 {
        self.life as f32 / self.max_life.max(1) as f32
    }
}

const GRAVITY_PER_FRAME: f32 = 0.18;
const DRAG_PER_FRAME: f32 = 0.94;
