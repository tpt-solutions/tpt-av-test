# tpt-av-test — Design Document

**Status:** Early-stage / Pre-1.0  
**License:** MIT OR Apache-2.0  
**Ecosystem:** [TPT Solutions Open Source](https://opensource.tptsolutions.co.nz/)

---

## 1. Vision & Philosophy

`tpt-av-test` is the quality assurance layer of the TPT AV Stack. Because we write custom audio/video parsers, CRDTs, and real-time engines from scratch to guarantee a pure-MIT codebase, we bear a large burden of proof: our code must be bit-exact against industry references, panic-free on adversarial input, and allocation-free on real-time paths.

Duplicating test infrastructure across a dozen repositories leads to drift and inconsistency. `tpt-av-test` centralizes the QA harness so every TPT repository depends on a single, battle-tested implementation for:

1. **Golden Master Testing** — Comparing pure-Rust output against industry-standard references (FFmpeg, libsndfile) without linking their GPL/LGPL code.
2. **Property-Based Fuzzing** — Feeding millions of malformed, random inputs to parsers to guarantee they never panic.
3. **Real-Time Safety Benchmarks** — Measuring heap allocations and maximum execution time per audio/video block to guarantee dropout-free performance.
4. **Mock Hardware** — Virtual audio devices, mock MIDI ports, and fake network transports for CI/CD testing.

### Core Tenets

1. **Zero Production Dependencies.** These crates are consumed exclusively via `[dev-dependencies]` and `cfg(test)`. They must never appear in a production binary's dependency graph.
2. **Subprocess Isolation.** When comparing against copyleft tools (FFmpeg, sox, ffprobe), we spawn them as CLI subprocesses. Their output is read through pipes; their code is never linked. This keeps the GPL/LGPL licenses from infecting our MIT code.
3. **Deterministic Fuzzing.** Every fuzz test is seedable and reproducible. A failing seed is saved to the regression corpus and re-run on every CI build.
4. **Strict Real-Time Assertions.** Benchmarks actively monitor the heap and fail if *any* allocation occurs inside a simulated audio callback.
5. **Pure MIT Licensing.** No GPL/LGPL/AGPL/MPL dependencies, enforced by `cargo-deny` in CI.

---

## 2. Ecosystem Integration

`tpt-av-test` is consumed as a `dev-dependency` by every other repository in the TPT AV Stack.

| Crate | Usage |
| :--- | :--- |
| `tpt-cadence` | `reference::assert_bit_exact_vs_ffmpeg` proves WAV/FLAC/Opus decoders match FFmpeg output; `fuzz::fuzz_parser` ensures no panics on corrupt files. |
| `tpt-audio` | `benchmark::assert_real_time_safe` proves the mixer graph allocates zero bytes during a 512-sample callback. |
| `tpt-visual` | `reference::assert_image_similar` compares GPU compositor output against reference images. |
| `tpt-av-sync` | `fuzz::assert_crdt_commutative` proves concurrent operations always resolve to the same state. |
| `tpt-av-control` | `mock::MockMidiDevice` tests MIDI parsing in CI without physical hardware. |

---

## 3. Repository Architecture

All sub-crates share the `tpt-av-test-` prefix.

```text
tpt-av-test/                        # GitHub Repository / Workspace Root
├── Cargo.toml                      # Workspace manifest (resolver "2")
├── deny.toml                       # cargo-deny license audit config
├── LICENSE-MIT
├── LICENSE-APACHE
├── README.md
├── DESIGN.md                       # This file
│
├── tpt-av-test-reference/          # Golden master comparison harness
│   ├── src/
│   │   ├── lib.rs
│   │   ├── ffmpeg.rs               # FFmpeg CLI subprocess wrapper
│   │   ├── libsndfile.rs           # libsndfile CLI wrapper (via sox/ffprobe)
│   │   ├── image.rs                # Pixel-exact image comparison (PSNR, SSIM)
│   │   └── audio.rs                # PCM bit-exact comparison (float epsilon)
│   ├── Cargo.toml
│   └── tests/
│
├── tpt-av-test-fuzz/               # Property-based fuzzing utilities
│   ├── src/
│   │   ├── lib.rs
│   │   ├── parser.rs               # Generic parser fuzzing macros
│   │   ├── crdt.rs                 # CRDT commutativity/idempotency proptests
│   │   ├── seed.rs                 # Deterministic seed management
│   │   └── corpus/                 # Known bad inputs (regression seeds)
│   ├── Cargo.toml
│   └── tests/
│
├── tpt-av-test-benchmark/          # Real-time safety and performance benchmarks
│   ├── src/
│   │   ├── lib.rs
│   │   ├── allocation_tracker.rs   # Custom allocator detecting heap allocations
│   │   ├── timing.rs               # Microsecond-precision execution timing
│   │   ├── audio_block.rs          # Simulated audio callback benchmarking
│   │   └── macros.rs               # `#[bench_real_time]` procedural macro
│   ├── Cargo.toml
│   └── tests/
│
├── tpt-av-test-mock/               # Mock hardware and network for CI
│   ├── src/
│   │   ├── lib.rs
│   │   ├── audio_device.rs         # Virtual audio I/O (sine wave generator)
│   │   ├── midi_port.rs            # Virtual MIDI input/output
│   │   ├── network.rs              # Mock WebRTC/WebSocket transport
│   │   └── filesystem.rs           # In-memory virtual filesystem
│   ├── Cargo.toml
│   └── tests/
│
└── tpt-av-test-vectors/            # Static test files (Git-LFS managed)
    ├── audio/
    │   ├── wav/                    # Official WAV test vectors
    │   ├── flac/                   # Official FLAC test suite
    │   └── opus/                   # Official Opus test vectors
    ├── video/
    │   └── h264/                   # ITU-T H.264 conformance streams
    └── images/
        └── reference/              # Golden master images for compositor tests
```

---

## 4. Core API Design

### 4.1 Golden Master Comparison (`tpt-av-test-reference`)

The most critical tool for proving our pure-Rust parsers are correct without linking GPL code.

```rust
/// Compares the output of a TPT decoder against FFmpeg (via CLI subprocess).
///
/// # Safety
/// This does NOT link against FFmpeg. It spawns `ffmpeg` as a subprocess,
/// ensuring the GPL license of FFmpeg does not infect the MIT-licensed TPT code.
pub fn assert_bit_exact_vs_ffmpeg(
    input_file: &Path,
    tpt_decoder_output: &[f32],
    sample_rate: u32,
    channels: u16,
    tolerance: f32, // 0.0 for integer formats, small epsilon (e.g., 1e-6) for float
) -> Result<(), ReferenceError> {
    // 1. Spawn `ffmpeg -i input_file -f f32le -ar {sample_rate} -ac {channels} pipe:1`
    // 2. Read FFmpeg's stdout into a buffer
    // 3. Compare buffer with `tpt_decoder_output`
    // 4. Assert that the difference is within `tolerance`
    todo!()
}

/// Compares two images for visual equivalence (used for tpt-visual).
pub fn assert_image_similar(
    generated: &image::DynamicImage,
    reference_path: &Path,
    max_psnr_diff: f64,
) -> Result<(), ReferenceError> {
    // 1. Load reference image
    // 2. Compute PSNR (Peak Signal-to-Noise Ratio) or SSIM
    // 3. Assert similarity
    todo!()
}
```

### 4.2 Property-Based Fuzzing (`tpt-av-test-fuzz`)

Guarantees that parsers never panic, even on malicious or corrupt input.

```rust
use proptest::prelude::*;
use tpt_av_test_fuzz::parser::fuzz_parser_never_panics;

// Example usage in tpt-cadence:
proptest! {
    #[test]
    fn wav_decoder_never_panics_on_random_input(
        data in proptest::collection::vec(any::<u8>(), 0..10000)
    ) {
        // This macro wraps the parser and asserts it returns Err, not panic.
        fuzz_parser_never_panics!(
            parser: WavDecoder::open,
            input: std::io::Cursor::new(data)
        );
    }
}

/// CRDT Commutativity Test
/// Proves that Operation A then B yields the same state as B then A.
pub fn assert_crdt_commutative<A, B, S>(
    initial_state: S,
    op_a: A,
    op_b: B,
    apply_fn: impl Fn(S, A) -> S,
) where
    S: PartialEq + std::fmt::Debug,
{
    let state_ab = apply_fn(apply_fn(initial_state.clone(), op_a.clone()), op_b.clone());
    let state_ba = apply_fn(apply_fn(initial_state.clone(), op_b), op_a);

    assert_eq!(state_ab, state_ba, "CRDT operations are not commutative!");
}
```

### 4.3 Real-Time Safety Benchmarking (`tpt-av-test-benchmark`)

Proves that audio/video processing threads never allocate memory.

```rust
/// A custom global allocator wrapper that tracks heap allocations.
pub struct AllocationTracker {
    inner: std::alloc::System,
    allocation_count: std::sync::atomic::AtomicUsize,
    panics_on_alloc: bool,
}

/// Macro to assert a block of code is real-time safe (zero allocations).
#[macro_export]
macro_rules! assert_real_time_safe {
    ($code:block) => {
        let mut tracker = tpt_av_test_benchmark::AllocationTracker::new(true);
        let result = std::panic::catch_unwind(|| {
            $code
        });

        let allocs = tracker.get_allocation_count();
        assert_eq!(allocs, 0, "Real-time safety violation: {} heap allocations detected!", allocs);

        result.expect("Code panicked during real-time execution");
    };
}
```

### 4.4 Mock Hardware (`tpt-av-test-mock`)

Enables CI testing without physical devices.

```rust
/// A virtual MIDI device that generates predictable test messages.
pub struct MockMidiDevice {
    tx: std::sync::mpsc::Sender<MidiMessage>,
    rx: std::sync::mpsc::Receiver<MidiMessage>,
}

impl MockMidiDevice {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx }
    }

    /// Injects a MIDI message into the virtual port.
    pub fn inject(&self, msg: MidiMessage) {
        self.tx.send(msg).unwrap();
    }
}

