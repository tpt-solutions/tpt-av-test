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

- [x] `tpt-av-test-reference/src/ffmpeg.rs` — FFmpeg CLI subprocess wrapper (spawn, stream raw PCM stdout, never link)
- [x] `tpt-av-test-reference/src/audio.rs` — PCM bit-exact comparison w/ float epsilon (`assert_bit_exact_vs_ffmpeg`)
- [x] `tpt-av-test-reference/src/image.rs` — pixel comparison via PSNR/SSIM (`assert_image_similar`)
- [x] `tpt-av-test-reference/src/libsndfile.rs` — libsndfile/sox/ffprobe CLI wrapper
- [x] Source & commit official WAV test vectors
- [x] Source & commit official FLAC test suite
- [x] Source & commit official Opus test vectors
- [x] Source & commit ITU-T H.264 conformance streams
- [x] Source & commit reference images for compositor tests
- [x] Wire `cargo-deny` into CI, failing on GPL/LGPL/AGPL/MPL deps
- [x] Integration tests for `assert_bit_exact_vs_ffmpeg`
- [x] Integration tests for `assert_image_similar`

## Phase 2 — Fuzzing Infrastructure

- [x] `tpt-av-test-fuzz/src/parser.rs` — generic parser fuzzing macros (`fuzz_parser_never_panics!`)
- [x] `tpt-av-test-fuzz/src/crdt.rs` — CRDT commutativity/idempotency proptests (`assert_crdt_commutative`)
- [x] `tpt-av-test-fuzz/src/seed.rs` — deterministic seed management (reproducible failing seeds)
- [x] `tpt-av-test-fuzz/src/corpus/` — regression corpus of known-bad inputs, re-run every CI build
- [x] Example/integration tests demonstrating fuzz macros against a toy parser

## Phase 3 — Real-Time Benchmarking

- [x] `tpt-av-test-benchmark/src/allocation_tracker.rs` — custom global allocator tracking heap allocations
- [x] `tpt-av-test-benchmark/src/timing.rs` — microsecond-precision execution timing
- [x] `tpt-av-test-benchmark/src/audio_block.rs` — simulated audio callback benchmarking (512/1024 samples)
- [x] `tpt-av-test-benchmark/src/macros.rs` — `#[bench_real_time]` proc macro + `assert_real_time_safe!` macro
- [x] Wire `cargo bench` CI gate to fail on any detected allocation

## Phase 4 — Mock Hardware

- [x] `tpt-av-test-mock/src/midi_port.rs` — `MockMidiDevice` (virtual MIDI in/out, `inject`/`try_recv`)
- [x] `tpt-av-test-mock/src/audio_device.rs` — `MockAudioDevice` (sine wave generator, accepts buffers)
- [x] `tpt-av-test-mock/src/network.rs` — `MockNetworkTransport` (mock WebRTC/WebSocket transport)
- [x] `tpt-av-test-mock/src/filesystem.rs` — in-memory virtual filesystem

## Phase 5 — Ecosystem Rollout

- [x] Publish/tag `v0.1.0` (git tag created; crates.io publish pending publish credentials)
- [x] Wire `tpt-cadence` to use `reference::assert_bit_exact_vs_ffmpeg` + `fuzz::fuzz_parser` — `tpt-av-cadence-test-utils` delegates FFmpeg subprocess decode to `tpt-av-test-reference`; `tpt-av-cadence-wav` runs `fuzz_parser_never_panics!` (256 proptest cases) plus the shared regression corpus
- [x] Wire `tpt-audio` to use `benchmark::assert_real_time_safe` — `tpt-av-audio-core/tests/real_time_harness.rs` gates `AudioGraph::process` (gain→pan→fade) at zero allocations per 512-frame callback; the package opts out of the default `tracking-allocator` feature and installs `TrackingAllocator` explicitly because `rt_safety.rs` declares its own allocator
- [x] Wire `tpt-visual` to use `reference::assert_frame_exact` — the API shipped in `tpt-av-test-reference` (bit-exact RGB comparison with first-diff reporting); wired into `tpt-visual` via `tpt-av-visual-effects/tests/golden_frames.rs` (pinned git dev-dependency)
- [x] Wire `tpt-av-sync` to use `fuzz::proptest_crdt_commutative` — `tpt-av-sync-crdt/tests/harness_convergence.rs` proves `LwwReg` commutativity, idempotency, and tie-breaking via `tpt-av-test-fuzz`
- [x] Wire `tpt-av-control` to use `mock::MockMidiDevice` — `tpt-av-control-midi/tests/harness_mock_port.rs` drives `parse_midi1` through the virtual port (note round-trip, FIFO order, full gesture stream)

> The four wired repositories received additive changes only (dev-dependency
> entries + new test files / delegation). They are left uncommitted in each
> repo for review alongside that repo's in-flight work; tpt-av-sync's tree is
> otherwise fully untracked upstream.

## Phase 6 — Gap Fixes, Security Hardening, Adoption Tooling

- [x] Real `cargo-fuzz` target: `tpt-av-test-fuzz/fuzz/` sub-crate with `fuzz_targets/pcm_bytes_to_f32.rs` wrapping `tpt_av_test_reference::ffmpeg::pcm_bytes_to_f32`
- [x] Seed the new fuzz target's corpus from relevant existing `tpt-av-test-fuzz/corpus/*.bin` files
- [x] Fix `.github/workflows/ci.yml` `fuzz` job to actually run `cargo fuzz run pcm_bytes_to_f32` instead of the `if [ -d ... ]` no-op placeholder
- [x] Reconcile README/todo.md wording with the now-real fuzz job
- [x] Add `[advisories]` section to `deny.toml` (`vulnerability = "deny"`, `unmaintained = "warn"`)
- [x] Add a `cargo-audit` CI job as defense-in-depth alongside `cargo deny`
- [x] Pin third-party GitHub Actions (`actions/checkout`, `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `EmbarkStudios/cargo-deny-action`) to commit SHAs with version comments
- [x] Add a `lint` CI job: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] Add `rustfmt.toml` and `clippy.toml` (minimal, workspace defaults)
- [x] README: add CI/license badges + a Quickstart section with a copy-pasteable dev-dependency snippet and minimal usage example
- [x] Add `CONTRIBUTING.md` (short, issue-first: open an issue before a PR, list required local checks — fmt/clippy/test/deny)
- [x] Add `CHANGELOG.md` seeded with a `0.1.0` entry summarizing Phases 0-5
- [x] Add `templates/consumer-crate/` cargo-generate template wiring the five harness crates as dev-deps for new downstream repos
- [x] Document the `cargo generate --git <repo> templates/consumer-crate` flow in README
- [x] Add a `pages` CI job (push-to-`master`) publishing `target/criterion` benchmark HTML reports to GitHub Pages
- [x] Link the published benchmark dashboard URL from README
