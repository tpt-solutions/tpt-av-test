# tpt-av-test — Project Todo

Centralized conformance, fuzzing, and benchmarking harness for the TPT AV Stack (TPT Solutions). Dual-licensed MIT OR Apache-2.0.

---

## Phase 0 — Repository & Workspace Scaffolding

- [x] `git init` + Rust `.gitignore`
- [x] Workspace `Cargo.toml` (resolver "2", `[workspace.package]`: version 0.1.0, edition 2021, license "MIT OR Apache-2.0", repository URL, rust-version 1.75; `[workspace.dependencies]`)
- [x] `LICENSE-MIT`
- [x] `LICENSE-APACHE`
- [x] `README.md` (vision & philosophy summary)
- [x] `DESIGN.md` (full design doc, from spec)
- [x] `deny.toml` (allow/deny license lists, MIT + Apache-2.0 wording)
- [x] Create `tpt-av-test-reference/` crate skeleton (`Cargo.toml` + `src/lib.rs`)
- [x] Create `tpt-av-test-fuzz/` crate skeleton (`Cargo.toml` + `src/lib.rs`)
- [x] Create `tpt-av-test-benchmark/` crate skeleton (`Cargo.toml` + `src/lib.rs`)
- [x] Create `tpt-av-test-mock/` crate skeleton (`Cargo.toml` + `src/lib.rs`)
- [x] Create `tpt-av-test-vectors/` directory tree (`audio/{wav,flac,opus}`, `video/h264`, `images/reference`)
- [x] Set up Git LFS tracking for `tpt-av-test-vectors/`
- [x] GitHub Actions CI workflow (`.github/workflows/ci.yml`): `cargo test`, `cargo fuzz`, `cargo bench`, `cargo deny check`
- [x] Initial commit

## Phase 1 — Foundation & Reference Harness

- [ ] `tpt-av-test-reference/src/ffmpeg.rs` — FFmpeg CLI subprocess wrapper (spawn, stream raw PCM stdout, never link)
- [ ] `tpt-av-test-reference/src/audio.rs` — PCM bit-exact comparison w/ float epsilon (`assert_bit_exact_vs_ffmpeg`)
- [ ] `tpt-av-test-reference/src/image.rs` — pixel comparison via PSNR/SSIM (`assert_image_similar`)
- [ ] `tpt-av-test-reference/src/libsndfile.rs` — libsndfile/sox/ffprobe CLI wrapper
- [ ] Source & commit official WAV test vectors
- [ ] Source & commit official FLAC test suite
- [ ] Source & commit official Opus test vectors
- [ ] Source & commit ITU-T H.264 conformance streams
- [ ] Source & commit reference images for compositor tests
- [ ] Wire `cargo-deny` into CI, failing on GPL/LGPL/AGPL/MPL deps
- [ ] Integration tests for `assert_bit_exact_vs_ffmpeg`
- [ ] Integration tests for `assert_image_similar`

## Phase 2 — Fuzzing Infrastructure

- [ ] `tpt-av-test-fuzz/src/parser.rs` — generic parser fuzzing macros (`fuzz_parser_never_panics!`)
- [ ] `tpt-av-test-fuzz/src/crdt.rs` — CRDT commutativity/idempotency proptests (`assert_crdt_commutative`)
- [ ] `tpt-av-test-fuzz/src/seed.rs` — deterministic seed management (reproducible failing seeds)
- [ ] `tpt-av-test-fuzz/src/corpus/` — regression corpus of known-bad inputs, re-run every CI build
- [ ] Example/integration tests demonstrating fuzz macros against a toy parser

## Phase 3 — Real-Time Benchmarking

- [ ] `tpt-av-test-benchmark/src/allocation_tracker.rs` — custom global allocator tracking heap allocations
- [ ] `tpt-av-test-benchmark/src/timing.rs` — microsecond-precision execution timing
- [ ] `tpt-av-test-benchmark/src/audio_block.rs` — simulated audio callback benchmarking (512/1024 samples)
- [ ] `tpt-av-test-benchmark/src/macros.rs` — `#[bench_real_time]` proc macro + `assert_real_time_safe!` macro
- [ ] Wire `cargo bench` CI gate to fail on any detected allocation

## Phase 4 — Mock Hardware

- [ ] `tpt-av-test-mock/src/midi_port.rs` — `MockMidiDevice` (virtual MIDI in/out, `inject`/`try_recv`)
- [ ] `tpt-av-test-mock/src/audio_device.rs` — `MockAudioDevice` (sine wave generator, accepts buffers)
- [ ] `tpt-av-test-mock/src/network.rs` — `MockNetworkTransport` (mock WebRTC/WebSocket transport)
- [ ] `tpt-av-test-mock/src/filesystem.rs` — in-memory virtual filesystem

## Phase 5 — Ecosystem Rollout

- [ ] Publish/tag `v0.1.0`
- [ ] Wire `tpt-cadence` to use `reference::assert_bit_exact_vs_ffmpeg` + `fuzz::fuzz_parser`
- [ ] Wire `tpt-audio` to use `benchmark::assert_real_time_safe`
- [ ] Wire `tpt-visual` to use `reference::assert_frame_exact`
- [ ] Wire `tpt-av-sync` to use `fuzz::proptest_crdt_commutative`
- [ ] Wire `tpt-av-control` to use `mock::MockMidiDevice`
