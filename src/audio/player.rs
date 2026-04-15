use anyhow::Result;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct AudioPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    start_time: Option<Instant>,
    pause_elapsed_ms: u64,
    position_ms: Arc<AtomicU64>,
    is_playing: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        sink.pause();

        Ok(Self {
            _stream: stream,
            _stream_handle: stream_handle,
            sink,
            start_time: None,
            pause_elapsed_ms: 0,
            position_ms: Arc::new(AtomicU64::new(0)),
            is_playing: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let file = BufReader::new(File::open(path)?);
        let source = Decoder::new(file)?;
        self.sink.append(source);
        self.sink.pause();
        self.start_time = None;
        self.pause_elapsed_ms = 0;
        self.position_ms.store(0, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn play(&mut self) {
        self.sink.play();
        self.start_time = Some(Instant::now());
        self.is_playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&mut self) {
        if self.is_playing.load(Ordering::Relaxed) {
            self.pause_elapsed_ms = self.position_ms();
            self.sink.pause();
            self.start_time = None;
            self.is_playing.store(false, Ordering::Relaxed);
        }
    }

    pub fn resume(&mut self) {
        if !self.is_playing.load(Ordering::Relaxed) {
            self.sink.play();
            self.start_time = Some(Instant::now());
            self.is_playing.store(true, Ordering::Relaxed);
        }
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        self.start_time = None;
        self.pause_elapsed_ms = 0;
        self.position_ms.store(0, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
    }

    pub fn position_ms(&self) -> u64 {
        if let Some(start) = self.start_time {
            self.pause_elapsed_ms + start.elapsed().as_millis() as u64
        } else {
            self.pause_elapsed_ms
        }
    }

    pub fn update_position(&self) {
        self.position_ms.store(self.position_ms(), Ordering::Relaxed);
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.sink.set_volume(volume);
    }
}