impl MidiInput for MockMidiDevice {
    fn try_recv(&mut self) -> Result<Option<MidiMessage>, MidiError> {
        Ok(self.rx.try_recv().ok())
    }
}
```

---

## 4.5 Test Vector Provenance

All files under `tpt-av-test-vectors/` are stored via Git LFS. Their sources are:

| Directory | Files | Provenance |
| :--- | :--- | :--- |
| `audio/wav/` | `sine-*.wav` (s16 mono/stereo, s24 stereo, f32 stereo, 44.1/48 kHz) | Encoded by the reference FFmpeg `9.0.1` builds from a deterministic `sine=frequency=440:duration=2` lavfi source. |
| `audio/flac/` | `sine-*-stereo.flac` (s16, s24) | Encoded by reference FFmpeg from the same deterministic sine source. |
| `audio/flac/` | `input-SCVA.flac`, `input-SCVAUP.flac`, `input-SVAUP.flac` | Official Xiph test files from `xiph/flac` at `test/flac-to-flac-metadata-test-files/`. |
| `audio/opus/` | `sine-440-48k-stereo-128k.opus` | Encoded by reference FFmpeg with `libopus` at 128 kbit/s from the deterministic sine source. |
| `audio/opus/` | `testvector01..03.{bit,dec,m.dec}` | Official Opus test vectors from RFC 8251; SHA-1 for each file matches the values published in the RFC. `dec` = 16-bit LE PCM reference output; `*.m.dec` = output without the optional CELT 180° phase shift. |
| `video/h264/` | `BA1_FT_C.264`, `CABA1_SVA_B.264`, `CABA1_Sony_D.jsv`, `CANL1_SVA_B.264`, `CANL1_Sony_E.jsv`, `CAMANL1_TOSHIBA_B.264` | Official ITU-T/JVT H.264 conformance bitstreams, mirrored by the FFmpeg FATE suite at `fate-suite.ffmpeg.org/h264-conformance/`. `.jsv` is JVT raw-stream format. |
| `images/reference/` | `smpte_bars_1080p.png`, `smooth_gradient_1080p.png`, `checkerboard_16px_1024.png` | Deterministic golden masters generated by `tpt-av-test-reference/examples/generate_reference_images.rs`; re-running reproduces byte-identical output. |

All synthetic audio and image generation is deterministic and reproducible; no RNG, timestamps, or host-specific metadata is included.

---

## 5. CI/CD Integration

`tpt-av-test` is the gatekeeper of TPT AV Stack quality. Every PR to any TPT repository must pass:

- **`cargo test`** — all unit and integration tests, including `assert_bit_exact_vs_ffmpeg` verification.
- **`cargo fuzz`** — property-based tests for 60 seconds on all parsers.
- **`cargo bench`** — real-time safety benchmarks; fails if any allocation is detected inside `#[bench_real_time]` blocks.
- **`cargo deny check`** — ensures no GPL/LGPL/AGPL/MPL dependencies sneak into `Cargo.lock`.

