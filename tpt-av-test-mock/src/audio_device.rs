//! Virtual audio device: a deterministic sine generator on the input side
//! and a recording buffer sink on the output side.
//!
//! [`MockAudioDevice`] stands in for a sound card in CI. As a *source* it
//! fills caller-provided interleaved buffers with a phase-continuous sine
//! (or silence) exactly like a real driver callback; as a *sink* it accepts
//! the buffers your code renders and keeps statistics (frames, peak, RMS)
//! for assertions.
//!
//! ```rust
//! use tpt_av_test_mock::audio_device::MockAudioDevice;
//!
//! // "Input": a 440 Hz stereo tone at 48 kHz, injected into buffers.
//! let mut device = MockAudioDevice::sine(48_000, 2, 440.0).with_amplitude(0.5);
//! let mut buffer = vec![0.0f32; 512 * 2];
//! device.fill(&mut buffer);
//! assert_eq!(buffer.len(), 1024);
//! assert!(buffer.iter().all(|sample| sample.abs() <= 0.5 + 1e-6));
//!
//! // "Output": the code under test renders, the device records.
//! device.accept(&buffer);
//! assert_eq!(device.total_frames_accepted(), 512);
//! assert!(device.peak_accepted() > 0.4);
//! ```

use std::fmt;

/// Configuration error for a virtual audio device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioDeviceError {
    /// The interleaved buffer length is not a multiple of the channel count.
    BufferNotFrameAligned { length: usize, channels: usize },
    /// The buffer is not the block size the device was opened with.
    UnexpectedBlockSize { expected: usize, got: usize },
}

impl fmt::Display for AudioDeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioDeviceError::BufferNotFrameAligned { length, channels } => {
                write!(
                    f,
                    "buffer of {length} samples is not a multiple of {channels} channels"
                )
            }
            AudioDeviceError::UnexpectedBlockSize { expected, got } => {
                write!(f, "device opened for {expected}-sample blocks, got {got}")
            }
        }
    }
}

impl std::error::Error for AudioDeviceError {}

/// A virtual audio device running at a fixed sample rate and channel count.
///
/// The generator state (sine phase) advances across calls, so consecutive
/// blocks are phase-continuous — a ramp or FFT in the test sees a real tone,
/// not 512 independent snippets.
#[derive(Debug, Clone)]
pub struct MockAudioDevice {
    sample_rate: u32,
    channels: usize,
    frequency: Option<f64>,
    amplitude: f32,
    phase: f64,
    frames_generated: u64,
    frames_accepted: u64,
    accepted: Vec<Vec<f32>>,
    peak: f32,
    squared_sum: f64,
    strict_block_size: bool,
    block_size: Option<usize>,
}

