use crate::audio::analyzer::{self, DecodedAudio, SpectrumAnalyzer};
use anyhow::Result;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source, buffer::SamplesBuffer};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(300);
const EXCERPT_MS: u64 = 20_000;
const FADE_MS: u64 = 800;
const LEVEL_WINDOW: usize = 1024;

pub type PreviewTarget = (PathBuf, u64);

pub struct Preview {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    volume: f32,
    wanted: Option<PreviewTarget>,
    wanted_since: Instant,
    current: Option<PreviewTarget>,
    pending: Option<Receiver<Result<DecodedAudio>>>,
    sink: Option<Sink>,
    mono: Vec<f32>,
    sample_rate: u32,
    analyzer: SpectrumAnalyzer,
}

impl Preview {
    pub fn new(volume: f32) -> Result<Self> {
        let (stream, handle) = OutputStream::try_default()?;
        Ok(Self {
            _stream: stream,
            handle,
            volume,
            wanted: None,
            wanted_since: Instant::now(),
            current: None,
            pending: None,
            sink: None,
            mono: Vec::new(),
            sample_rate: 44_100,
            analyzer: SpectrumAnalyzer::new(),
        })
    }

    pub fn request(&mut self, target: Option<PreviewTarget>) {
        if target != self.wanted {
            self.wanted = target;
            self.wanted_since = Instant::now();
        }
    }

    pub fn stop(&mut self) {
        self.request(None);
        self.tick();
    }

    pub fn tick(&mut self) {
        if self.wanted != self.current {
            if let Some(sink) = self.sink.take() {
                sink.stop();
            }
            self.pending = None;
            self.current = None;
            self.mono.clear();
            if let Some((path, start_ms)) = self.wanted.clone()
                && self.wanted_since.elapsed() >= DEBOUNCE
            {
                let (tx, rx) = mpsc::channel();
                let thread_path = path.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(analyzer::decode_excerpt(&thread_path, start_ms, EXCERPT_MS));
                });
                self.pending = Some(rx);
                self.current = Some((path, start_ms));
            }
            return;
        }

        let Some(rx) = &self.pending else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.pending = None;
        let Ok(mut audio) = result else { return };
        if audio.interleaved.is_empty() {
            return;
        }
        apply_fades(&mut audio);
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return;
        };
        sink.set_volume(self.volume);
        sink.append(
            SamplesBuffer::new(audio.channels, audio.sample_rate, audio.interleaved)
                .repeat_infinite(),
        );
        self.sink = Some(sink);
        self.mono = audio.mono;
        self.sample_rate = audio.sample_rate;
    }

    pub fn levels(&mut self) -> Option<[f32; 3]> {
        let sink = self.sink.as_ref()?;
        if self.mono.len() < LEVEL_WINDOW {
            return None;
        }
        let pos = (sink.get_pos().as_secs_f64() * self.sample_rate as f64) as usize;
        let start = pos % (self.mono.len() - LEVEL_WINDOW);
        self.analyzer
            .process(&self.mono[start..start + LEVEL_WINDOW]);
        let spectrum = self.analyzer.spectrum.lock().ok()?;
        let avg = |r: std::ops::Range<usize>| {
            let len = r.len().max(1) as f32;
            spectrum.bands.get(r).map(|b| b.iter().sum::<f32>() / len)
        };
        Some([avg(0..8)?, avg(8..18)?, avg(18..32)?])
    }
}

fn apply_fades(audio: &mut DecodedAudio) {
    let channels = audio.channels.max(1) as usize;
    let frames = audio.interleaved.len() / channels;
    let fade = ((FADE_MS * audio.sample_rate as u64 / 1000) as usize).min(frames / 2);
    for f in 0..fade {
        let gain = f as f32 / fade as f32;
        for c in 0..channels {
            for idx in [f * channels + c, (frames - 1 - f) * channels + c] {
                audio.interleaved[idx] = (audio.interleaved[idx] as f32 * gain) as i16;
            }
        }
    }
}
