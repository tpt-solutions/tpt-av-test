//! End-to-end demonstration of the fuzzing macros: a toy WAV parser is
//! hammered with random bytes (it must never panic) and a CRDT is checked for
//! batch commutativity, all under a pinned deterministic seed.

use std::fmt::Debug;

use proptest::prelude::*;
use tpt_av_test_fuzz::crdt::{apply_register, apply_register_batch, RegisterOp, Registers};
use tpt_av_test_fuzz::fuzz_parser_never_panics;
use tpt_av_test_fuzz::seed::{determinism_config, Seed};

#[derive(Debug, PartialEq, Eq)]
struct ToyWav {
    channels: u16,
    rate: u32,
    bits: u16,
}

/// A deliberately naive WAV parser. Every slice access is bounds-checked; the
/// whole point of the macro is that no combination of input bytes can panic
/// it.
fn toy_wav_decode(data: &[u8]) -> Result<ToyWav, String> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE stream".to_string());
    }
    let mut pos = 12usize;
    let mut channels = 0u16;
    let mut rate = 0u32;
    let mut bits = 0u16;
    while pos + 8 <= data.len() {
        let size = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
        let body_end = pos
            .checked_add(8 + size as usize)
            .ok_or_else(|| "chunk size overflow".to_string())?;
        if body_end > data.len() {
            return Err("chunk overruns file".to_string());
        }
        if &data[pos..pos + 4] == b"fmt " && size >= 16 {
            channels = u16::from_le_bytes([data[pos + 8], data[pos + 9]]);
            rate = u32::from_le_bytes([
                data[pos + 12],
                data[pos + 13],
                data[pos + 14],
                data[pos + 15],
            ]);
            bits = u16::from_le_bytes([data[pos + 22], data[pos + 23]]);
        }
        if &data[pos..pos + 4] == b"data" {
            break;
        }
        pos = body_end;
    }
    if channels == 0 {
        return Err("zero channels".to_string());
    }
    if rate == 0 {
        return Err("zero sample rate".to_string());
    }
    Ok(ToyWav {
        channels,
        rate,
        bits,
    })
}

fn register_op_strategy() -> impl Strategy<Value = RegisterOp> {
    (0..4usize, 0..1_000u64, proptest::option::of("[a-z]{0,4}")).prop_map(|(key_idx, ts, value)| {
        RegisterOp {
            key: format!("key{key_idx}"),
            ts,
            value,
        }
    })
}

proptest! {
    #![proptest_config(determinism_config(Seed::default()))]

    #[test]
    fn toy_wav_never_panics(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        fuzz_parser_never_panics!(parser: toy_wav_decode, input: &data);
    }

    #[test]
    fn crdt_commutes_across_batches(
        ops_a in proptest::collection::vec(register_op_strategy(), 0..12),
        ops_b in proptest::collection::vec(register_op_strategy(), 0..12),
    ) {
        let initial = Registers::default();
        let ab = apply_register_batch(
            apply_register_batch(initial.clone(), ops_a.clone()),
            ops_b.clone(),
        );
        let ba = apply_register_batch(apply_register_batch(initial, ops_b), ops_a);
        prop_assert_eq!(ab, ba);
    }

    #[test]
    fn crdt_single_ops_are_idempotent(
        op in register_op_strategy(),
    ) {
        let initial = Registers::default();
        let once = apply_register(initial, op.clone());
        let twice = apply_register(once.clone(), op);
        prop_assert_eq!(once, twice);
    }
}

#[cfg(test)]
mod direct {
    use std::panic::catch_unwind;

    use tpt_av_test_fuzz::parser::panics_on;

    #[test]
    fn panics_on_detects_panics_and_only_panics() {
        assert!(panics_on(|| panic!("boom")));
        assert!(!panics_on(|| {}));
    }

    #[test]
    fn macro_catches_a_panicking_parser() {
        fn always_panics(_: &[u8]) -> Result<(), ()> {
            panic!("intentional")
        }
        // The failing assertion must surface as a panic of the *test*.
        let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
            tpt_av_test_fuzz::fuzz_parser_never_panics!(parser: always_panics, input: &b""[..]);
        }));
        assert!(result.is_err(), "macro must assert on a panicking parser");
    }
}
