//! Real-time safety and performance benchmarking for the TPT AV Stack.
//!
//! Proven hunters: custom global allocator that counts (and can panic on) heap
//! allocations, microsecond-precision execution timing, simulated audio
//! callbacks, and the `assert_real_time_safe!` macro that fails a test the
//! moment a real-time path allocates.
//!
//! This crate is intended for use via `[dev-dependencies]`, `cfg(test)`, and
//! dedicated `benches/` targets only.
//!
//! ## Modules (Phase 3)
//!
//! - [`allocation_tracker`] — custom global allocator tracking heap allocations.
//! - [`timing`] — microsecond-precision execution timing.
//! - [`audio_block`] — simulated audio callback benchmarking (512/1024 samples).
//! - [`macros`] — `#[bench_real_time]` proc macro + `assert_real_time_safe!`.

/// Placeholder version marker; replaced by full module wiring in Phase 3.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");