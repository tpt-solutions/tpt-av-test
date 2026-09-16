//! Simulated audio callback benchmarking at the stack's standard block
//! sizes (512 / 1024 frames).
//!
//! A real audio callback has two hard rules: finish inside the block
//! duration (a 512-frame block at 48 kHz is ~10.67 ms), and never touch the
//! heap. [`benchmark_audio_block`] simulates the callback loop: it prepares
//! an interleaved output buffer up front, then repeatedly invokes the
//! processing closure with an [`AudioBlockContext`] while the
//! [`AllocationTracker`](crate::allocation_tracker::AllocationTracker) and a
//! monotonic clock watch every iteration. The resulting
//! [`AudioBlockResult`] knows how to fail a test or bench for either rule.
//!
//! ```rust
//! use tpt_av_test_benchmark::audio_block::{benchmark_audio_block, AudioBlockContext};
//!
//! // An allocation-free "mixer" (pure arithmetic on the caller's buffer).
//! fn process(buffer: &mut [f32], context: AudioBlockContext) {
//!     for (index, sample) in buffer.iter_mut().enumerate() {
//!         let channel = index % context.channels;
//!         *sample = if channel == 0 { 0.25 } else { -0.25 };
//!     }
//! }
//!
//! let result = benchmark_audio_block(512, 32, process);
//! assert_eq!(result.allocations.allocations, 0, "no heap traffic allowed");
//! assert!(
//!     result.stats.max() <= result.block_duration(),
//!     "callback ran longer than one 512-sample block at 48 kHz"
//! );
//! ```

use core::fmt;
use core::time::Duration;
use std::time::Instant;

use crate::allocation_tracker::{AllocationStats, AllocationTracker};
use crate::timing::TimingStats;

/// Default sample rate for simulated callbacks (48 kHz, the TPT AV Stack's
/// project rate).
pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;

/// Default channel count (stereo, interleaved).
pub const DEFAULT_CHANNELS: usize = 2;

/// The standard small callback: 512 frames per block.
pub const BLOCK_SIZE_512: usize = 512;

/// The standard large callback: 1024 frames per block.
pub const BLOCK_SIZE_1024: usize = 1024;

/// Wall-clock duration of one block: `block_size / sample_rate`.
///
/// ```rust
/// use core::time::Duration;
/// use tpt_av_test_benchmark::audio_block::block_duration;
///
/// assert_eq!(
///     block_duration(512, 48_000),
///     Duration::from_nanos(10_666_667)
/// );
/// assert_eq!(
///     block_duration(1024, 48_000),
///     Duration::from_nanos(21_333_333)
/// );
/// ```
pub fn block_duration(block_size: usize, sample_rate: u32) -> Duration {
    assert!(block_size > 0, "block_size must be non-zero");
    assert!(sample_rate > 0, "sample_rate must be non-zero");
    Duration::from_secs_f64(block_size as f64 / sample_rate as f64)
}

/// What the processing closure learns about the block it is filling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioBlockContext {
    /// Sample rate the device is running at.
    pub sample_rate: u32,
    /// Channels per frame (the buffer is interleaved: `block_size * channels`
    /// samples).
    pub channels: usize,
    /// Frames in this block.
    pub block_size: usize,
    /// Zero-based index of this block since the benchmark started
    /// (warm-up blocks included, so the value keeps increasing smoothly).
    pub frame_index: u64,
}

/// Runs `process` for `iterations` blocks of `block_size` frames and reports
/// the timing and heap statistics of every simulated callback.
///
/// `iterations` warm-up callbacks (untracked, untimed) run first so caches
/// and branch predictors reach steady state before measurement.
///
/// Panics if `block_size` is zero, if `block_size * channels` would overflow,
/// or if `iterations` is zero.
pub fn benchmark_audio_block<F>(
    block_size: usize,
    iterations: usize,
    process: F,
) -> AudioBlockResult
where
    F: FnMut(&mut [f32], AudioBlockContext),
{
    AudioBlockBenchmark::new(block_size).run(iterations, process)
}