---

## 6. Roadmap

| Phase | Scope | Status |
| :--- | :--- | :--- |
| 0 | Repository & workspace scaffolding | In progress |
| 1 | Foundation & reference harness (`reference` crate, Git LFS vectors, cargo-deny CI) | Planned |
| 2 | Fuzzing infrastructure (`fuzz` proptest macros, regression corpus, CRDT harness) | Planned |
| 3 | Real-time benchmarking (`benchmark` AllocationTracker, `#[bench_real_time]`, audio blocks) | Planned |
| 4 | Mock hardware (`mock` MIDI, audio device, network, filesystem) | Planned |
| 5 | Ecosystem rollout (publish `v0.1.0`, wire `tpt-*` consumers) | Planned |

---

## 7. Dependency & Licensing Rules

### Allowed Dependencies (Permissive Only)

- `proptest` (MIT/Apache) — property-based testing
- `criterion` (MIT/Apache) — benchmarking
- `image` (MIT) — image comparison
- `tempfile` (MIT/Apache) — temporary files for subprocess tests
- `log` (MIT/Apache)

### Banned Dependencies

- `ffmpeg-sys` (LGPL/GPL) — we use the CLI binary, never the sys crate.
- Any crate that forces GPL, LGPL, AGPL, or MPL.

### Enforcement

`deny.toml` pins the allow/deny license lists; `cargo-deny` runs in CI on every build.

---

## 8. Contributing

Contributions to `tpt-av-test` must:

- Be licensed under the same MIT OR Apache-2.0 terms.
- Never introduce a dependency that is GPL, LGPL, AGPL, or MPL licensed.
- Use subprocess invocation for all reference comparisons — never direct linking.
- Keep all fuzz tests deterministic and reproducible.