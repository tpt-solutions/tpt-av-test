//! Real-time safety and performance benchmarking for the TPT AV Stack.
//!
//! Proven hunters: a custom global allocator that counts heap allocations,
//! microsecond-precision execution timing, simulated audio callbacks at the
//! standard 512/1024-frame block sizes, and the [`assert_real_time_safe!`]
//! macro plus [`bench_real_time`](crate::macros::bench_real_time) attribute
//! that fail a test the moment a real-time path allocates.
//!
//! This crate is intended for use via `[dev-dependencies]`, `cfg(test)`, and
//! dedicated `benches/` targets only.
//!
//! ```rust
//! use tpt_av_test_benchmark::{assert_real_time_safe, AllocationTracker};
//!
//! // Zero heap traffic on a real-time path, or the test fails:
//! assert_real_time_safe!({
//!     let mut buffer = [0.0f32; 512];
//!     for sample in &mut buffer {
//!         *sample = *sample * 0.5 + 0.25;
//!     }
//! });
//!
//! // Or measure first, assert later:
//! let tracker = AllocationTracker::new(false);
//! let (_result, elapsed) =
//!     tpt_av_test_benchmark::timing::measure(|| std::hint::black_box(1 + 1));
//! assert_eq!(tracker.get_allocation_count(), 0, "measurement must not allocate");
//! ```
//!
//! ## Modules
//!
//! - [`allocation_tracker`] — custom global allocator tracking heap allocations.
//! - [`timing`] — microsecond-precision execution timing.
//! - [`audio_block`] — simulated audio callback benchmarking (512/1024 samples).
//! - [`macros`] — `#[bench_real_time]` proc macro + `assert_real_time_safe!`.
//!
//! # Global allocator
//!
//! By default this crate installs [`allocation_tracker::TrackingAllocator`]
//! as the process `#[global_allocator]` (`tracking-allocator` feature), so
//! simply depending on it (as a dev-dependency) makes every allocation
//! observable. Binaries that need their own allocator build the crate with
//! `default-features = false` and declare `TrackingAllocator` themselves —
//! there can only be one `#[global_allocator]` per binary.

pub mod allocation_tracker;
pub mod audio_block;
pub mod macros;
pub mod timing;

pub use allocation_tracker::{AllocationStats, AllocationTracker, TrackingAllocator};
pub use tpt_av_test_macros::bench_real_time;