impl MockAudioDevice {
    /// A silent stereo device at 48 kHz. Use [`MockAudioDevice::sine`] for a
    /// tone, or [`MockAudioDevice::with_frequency`] to add one later.
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        assert!(sample_rate > 0, "sample_rate must be non-zero");
        assert!(channels > 0, "channels must be non-zero");
        MockAudioDevice {
            sample_rate,
            channels,
            frequency: None,
            amplitude: 0.5,
            phase: 0.0,
            frames_generated: 0,
            frames_accepted: 0,
            accepted: Vec::new(),
            peak: 0.0,
            squared_sum: 0.0,
            strict_block_size: false,
            block_size: None,
        }
    }

    /// A device generating a sine tone at `frequency` Hz (defaults to
    /// amplitude 0.5; chain [`MockAudioDevice::with_amplitude`]).
    ///
    /// The frequency is clamped into `(0, sample_rate / 2)` — a generator
    /// outside the Nyquist limit is a fixture bug, and aliasing it silently
    /// would hide that.
    pub fn sine(sample_rate: u32, channels: usize, frequency: f64) -> Self {
        let mut device = MockAudioDevice::new(sample_rate, channels);
        device.set_frequency(frequency);
        device
    }

    /// Sets the tone frequency (clamped into `(0, sample_rate / 2)`); use
    /// `None` (via [`MockAudioDevice::with_frequency`]) for silence.
    pub fn set_frequency(&mut self, frequency: f64) {
        let nyquist = f64::from(self.sample_rate) / 2.0;
        let clamped = frequency.clamp(f64::MIN_POSITIVE, nyquist);
        self.frequency = Some(clamped);
    }

    /// Builder version of [`MockAudioDevice::set_frequency`].
    pub fn with_frequency(mut self, frequency: f64) -> Self {
        self.set_frequency(frequency);
        self
    }

    /// Builder: sets the sine amplitude (`0.0 ..= 1.0`).
    pub fn with_amplitude(mut self, amplitude: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&amplitude),
            "amplitude must be within 0.0..=1.0"
        );
        self.amplitude = amplitude;
        self
    }

    /// Builder: makes [`MockAudioDevice::accept`] and
    /// [`MockAudioDevice::fill`] reject buffers whose sample count differs
    /// from `block_size` (the driver contract in tpt-audio).
    pub fn with_fixed_block_size(mut self, block_size: usize) -> Self {
        assert!(block_size > 0, "block_size must be non-zero");
        self.block_size = Some(block_size);
        self.strict_block_size = true;
        self
    }

    /// The configured sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The configured channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Total frames generated so far (across all `fill`/`generate` calls).
    pub fn total_frames_generated(&self) -> u64 {
        self.frames_generated
    }

    /// Total frames accepted so far (across all `accept` calls).
    pub fn total_frames_accepted(&self) -> u64 {
        self.frames_accepted
    }

    /// Peak absolute sample among everything accepted so far.
    pub fn peak_accepted(&self) -> f32 {
        self.peak
    }

    /// RMS level of everything accepted so far.
    pub fn rms_accepted(&self) -> f32 {
        if self.frames_accepted == 0 {
            return 0.0;
        }
        let samples = (self.frames_accepted * self.channels as u64) as f64;
        (self.squared_sum / samples).sqrt() as f32
    }

    /// Buffers accepted so far, oldest first.
    pub fn accepted_buffers(&self) -> &[Vec<f32>] {
        &self.accepted
    }

    /// Forgets accepted buffers (counters and peak/RMS stay).
    pub fn clear_accepted(&mut self) {
        self.accepted.clear();
    }

    /// Fills `buffer` (interleaved, `frames * channels` samples) with the
    /// next block of tone or silence, advancing the generator phase.
    ///
    /// # Errors
    /// If the device has a fixed block size and `buffer` does not match it,
    /// or the length is not frame-aligned.
    pub fn fill(&mut self, buffer: &mut [f32]) -> Result<(), AudioDeviceError> {
        self.check_buffer(buffer)?;
        if buffer.len() % self.channels != 0 {
            return Err(AudioDeviceError::BufferNotFrameAligned {
                length: buffer.len(),
                channels: self.channels,
            });
        }
        let phase_step = self.phase_step();
        for sample in buffer.iter_mut() {
            *sample = self.next_sample(phase_step);
        }
        self.frames_generated += (buffer.len() / self.channels) as u64;
        Ok(())
    }

    /// Allocating convenience: [`MockAudioDevice::fill`] into a fresh vector
    /// of `frames` frames.
    pub fn generate(&mut self, frames: usize) -> Result<Vec<f32>, AudioDeviceError> {
        let mut buffer = vec![0.0f32; frames * self.channels];
        self.fill(&mut buffer)?;
        Ok(buffer)
    }

    /// Accepts a rendered interleaved buffer (the output side of the
    /// device): records it and folds it into the peak/RMS/frame counters.
    ///
    /// # Errors
    /// If the buffer is not frame-aligned, or violates a fixed block size.
    pub fn accept(&mut self, buffer: &[f32]) -> Result<(), AudioDeviceError> {
        self.check_buffer(buffer)?;
        if buffer.len() % self.channels != 0 {
            return Err(AudioDeviceError::BufferNotFrameAligned {
                length: buffer.len(),
                channels: self.channels,
            });
        }
        for &sample in buffer {
            let magnitude = sample.abs();
            if magnitude > self.peak {
                self.peak = magnitude;
            }
            self.squared_sum += f64::from(sample) * f64::from(sample);
        }
        self.frames_accepted += (buffer.len() / self.channels) as u64;
        self.accepted.push(buffer.to_vec());
        Ok(())
    }

    fn check_buffer(&self, buffer: &[f32]) -> Result<(), AudioDeviceError> {
        if self.strict_block_size {
            let expected = self.block_size.unwrap_or_default() * self.channels;
            if buffer.len() != expected {
                return Err(AudioDeviceError::UnexpectedBlockSize {
                    expected,
                    got: buffer.len(),
                });
            }
        }
        Ok(())
    }

    fn phase_step(&self) -> f64 {
        match self.frequency {
            Some(frequency) => 2.0 * std::f64::consts::PI * frequency / f64::from(self.sample_rate),
            None => 0.0,
        }
    }

    fn next_sample(&mut self, phase_step: f64) -> f32 {
        if self.frequency.is_none() {
            return 0.0;
        }
        let sample = self.amplitude * self.phase.sin() as f32;
        self.phase += phase_step;
        // Keep the phase bounded so long runs cannot lose precision.
        if self.phase >= std::f64::consts::TAU {
            self.phase -= std::f64::consts::TAU;
        }
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_the_default_source() {
        let mut device = MockAudioDevice::new(48_000, 2);
        let block = device.generate(512).unwrap();
        assert_eq!(block.len(), 1024);
        assert!(block.iter().all(|sample| *sample == 0.0));
        assert_eq!(device.total_frames_generated(), 512);
    }

    #[test]
    fn sine_matches_the_math_and_stays_phase_continuous() {
        // 1000 Hz at 48 kHz: 48 samples per period.
        let mut device = MockAudioDevice::sine(48_000, 1, 1000.0).with_amplitude(0.25);
        let first = device.generate(48).unwrap();
        let expected: Vec<f32> = (0..48)
            .map(|n| {
                0.25 * (2.0 * std::f64::consts::PI * 1000.0 * n as f64 / 48_000.0).sin() as f32
            })
            .collect();
        for (got, want) in first.iter().zip(&expected) {
            assert!((got - want).abs() < 1e-4, "{got} vs {want}");
        }
        // One full period later the phase must have wrapped, not drifted.
        let second = device.generate(48).unwrap();
        for (got, want) in second.iter().zip(&expected) {
            assert!((got - want).abs() < 1e-4, "{got} vs {want}");
        }
        assert_eq!(device.total_frames_generated(), 96);
    }

    #[test]
    fn frequency_is_clamped_to_nyquist() {
        let mut device = MockAudioDevice::sine(48_000, 1, 1_000_000.0);
        assert!(device.frequency.unwrap() <= f64::from(48_000) / 2.0);
        let block = device.generate(64).unwrap();
        assert!(block.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn fill_is_frame_aligned() {
        let mut device = MockAudioDevice::sine(48_000, 2, 440.0);
        let mut odd = vec![0.0f32; 7];
        assert!(matches!(
            device.fill(&mut odd),
            Err(AudioDeviceError::BufferNotFrameAligned { .. })
        ));
        let mut even = vec![0.0f32; 64];
        assert!(device.fill(&mut even).is_ok());
    }

    #[test]
    fn accept_records_frames_peak_and_rms() {
        let mut device = MockAudioDevice::new(48_000, 2);
        device.accept(&[0.5, -0.5, 0.25, -0.25]).unwrap();
        device.accept(&[1.0, -1.0]).unwrap();
        assert_eq!(device.total_frames_accepted(), 3);
        assert_eq!(device.accepted_buffers().len(), 2);
        assert!((device.peak_accepted() - 1.0).abs() < 1e-6);
        // RMS of [0.5, 0.5, 0.25, 0.25, 1.0, 1.0].
        let expected = ((0.25f64 + 0.25 + 0.0625 + 0.0625 + 1.0 + 1.0) / 6.0).sqrt();
        assert!((f64::from(device.rms_accepted()) - expected).abs() < 1e-6);
        device.clear_accepted();
        assert!(device.accepted_buffers().is_empty());
        assert_eq!(device.total_frames_accepted(), 3, "counters survive clear");
    }

    #[test]
    fn fixed_block_size_contract_is_enforced() {
        let mut device = MockAudioDevice::sine(48_000, 2, 440.0).with_fixed_block_size(512);
        let wrong = vec![0.0f32; 1024 * 2];
        assert!(matches!(
            device.accept(&wrong),
            Err(AudioDeviceError::UnexpectedBlockSize { .. })
        ));
        let right = vec![0.0f32; 512 * 2];
        assert!(device.accept(&right).is_ok());
        assert!(device.fill(&mut vec![0.0f32; 512 * 2]).is_ok());
    }
}
