//! Microsecond-precision execution timing for real-time paths.
//!
//! Wall-clock helpers used by the audio-block benchmark and the
//! `#[bench_real_time]` attribute: measure how long a call takes, aggregate
//! many samples into statistics, assert a maximum per-call duration, and
//! enforce a running real-time budget. Durations are read from
//! [`std::time::Instant`], which is monotonic and (on all supported hosts)
//! nanosecond-resolution.
//!
//! ```rust
//! use core::time::Duration;
//! use tpt_av_test_benchmark::timing::{assert_max_duration, measure};
//!
//! let (_result, elapsed) = measure(|| 2 + 2);
//! assert!(elapsed < Duration::from_secs(1));
//!
//! // Panics if the call takes longer than the given budget.
//! let answer = assert_max_duration(Duration::from_millis(10), || 40 + 2);
//! assert_eq!(answer, 42);
//! ```

use core::fmt;
use core::time::Duration;
use std::time::Instant;

/// Parses a duration written the way humans write real-time budgets:
/// `"10ms"`, `"512us"`, `"21.33ms"`, `"2s"`.
///
/// Accepted units: `ns`, `us` / `µs`, `ms`, `s`. The unit is required.
///
/// ```rust
/// use core::time::Duration;
/// use tpt_av_test_benchmark::timing::parse_duration;
///
/// assert_eq!(parse_duration("10ms"), Ok(Duration::from_millis(10)));
/// assert_eq!(parse_duration("512us"), Ok(Duration::from_nanos(512_000)));
/// assert_eq!(parse_duration("0.25s"), Ok(Duration::from_millis(250)));
/// assert!(parse_duration("10").is_err(), "a unit is required");
/// assert!(parse_duration("10 fortnights").is_err());
/// ```
pub fn parse_duration(text: &str) -> Result<Duration, ParseDurationError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ParseDurationError::Empty);
    }

    // Longest suffix first so "ms" is not eaten by "s".
    let (unit, factor_secs): (&str, f64) = if let Some(value) = trimmed.strip_suffix("ms") {
        (value, 1e-3)
    } else if let Some(value) = trimmed.strip_suffix("µs") {
        (value, 1e-6)
    } else if let Some(value) = trimmed.strip_suffix("us") {
        (value, 1e-6)
    } else if let Some(value) = trimmed.strip_suffix("ns") {
        (value, 1e-9)
    } else if let Some(value) = trimmed.strip_suffix('s') {
        (value, 1.0)
    } else {
        return Err(ParseDurationError::MissingUnit {
            text: trimmed.to_string(),
        });
    };

    let unit_trimmed = unit.trim_end();
    if unit_trimmed.is_empty() {
        return Err(ParseDurationError::MissingNumber {
            text: trimmed.to_string(),
        });
    }
    let value: f64 = unit_trimmed
        .parse()
        .map_err(|_| ParseDurationError::InvalidNumber {
            text: unit_trimmed.to_string(),
        })?;
    if !value.is_finite() || value < 0.0 {
        return Err(ParseDurationError::InvalidNumber {
            text: unit_trimmed.to_string(),
        });
    }

    let secs = value * factor_secs;
    if secs > Duration::MAX.as_secs_f64() {
        return Err(ParseDurationError::OutOfRange {
            text: trimmed.to_string(),
        });
    }
    Ok(Duration::from_secs_f64(secs))
}

/// Why a duration string could not be parsed by [`parse_duration`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseDurationError {
    /// The input was empty (or only whitespace).
    Empty,
    /// A number was given without a time unit.
    MissingUnit { text: String },
    /// A unit was given without a number.
    MissingNumber { text: String },
    /// The numeric part is not a valid non-negative number.
    InvalidNumber { text: String },
    /// The value overflows [`Duration::MAX`].
    OutOfRange { text: String },
}

impl fmt::Display for ParseDurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseDurationError::Empty => write!(f, "empty duration string"),
            ParseDurationError::MissingUnit { text } => {
                write!(
                    f,
                    "duration `{text}` has no time unit (expected ns, us, ms, or s)"
                )
            }
            ParseDurationError::MissingNumber { text } => {
                write!(f, "duration `{text}` has no number")
            }
            ParseDurationError::InvalidNumber { text } => {
                write!(f, "duration `{text}` is not a valid non-negative number")
            }
            ParseDurationError::OutOfRange { text } => {
                write!(f, "duration `{text}` overflows `Duration::MAX`")
            }
        }
    }
}

impl std::error::Error for ParseDurationError {}

/// Runs `f` once and returns its result together with the elapsed time.
pub fn measure<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    (result, start.elapsed())
}

