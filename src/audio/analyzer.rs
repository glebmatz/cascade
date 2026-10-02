use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::{Arc, Mutex};

pub struct SpectrumData {
    pub bands: Vec<f32>,
    pub energy: f32,
}

impl SpectrumData {
    pub fn empty(num_bands: usize) -> Self {
        Self {
            bands: vec![0.0; num_bands],
            energy: 0.0,
        }
    }
}

const ANALYZER_FFT_SIZE: usize = 1024;
const NUM_BANDS: usize = 32;

pub struct SpectrumAnalyzer {
    planner: FftPlanner<f32>,
    buffer: Vec<Complex<f32>>,
    band_ranges: Vec<(usize, usize)>,
    pub spectrum: Arc<Mutex<SpectrumData>>,
    decay: f32,
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrumAnalyzer {
    pub fn new() -> Self {
        let planner = FftPlanner::new();
        let buffer = vec![Complex::new(0.0, 0.0); ANALYZER_FFT_SIZE];

        let mut band_ranges = Vec::with_capacity(NUM_BANDS);
        let min_freq = 20.0f32;
        let max_freq = 16000.0f32;
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        for i in 0..NUM_BANDS {
            let low = ((log_min + (log_max - log_min) * i as f32 / NUM_BANDS as f32).exp()
                / 44100.0
                * ANALYZER_FFT_SIZE as f32) as usize;
            let high = ((log_min + (log_max - log_min) * (i + 1) as f32 / NUM_BANDS as f32).exp()
                / 44100.0
                * ANALYZER_FFT_SIZE as f32) as usize;
            band_ranges.push((low.max(1), high.max(low + 1).min(ANALYZER_FFT_SIZE / 2)));
        }

        Self {
            planner,
            buffer,
            band_ranges,
            spectrum: Arc::new(Mutex::new(SpectrumData::empty(NUM_BANDS))),
            decay: 0.85,
        }
    }

    #[allow(dead_code)]
    pub fn shared_spectrum(&self) -> Arc<Mutex<SpectrumData>> {
        Arc::clone(&self.spectrum)
    }

    pub fn process(&mut self, samples: &[f32]) {
        let fft = self.planner.plan_fft_forward(ANALYZER_FFT_SIZE);

        let start = samples.len().saturating_sub(ANALYZER_FFT_SIZE);
        for (i, val) in self.buffer.iter_mut().enumerate() {
            let sample_idx = start + i;
            let s = if sample_idx < samples.len() {
                samples[sample_idx]
            } else {
                0.0
            };
            let window = 0.5
                * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / ANALYZER_FFT_SIZE as f32).cos());
            *val = Complex::new(s * window, 0.0);
        }

        fft.process(&mut self.buffer);

        let magnitudes: Vec<f32> = self.buffer[..ANALYZER_FFT_SIZE / 2]
            .iter()
            .map(|c| c.norm())
            .collect();

        let max_mag = magnitudes.iter().copied().fold(0.0f32, f32::max).max(0.001);

        let mut spectrum = self.spectrum.lock().unwrap();
        let mut total_energy = 0.0f32;

        for (i, &(low, high)) in self.band_ranges.iter().enumerate() {
            let sum: f32 = magnitudes[low..high.min(magnitudes.len())].iter().sum();
            let avg = sum / (high - low).max(1) as f32;
            let normalized = (avg / max_mag).clamp(0.0, 1.0);

            let prev = spectrum.bands.get(i).copied().unwrap_or(0.0);
            spectrum.bands[i] = if normalized > prev {
                normalized
            } else {
                prev * self.decay + normalized * (1.0 - self.decay)
            };
            total_energy += spectrum.bands[i];
        }

        spectrum.energy = (total_energy / NUM_BANDS as f32).clamp(0.0, 1.0);
    }
}

pub struct DecodedAudio {
    pub mono: Vec<f32>,
    pub interleaved: Vec<i16>,
    pub channels: u16,
    pub sample_rate: u32,
}

pub fn decode_audio(path: &std::path::Path) -> anyhow::Result<(Vec<f32>, u32)> {
    let decoded = decode(path, false, None)?;
    Ok((decoded.mono, decoded.sample_rate))
}

pub fn decode_for_playback(path: &std::path::Path) -> anyhow::Result<DecodedAudio> {
    decode(path, true, None)
}

pub fn decode_excerpt(
    path: &std::path::Path,
    start_ms: u64,
    length_ms: u64,
) -> anyhow::Result<DecodedAudio> {
    decode(path, true, Some((start_ms, length_ms)))
}

fn decode(
    path: &std::path::Path,
    keep_interleaved: bool,
    excerpt: Option<(u64, u64)>,
) -> anyhow::Result<DecodedAudio> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    use symphonia::core::units::Time;

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| anyhow::anyhow!("no audio track"))?;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let track_id = track.id;

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut frame_limit = usize::MAX;
    if let Some((start_ms, length_ms)) = excerpt {
        if start_ms > 0 {
            let time = Time::new(start_ms / 1000, (start_ms % 1000) as f64 / 1000.0);
            let _ = format.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time,
                    track_id: Some(track_id),
                },
            );
            decoder.reset();
        }
        frame_limit = (length_ms as f64 / 1000.0 * sample_rate as f64) as usize;
    }

    let mut out = DecodedAudio {
        mono: Vec::new(),
        interleaved: Vec::new(),
        channels: 1,
        sample_rate,
    };

    while out.mono.len() < frame_limit {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        let channels = spec.channels.count().max(1);
        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let samples = sample_buf.samples();
        let take = (frame_limit - out.mono.len()).min(samples.len() / channels);
        let samples = &samples[..take * channels];
        out.channels = channels as u16;

        if keep_interleaved {
            out.interleaved.extend(
                samples
                    .iter()
                    .map(|&v| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
            );
        }
        if channels == 1 {
            out.mono.extend_from_slice(samples);
        } else {
            for chunk in samples.chunks(channels) {
                out.mono.push(chunk.iter().sum::<f32>() / channels as f32);
            }
        }
    }

    Ok(out)
}
