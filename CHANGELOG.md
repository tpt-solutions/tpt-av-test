# Changelog

All notable changes to this project are documented in this file.

## [0.1.0] — 2026-09-21

Initial release. Five workspace crates providing centralized conformance,
fuzzing, and benchmarking infrastructure for the TPT AV Stack:

- **`tpt-av-test-reference`** — FFmpeg/libsndfile subprocess wrapper;
  bit-exact PCM comparison (`assert_bit_exact_vs_ffmpeg`), pixel-exact frame
  comparison (`assert_frame_exact`), and PSNR/SSIM image comparison
  (`assert_image_similar`). Ships with official WAV, FLAC, Opus, and ITU-T
  H.264 conformance vectors plus reference compositor images, tracked via
  Git LFS.
- **`tpt-av-test-fuzz`** — generic parser-fuzzing macros
  (`fuzz_parser_never_panics!`), CRDT commutativity/idempotency proptests
  (`assert_crdt_commutative`), deterministic seed management, and a
  regression corpus of known-bad inputs re-run on every CI build.
- **`tpt-av-test-benchmark`** — a custom global allocator that flags heap
  allocations inside simulated audio callbacks, microsecond-precision
  timing, and the `assert_real_time_safe!` / `#[bench_real_time]` gates
  wired into CI to fail on any detected allocation.
- **`tpt-av-test-mock`** — `MockMidiDevice`, `MockAudioDevice`,
  `MockNetworkTransport`, and an in-memory virtual filesystem, so downstream
  CI never needs physical hardware or a network.
- **`tpt-av-test-macros`** — procedural macros backing the benchmark
  harness.

Wired as a `dev-dependency` into `tpt-cadence`, `tpt-audio`, `tpt-av-sync`,
and `tpt-av-control`; `tpt-visual` wiring is tracked separately pending its
GPU toolchain setup.

CI enforces `cargo test`, `cargo bench` (real-time safety gate), and
`cargo deny check` (pure MIT OR Apache-2.0 dependency graph, no
GPL/LGPL/AGPL/MPL) on every PR.