/// Runs `f` `samples` times, discards the results, and collects the per-call
/// durations into [`TimingStats`].
///
/// ```rust
/// use core::time::Duration;
/// use tpt_av_test_benchmark::timing::measure_samples;
///
/// let stats = measure_samples(8, || {
///     std::hint::black_box([0.0f32; 512]);
/// });
/// assert_eq!(stats.count(), 8);
/// assert!(stats.max() < Duration::from_secs(1));
/// ```
pub fn measure_samples<F, R>(samples: usize, mut f: F) -> TimingStats
where
    F: FnMut() -> R,
{
    assert!(samples > 0, "measure_samples needs at least one sample");
    let mut durations = Vec::with_capacity(samples);
    for _ in 0..samples {
        let (_result, elapsed) = measure(&mut f);
        durations.push(elapsed);
    }
    TimingStats { durations }
}

/// Per-call durations collected by [`measure_samples`], with the aggregates a
/// real-time report needs: the *max* (the sample that would cause a dropout)
/// above all.
#[derive(Debug, Clone)]
pub struct TimingStats {
    durations: Vec<Duration>,
}

impl TimingStats {
    /// Builds statistics from durations collected by the caller, e.g. when
    /// instrumenting a loop that [`measure_samples`] cannot express.
    pub fn from_durations(durations: Vec<Duration>) -> Self {
        assert!(
            !durations.is_empty(),
            "TimingStats needs at least one sample"
        );
        TimingStats { durations }
    }

    /// Number of samples.
    pub fn count(&self) -> usize {
        self.durations.len()
    }

    /// Individual per-call durations, in collection order.
    pub fn samples(&self) -> &[Duration] {
        &self.durations
    }

    /// Fastest call.
    pub fn min(&self) -> Duration {
        *self
            .durations
            .iter()
            .min()
            .expect("TimingStats always holds at least one sample")
    }

    /// Slowest call — the one that would cause an audible dropout.
    pub fn max(&self) -> Duration {
        *self
            .durations
            .iter()
            .max()
            .expect("TimingStats always holds at least one sample")
    }

    /// Arithmetic mean.
    pub fn mean(&self) -> Duration {
        let total_nanos: u128 = self
            .durations
            .iter()
            .map(|duration| duration.as_nanos())
            .sum();
        Duration::from_nanos((total_nanos / self.durations.len() as u128) as u64)
    }

    /// Median (lower middle sample for even counts).
    pub fn median(&self) -> Duration {
        self.percentile(0.5)
    }

    /// The `fraction` percentile (`0.0 ..= 1.0`), e.g. `percentile(0.99)`.
    /// Rounds up: the returned sample is at least `fraction * count` samples
    /// are at or below it.
    pub fn percentile(&self, fraction: f64) -> Duration {
        assert!(
            (0.0..=1.0).contains(&fraction),
            "percentile fraction must be within 0.0..=1.0"
        );
        let mut sorted = self.durations.clone();
        sorted.sort();
        let count = sorted.len();
        let index = ((fraction * count as f64).ceil() as usize)
            .saturating_sub(1)
            .min(count - 1);
        sorted[index]
    }

    /// Sum of all samples.
    pub fn total(&self) -> Duration {
        self.durations.iter().sum()
    }
}

/// Runs `f` once and panics if it takes longer than `max`.
///
/// The budget typically comes from [`crate::audio_block::block_duration`] —
/// a 512-sample callback at 48 kHz must finish in ~10.67 ms or the stream
/// drops out.
pub fn assert_max_duration<F, R>(max: Duration, f: F) -> R
where
    F: FnOnce() -> R,
{
    let (result, elapsed) = measure(f);
    assert!(
        elapsed <= max,
        "real-time budget exceeded: {elapsed:?} elapsed > {max:?} budget",
    );
    result
}

/// A running real-time budget: created with the time available, checked as
/// work progresses.
///
/// ```rust
/// use core::time::Duration;
/// use tpt_av_test_benchmark::timing::RealtimeBudget;
///
/// let budget = RealtimeBudget::new(Duration::from_millis(10));
/// assert!(!budget.exhausted());
/// assert!(budget.remaining() <= Duration::from_millis(10));
/// budget.assert_within(); // has not run long enough to exceed the budget
/// ```
pub struct RealtimeBudget {
    start: Instant,
    budget: Duration,
}

impl RealtimeBudget {
    /// Starts a budget of `budget` starting now.
    pub fn new(budget: Duration) -> Self {
        RealtimeBudget {
            start: Instant::now(),
            budget,
        }
    }

