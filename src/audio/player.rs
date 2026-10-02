use anyhow::Result;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, buffer::SamplesBuffer};
use std::cell::Cell;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, Instant};

const MAX_EXTRAPOLATION: Duration = Duration::from_millis(60);

pub struct AudioPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    started: bool,
    clock: Cell<Clock>,
}

#[derive(Clone, Copy)]
struct Clock {
    raw: Duration,
    raw_at: Instant,
    last: Duration,
}

impl Clock {
    fn at(pos: Duration) -> Self {
        Self {
            raw: pos,
            raw_at: Instant::now(),
            last: pos,
        }
    }
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
            started: false,
            clock: Cell::new(Clock::at(Duration::ZERO)),
        })
    }

    #[allow(dead_code)]
    pub fn load(&mut self, path: &Path) -> Result<()> {
        let file = BufReader::new(File::open(path)?);
        let source = Decoder::new(file)?;
        self.sink.append(source);
        self.sink.pause();
        self.started = false;
        Ok(())
    }

    pub fn load_samples(
        &mut self,
        interleaved: Vec<i16>,
        channels: u16,
        sample_rate: u32,
    ) -> Result<()> {
        let buf = SamplesBuffer::new(channels.max(1), sample_rate, interleaved);
        self.sink.append(buf);
        self.sink.pause();
        self.started = false;
        Ok(())
    }

    pub fn play(&mut self) {
        self.sink.play();
        self.started = true;
        self.clock.set(Clock::at(self.sink.get_pos()));
    }

    pub fn pause(&mut self) {
        self.sink.pause();
    }

    pub fn resume(&mut self) {
        let last = self.clock.get().last;
        self.sink.play();
        self.clock.set(Clock {
            last,
            ..Clock::at(self.sink.get_pos())
        });
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        self.started = false;
    }

    pub fn position_ms(&self) -> u64 {
        if !self.started {
            return 0;
        }
        let raw = self.sink.get_pos();
        let mut clock = self.clock.get();
        if raw != clock.raw {
            clock.raw = raw;
            clock.raw_at = Instant::now();
        }
        let estimate = if self.sink.is_paused() {
            clock.raw
        } else {
            clock.raw + clock.raw_at.elapsed().min(MAX_EXTRAPOLATION)
        };
        clock.last = estimate.max(clock.last);
        self.clock.set(clock);
        clock.last.as_millis() as u64
    }

    pub fn update_position(&self) {
        // no-op: position is queried directly from sink
    }

    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused() && self.started
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.sink.set_volume(volume);
    }

    /// Jump playback to a wall-clock position. A successful seek may produce
    /// a small (~50ms) audible gap with `SamplesBuffer` — acceptable for
    /// practice-mode looping. Failures are swallowed; the caller can retry.
    pub fn seek_to_ms(&self, ms: u64) -> Result<()> {
        self.sink
            .try_seek(Duration::from_millis(ms))
            .map_err(|e| anyhow::anyhow!("seek failed: {e}"))?;
        self.clock.set(Clock::at(self.sink.get_pos()));
        Ok(())
    }

    /// Set playback speed. `1.0` is normal. Values < 1.0 slow down and drop
    /// pitch; `rodio` does not time-stretch. Pitch change is acceptable for
    /// practice mode.
    pub fn set_speed(&self, speed: f32) {
        self.sink.set_speed(speed);
    }

    /// Fire-and-forget SFX on a detached sink sharing the same output device.
    /// Music playback and position are unaffected.
    pub fn play_sfx(&self, samples: Vec<f32>, sample_rate: u32, volume: f32) {
        if let Ok(sfx_sink) = Sink::try_new(&self._stream_handle) {
            sfx_sink.set_volume(volume);
            sfx_sink.append(SamplesBuffer::new(1, sample_rate, samples));
            sfx_sink.detach();
        }
    }
}
