# tpt-av-test

**A pure-Rust, rigorous conformance, fuzzing, and benchmarking harness for the TPT AV Stack.**

`tpt-av-test` is the quality assurance backbone of the [TPT AV Stack](https://opensource.tptsolutions.co.nz/). Because every audio/video parser, CRDT, and real-time engine is written from scratch to guarantee a pure MIT license, we have a massive burden of proof: our code must be **bit-exact**, **panic-free**, and **performant**. This workspace provides centralized, reusable infrastructure that every other TPT repository depends on as a `dev-dependency`.

## Vision & Philosophy

1. **Zero Production Dependencies** — This is strictly `dev-dependencies` / `cfg(test)` material. It must never bloat a production binary.
2. **Subprocess Isolation** — When comparing against industry-standard, copyleft tools (FFmpeg, libsndfile), we invoke them via CLI subprocesses. Their licenses never infect our MIT codebase.
3. **Deterministic Fuzzing** — Every fuzz test is reproducible. Failing seeds are saved and re-run on every CI build.
4. **Strict Real-Time Assertions** — Benchmarks fail if *any* heap allocation occurs during a simulated audio callback.
5. **Pure MIT Licensing** — No GPL/LGPL/AGPL/MPL dependencies, enforced by `cargo-deny` in CI.

## Workspace Layout

| Crate | Purpose |
| :--- | :--- |
| `tpt-av-test-reference` | Golden-master harness: compares pure-Rust decoder/compositor output against FFmpeg & libsndfile via subprocess. |
| `tpt-av-test-fuzz` | Property-based fuzzing macros (parser never panics, CRDT commutativity/idempotency) with deterministic seeds. |
| `tpt-av-test-benchmark` | Real-time safety: custom global allocator that flags heap allocations inside audio callbacks, µs-precision timing, and the `assert_real_time_safe!` / `#[bench_real_time]` gates. |
| `tpt-av-test-macros` | Procedural macros backing the benchmark harness (`#[bench_real_time]`). |
| `tpt-av-test-mock` | Virtual MIDI ports, audio devices, network transports, and in-memory filesystems for CI. |
| `tpt-av-test-vectors` | Static, Git-LFS-managed official test files (WAV, FLAC, Opus, H.264, reference images). |

## Ecosystem Integration

- **`tpt-cadence`** uses `reference::assert_bit_exact_vs_ffmpeg` to prove WAV/FLAC/Opus decoders match FFmpeg output, and `fuzz::fuzz_parser` to ensure no panics on corrupt files.
- **`tpt-audio`** uses `benchmark::assert_real_time_safe` to prove the mixer graph allocates zero bytes per callback.
- **`tpt-visual`** uses `reference::assert_image_similar` to compare compositor output against reference images.
- **`tpt-av-sync`** uses `fuzz::assert_crdt_commutative` to mathematically prove concurrent operations converge.
- **`tpt-av-control`** uses `mock::MockMidiDevice` to test MIDI parsing in CI without physical hardware.

## CI Gate

Every PR to any TPT repository must pass:

- `cargo test` — all unit and integration tests.
- `cargo fuzz` — 60 seconds of property-based fuzzing on every parser.
- `cargo bench` — real-time safety benchmarks; fails on any detected allocation.
- `cargo deny check` — no copyleft dependencies in the lockfile.

See [DESIGN.md](DESIGN.md) for the full design documentation.

## License

Dual-licensed under either of

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option. Contributions must be submitted under the same terms, and must never introduce a GPL, LGPL, AGPL, or MPL dependency. All reference comparisons use subprocess invocation — never direct linking. All fuzz tests are deterministic and reproducible.