    /// Time elapsed since the budget started.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Time still available; zero once the budget is exhausted.
    pub fn remaining(&self) -> Duration {
        self.budget
            .checked_sub(self.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Whether the budget has been exceeded.
    pub fn exhausted(&self) -> bool {
        self.elapsed() > self.budget
    }

    /// `Ok(())` while the budget holds, `Err(elapsed)` once exceeded.
    pub fn check(&self) -> Result<(), Duration> {
        if self.exhausted() {
            Err(self.elapsed())
        } else {
            Ok(())
        }
    }

    /// Panics if the budget has been exceeded.
    pub fn assert_within(&self) {
        if let Err(elapsed) = self.check() {
            panic!(
                "real-time budget exceeded: {elapsed:?} elapsed > {:?} budget",
                self.budget
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_accepts_common_units() {
        assert_eq!(parse_duration("10ms"), Ok(Duration::from_millis(10)));
        assert_eq!(parse_duration("512us"), Ok(Duration::from_nanos(512_000)));
        assert_eq!(parse_duration("512µs"), Ok(Duration::from_nanos(512_000)));
        assert_eq!(parse_duration("100ns"), Ok(Duration::from_nanos(100)));
        assert_eq!(parse_duration("2s"), Ok(Duration::from_secs(2)));
        assert_eq!(
            parse_duration("21.33ms"),
            Ok(Duration::from_nanos(21_330_000))
        );
        assert_eq!(parse_duration(" 10ms "), Ok(Duration::from_millis(10)));
    }

    #[test]
    fn parse_duration_rejects_garbage() {
        assert_eq!(parse_duration(""), Err(ParseDurationError::Empty));
        assert_eq!(parse_duration("   "), Err(ParseDurationError::Empty));
        assert_eq!(
            parse_duration("10"),
            Err(ParseDurationError::MissingUnit { text: "10".into() })
        );
        assert_eq!(
            parse_duration("ms"),
            Err(ParseDurationError::MissingNumber { text: "ms".into() })
        );
        assert_eq!(
            parse_duration("ten ms"),
            Err(ParseDurationError::InvalidNumber { text: "ten".into() })
        );
        assert_eq!(
            parse_duration("-5ms"),
            Err(ParseDurationError::InvalidNumber { text: "-5".into() })
        );
        assert_eq!(
            parse_duration("10 fortnights"),
            Err(ParseDurationError::InvalidNumber {
                text: "10 fortnight".into()
            })
        );
        assert_eq!(
            parse_duration("10x"),
            Err(ParseDurationError::MissingUnit { text: "10x".into() })
        );
    }

    #[test]
    fn parse_duration_ms_wins_over_s_suffix() {
        // "ms" must not be parsed as "m" + "s" style tricks.
        assert_eq!(parse_duration("1500ms"), Ok(Duration::from_millis(1500)));
    }

    #[test]
    fn measure_reports_sane_elapsed_time() {
        let (_value, elapsed) = measure(|| std::thread::sleep(Duration::from_millis(2)));
        assert!(elapsed >= Duration::from_millis(2), "elapsed = {elapsed:?}");
    }

    #[test]
    fn timing_stats_aggregates() {
        let durations = [1, 2, 3, 4, 5].map(Duration::from_micros);
        let stats = TimingStats {
            durations: durations.to_vec(),
        };
        assert_eq!(stats.count(), 5);
        assert_eq!(stats.min(), Duration::from_micros(1));
        assert_eq!(stats.max(), Duration::from_micros(5));
        assert_eq!(stats.mean(), Duration::from_micros(3));
        assert_eq!(stats.median(), Duration::from_micros(3));
        assert_eq!(stats.total(), Duration::from_micros(15));
        assert_eq!(stats.percentile(0.0), Duration::from_micros(1));
        assert_eq!(stats.percentile(1.0), Duration::from_micros(5));
        assert_eq!(stats.percentile(0.99), Duration::from_micros(5));
    }

    #[test]
    fn measure_samples_collects_every_sample() {
        let stats = measure_samples(4, || std::hint::black_box(7u32));
        assert_eq!(stats.count(), 4);
        assert!(stats.max() < Duration::from_secs(1));
    }

    #[test]
    fn assert_max_duration_passes_fast_calls() {
        let result = assert_max_duration(Duration::from_secs(1), || vec![1u8; 8].len());
        assert_eq!(result, 8);
    }

    #[test]
    #[should_panic(expected = "real-time budget exceeded")]
    fn assert_max_duration_panics_on_slow_calls() {
        assert_max_duration(Duration::from_nanos(1), || {
            std::thread::sleep(Duration::from_millis(5));
        });
    }

    #[test]
    fn realtime_budget_tracks_remaining_time() {
        let budget = RealtimeBudget::new(Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(2));
        assert!(!budget.exhausted());
        assert!(budget.check().is_ok());
        assert!(budget.remaining() <= Duration::from_millis(3));

        let exhausted = RealtimeBudget::new(Duration::from_nanos(1));
        std::thread::sleep(Duration::from_millis(2));
        assert!(exhausted.exhausted());
        assert_eq!(exhausted.remaining(), Duration::ZERO);
        assert!(exhausted.check().is_err());
    }

    #[test]
    #[should_panic(expected = "real-time budget exceeded")]
    fn realtime_budget_assert_panics_when_exhausted() {
        let budget = RealtimeBudget::new(Duration::from_nanos(1));
        std::thread::sleep(Duration::from_millis(2));
        budget.assert_within();
    }
}
