# tpt-av-test-fuzz

Deterministic property-based fuzzing harness for the [TPT AV Stack](https://opensource.tptsolutions.co.nz/).

Guarantees that every parser in the ecosystem **never panics**, even on
malicious or corrupt input, and that CRDTs converge regardless of operation
ordering. Every failure is reproducible: the failing seed is captured and
re-run on every CI build.

This crate is `[dev-dependencies]` / `cfg(test)` only.

## Modules

- `parser` — `fuzz_parser_never_panics!` generic parser fuzzing macro.
- `crdt` — CRDT commutativity/idempotency proptests (`assert_crdt_commutative`).
- `seed` — deterministic seed management for reproducible failures.
- `corpus` — regression corpus of known-bad inputs, re-run every CI build.

A real `cargo-fuzz` target wrapping `tpt-av-test-reference`'s PCM decoding
lives under `fuzz/`.

## Usage

```toml
[dev-dependencies]
tpt-av-test-fuzz = { git = "https://github.com/tpt-solutions/tpt-av-test" }
```

```rust,ignore
use tpt_av_test_fuzz::fuzz_parser_never_panics;

fuzz_parser_never_panics!(my_parser_never_panics, |data: &[u8]| {
    let _ = my_crate::parse(data);
});
```

## License

Dual-licensed under [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE), at your option.

See the [workspace README](../README.md) for the full project vision and layout.