/// Configurable variant of [`benchmark_audio_block`] for non-default sample
/// rates, channel counts, and warm-up lengths.
pub struct AudioBlockBenchmark {
    block_size: usize,
    sample_rate: u32,
    channels: usize,
    warmup: usize,
}

impl AudioBlockBenchmark {
    /// A 512-frame stereo benchmark at 48 kHz with a 16-block warm-up.
    pub fn new(block_size: usize) -> Self {
        assert!(block_size > 0, "block_size must be non-zero");
        AudioBlockBenchmark {
            block_size,
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            warmup: 16,
        }
    }

    /// Sets the simulated device sample rate.
    pub fn sample_rate(mut self, sample_rate: u32) -> Self {
        assert!(sample_rate > 0, "sample_rate must be non-zero");
        self.sample_rate = sample_rate;
        self
    }

    /// Sets the channel count (interleaved).
    pub fn channels(mut self, channels: usize) -> Self {
        assert!(channels > 0, "channels must be non-zero");
        self.channels = channels;
        self
    }

    /// Sets how many untracked warm-up callbacks run before measurement.
    pub fn warmup(mut self, warmup: usize) -> Self {
        self.warmup = warmup;
        self
    }

    /// Runs the simulated callback loop; see [`benchmark_audio_block`].
    pub fn run<F>(self, iterations: usize, mut process: F) -> AudioBlockResult
    where
        F: FnMut(&mut [f32], AudioBlockContext),
    {
        assert!(iterations > 0, "iterations must be non-zero");

        // The callback's output buffer is handed to the processor up front,
        // exactly like a real audio driver; nothing inside the timed region
        // may allocate it.
        let mut buffer = vec![0.0f32; self.block_size.saturating_mul(self.channels)];

        let mut frame_index: u64 = 0;
        for _ in 0..self.warmup {
            process(
                &mut buffer,
                AudioBlockContext {
                    sample_rate: self.sample_rate,
                    channels: self.channels,
                    block_size: self.block_size,
                    frame_index,
                },
            );
            frame_index += 1;
        }

        let mut durations: Vec<Duration> = Vec::with_capacity(iterations);
        let mut allocations = AllocationStats::default();
        for _ in 0..iterations {
            let tracker = AllocationTracker::new(false);
            let start = Instant::now();
            process(
                &mut buffer,
                AudioBlockContext {
                    sample_rate: self.sample_rate,
                    channels: self.channels,
                    block_size: self.block_size,
                    frame_index,
                },
            );
            let elapsed = start.elapsed();
            let iteration_stats = tracker.allocation_stats();
            durations.push(elapsed);
            allocations = AllocationStats {
                allocations: allocations.allocations + iteration_stats.allocations,
                deallocations: allocations.deallocations + iteration_stats.deallocations,
                bytes_allocated: allocations.bytes_allocated + iteration_stats.bytes_allocated,
                bytes_deallocated: allocations.bytes_deallocated
                    + iteration_stats.bytes_deallocated,
            };
            frame_index += 1;
        }

        AudioBlockResult {
            block_size: self.block_size,
            sample_rate: self.sample_rate,
            channels: self.channels,
            iterations,
            stats: TimingStats::from_durations(durations),
            allocations,
        }
    }
}

/// Timing + heap results of one simulated-callback benchmark.
#[derive(Debug, Clone)]
pub struct AudioBlockResult {
    /// Frames per simulated callback.
    pub block_size: usize,
    /// Sample rate the callback was simulated at.
    pub sample_rate: u32,
    /// Channels per frame.
    pub channels: usize,
    /// Measured (post-warm-up) callback invocations.
    pub iterations: usize,
    /// Per-callback durations.
    pub stats: TimingStats,
    /// Heap traffic summed over every measured callback.
    pub allocations: AllocationStats,
}

impl AudioBlockResult {
    /// Wall-clock duration of a single block at the configured sample rate —
    /// the deadline each callback had to meet.
    pub fn block_duration(&self) -> Duration {
        block_duration(self.block_size, self.sample_rate)
    }

