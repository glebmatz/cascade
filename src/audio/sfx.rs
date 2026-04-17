use crate::game::hit_judge::Judgement;
use anyhow::Result;
use rodio::{OutputStream, OutputStreamHandle, Sink, buffer::SamplesBuffer};

pub const SFX_SAMPLE_RATE: u32 = 44_100;

pub struct SfxPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    pub volume: f32,
}

impl SfxPlayer {
    pub fn new(volume: f32) -> Result<Self> {
        let (stream, handle) = OutputStream::try_default()?;
        Ok(Self {
            _stream: stream,
            handle,
            volume,
        })
    }

    pub fn play(&self, samples: Vec<f32>) {
        if let Ok(sink) = Sink::try_new(&self.handle) {
            sink.set_volume(self.volume);
            sink.append(SamplesBuffer::new(1, SFX_SAMPLE_RATE, samples));
            sink.detach();
        }
    }
}

pub fn sfx_for(judgement: Judgement) -> Vec<f32> {
    match judgement {
        Judgement::Perfect => perfect_chime(),
        Judgement::Great => great_click(),
        Judgement::Good => good_click(),
        Judgement::Miss => miss_thud(),
    }
}

fn envelope(i: usize, len: usize, attack_len: usize, sharpness: f32) -> f32 {
    let t_norm = i as f32 / len as f32;
    let attack = (i as f32 / attack_len.max(1) as f32).min(1.0);
    let decay = (-t_norm * sharpness).exp();
    attack * decay
}

fn sine(freq: f32, t: f32) -> f32 {
    (2.0 * std::f32::consts::PI * freq * t).sin()
}

fn perfect_chime() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.070) as usize;
    let attack = (sr * 0.002) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 6.0);
        let s = sine(1320.0, t) * 0.55 + sine(1760.0, t) * 0.30 + sine(2640.0, t) * 0.15;
        out.push(s * env * 0.55);
    }
    out
}

fn great_click() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.055) as usize;
    let attack = (sr * 0.002) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 7.0);
        let s = sine(880.0, t) * 0.7 + sine(1320.0, t) * 0.25;
        out.push(s * env * 0.5);
    }
    out
}

fn good_click() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.04) as usize;
    let attack = (sr * 0.003) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 8.0);
        let s = sine(520.0, t) * 0.6 + sine(780.0, t) * 0.2;
        out.push(s * env * 0.38);
    }
    out
}

fn miss_thud() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.11) as usize;
    let attack = (sr * 0.004) as usize;
    let mut out = Vec::with_capacity(len);
    let mut rng: u32 = 0xC0FFEE;
    for i in 0..len {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((rng >> 16) as f32 / 32768.0) - 1.0;
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 5.0);
        let sweep = 70.0 + 110.0 * (-t * 15.0).exp();
        let s = sine(sweep, t) * 0.75 + noise * 0.18;
        out.push(s * env * 0.45);
    }
    out
}

pub fn nav_tick() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.025) as usize;
    let attack = (sr * 0.001) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 10.0);
        let s = sine(620.0, t) * 0.5 + sine(930.0, t) * 0.2;
        out.push(s * env * 0.22);
    }
    out
}

pub fn nav_select() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.08) as usize;
    let attack = (sr * 0.002) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 7.0);
        let freq = 700.0 + 520.0 * (1.0 - (-t * 12.0).exp());
        let s = sine(freq, t) * 0.55 + sine(freq * 1.5, t) * 0.2;
        out.push(s * env * 0.32);
    }
    out
}

pub fn nav_back() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.07) as usize;
    let attack = (sr * 0.002) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 8.0);
        let freq = 520.0 - 280.0 * (1.0 - (-t * 10.0).exp());
        let s = sine(freq, t) * 0.5;
        out.push(s * env * 0.28);
    }
    out
}

pub fn milestone_ding() -> Vec<f32> {
    let sr = SFX_SAMPLE_RATE as f32;
    let len = (sr * 0.18) as usize;
    let attack = (sr * 0.003) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let t = i as f32 / sr;
        let env = envelope(i, len, attack, 3.0);
        let s = sine(1046.0, t) * 0.45  // C6
              + sine(1318.0, t) * 0.32  // E6
              + sine(1568.0, t) * 0.22; // G6
        out.push(s * env * 0.45);
    }
    out
}
