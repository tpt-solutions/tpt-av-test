# Changelog

All notable changes to `tpt-av-test-fuzz` are documented in this file.

## [0.1.0] — 2026-09-21

Initial release.

- Generic parser-fuzzing macro (`fuzz_parser_never_panics!`).
- CRDT commutativity/idempotency proptests (`crdt::assert_crdt_commutative`).
- Deterministic seed management for reproducible failing cases (`seed`).
- Regression corpus of known-bad inputs, re-run on every CI build (`corpus`).
- Real `cargo-fuzz` target (`fuzz/fuzz_targets/pcm_bytes_to_f32.rs`) wrapping
  `tpt_av_test_reference::ffmpeg::pcm_bytes_to_f32`, seeded from the
  regression corpus.
