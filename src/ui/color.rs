pub type Rgb = (u8, u8, u8);

pub fn mul(c: Rgb, factor: f32) -> Rgb {
    (
        (c.0 as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.1 as f32 * factor).clamp(0.0, 255.0) as u8,
        (c.2 as f32 * factor).clamp(0.0, 255.0) as u8,
    )
}

pub fn add(a: Rgb, b: Rgb) -> Rgb {
    (
        (a.0 as u16 + b.0 as u16).min(255) as u8,
        (a.1 as u16 + b.1 as u16).min(255) as u8,
        (a.2 as u16 + b.2 as u16).min(255) as u8,
    )
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