    /// Fraction of the block budget consumed by the worst callback
    /// (`max / block_duration`); anything >= 1.0 means dropouts.
    pub fn budget_usage(&self) -> f64 {
        self.stats.max().as_secs_f64() / self.block_duration().as_secs_f64()
    }

    /// Panics if any callback allocated. This is the CI `cargo bench` gate.
    pub fn assert_real_time_safe(&self) {
        let stats = self.allocations;
        assert_eq!(
            stats.allocations, 0,
            "real-time safety violation: {} heap allocation(s) across {} callbacks \
             of {} frames ({stats})",
            stats.allocations, self.iterations, self.block_size,
        );
    }

    /// Panics if the slowest callback overran the block duration.
    pub fn assert_within_block_budget(&self) {
        let budget = self.block_duration();
        let max = self.stats.max();
        assert!(
            max <= budget,
            "real-time budget exceeded: slowest callback took {max:?}, \
             block of {} frames at {} Hz allows {budget:?}",
            self.block_size,
            self.sample_rate,
        );
    }
}

impl fmt::Display for AudioBlockResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} frames @ {} Hz, {} channels, {} callbacks: min {:?} / mean {:?} / max {:?} \
             ({:.1}% of {:?} budget), {}",
            self.block_size,
            self.sample_rate,
            self.channels,
            self.iterations,
            self.stats.min(),
            self.stats.mean(),
            self.stats.max(),
            self.budget_usage() * 100.0,
            self.block_duration(),
            self.allocations,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silence(buffer: &mut [f32], _context: AudioBlockContext) {
        for sample in buffer.iter_mut() {
            *sample = 0.0;
        }
    }

    #[test]
    fn block_durations_match_the_math() {
        assert_eq!(
            block_duration(512, 48_000),
            Duration::from_nanos(10_666_667)
        );
        assert_eq!(
            block_duration(1024, 48_000),
            Duration::from_nanos(21_333_333)
        );
        assert_eq!(block_duration(512, 96_000), Duration::from_nanos(5_333_333));
    }

    #[test]
    #[should_panic]
    fn zero_block_size_is_rejected() {
        block_duration(0, 48_000);
    }

    #[test]
    fn benchmark_passes_for_an_allocation_free_callback() {
        let result = benchmark_audio_block(512, 8, silence);
        assert_eq!(result.iterations, 8);
        assert_eq!(result.stats.count(), 8);
        assert_eq!(result.allocations.allocations, 0);
        assert_eq!(result.allocations.bytes_allocated, 0);
        result.assert_real_time_safe();
        result.assert_within_block_budget();
        assert!(result.budget_usage() < 1.0);
    }

    #[test]
    fn callback_sees_context_and_buffer_shape() {
        let mut seen = Vec::new();
        let result = benchmark_audio_block(1024, 4, |buffer, context| {
            assert_eq!(buffer.len(), context.block_size * context.channels);
            seen.push(context.frame_index);
            for sample in buffer.iter_mut() {
                *sample = 1.0;
            }
        });
        assert_eq!(
            &seen[16..],
            &[16, 17, 18, 19],
            "warm-up runs 16 blocks first, measured callbacks continue the index"
        );
        assert_eq!(result.block_size, 1024);
        assert_eq!(result.channels, 2);
    }

    #[test]
    fn allocating_callbacks_are_flagged() {
        let result = benchmark_audio_block(512, 4, |_buffer, _context| {
            // A per-callback scratch Vec, the classic real-time sin.
            let scratch = Vec::<f32>::with_capacity(64);
            std::hint::black_box(&scratch);
        });
        assert!(result.allocations.allocations > 0);
        let outcome = std::panic::catch_unwind(|| result.assert_real_time_safe());
        assert!(outcome.is_err(), "allocating callback must fail the gate");
    }

    #[test]
    fn display_reports_the_run() {
        let result = benchmark_audio_block(512, 4, silence);
        let text = result.to_string();
        assert!(text.contains("512 frames @ 48000 Hz"), "{text}");
        assert!(text.contains("4 callbacks"), "{text}");
        assert!(text.contains("0 allocation(s)"), "{text}");
    }
}
