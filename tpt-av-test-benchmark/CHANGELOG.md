# Changelog

All notable changes to `tpt-av-test-benchmark` are documented in this file.

## [0.1.0] — 2026-09-21

Initial release.

- Custom global allocator tracking heap allocations (`allocation_tracker::TrackingAllocator`),
  installed by default via the `tracking-allocator` feature.
- Microsecond-precision execution timing (`timing`).
- Simulated audio callback benchmarking at 512/1024-sample block sizes (`audio_block`).
- `assert_real_time_safe!` macro and `#[bench_real_time]` attribute (re-exported
  from `tpt-av-test-macros`) gating real-time paths on zero allocations.
- `cargo bench` CI gate wired to fail on any detected allocation.
