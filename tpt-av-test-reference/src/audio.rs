//! PCM bit-exact comparison with optional float epsilon.

use std::path::Path;

use crate::{ffmpeg, ReferenceError, ReferenceResult};

/// Compares two slice of interleaved `f32` samples, asserting that every
/// sample is within `tolerance` of its counterpart.
///
/// Use `tolerance = 0.0` for integer-sourced PCM (bit-exact), and a small
/// epsilon (e.g. `1e-6`) when one side has gone through float processing.
pub fn assert_pcm_equal(
    expected: &[f32],
    actual: &[f32],
    tolerance: f32,
    context: &str,
) -> ReferenceResult<()> {
    if expected.len() != actual.len() {
        return Err(ReferenceError::SampleCountMismatch {
            expected: expected.len(),
            actual: actual.len(),
            context: context.to_string(),
        });
    }
    for (sample_index, (expect, got)) in expected.iter().zip(actual).enumerate() {
        let diff = (expect - got).abs();
        let exceeds_tolerance = diff > tolerance || diff.is_nan();
        if exceeds_tolerance {
            return Err(ReferenceError::PcmMismatch {
                sample_index,
                expected: *expect,
                actual: *got,
                tolerance,
                context: context.to_string(),
            });
        }
    }
    Ok(())
}

/// Decodes `input_file` with FFmpeg (via subprocess) and asserts that the
/// supplied `tpt_decoder_output` matches the reference bit-for-bit within
/// `tolerance`.
pub fn assert_bit_exact_vs_ffmpeg(
    input_file: &Path,
    tpt_decoder_output: &[f32],
    sample_rate: u32,
    channels: u16,
    tolerance: f32,
) -> ReferenceResult<()> {
    let reference = ffmpeg::decode_to_f32le(input_file, sample_rate, channels)?;
    assert_pcm_equal(
        &reference,
        tpt_decoder_output,
        tolerance,
        "assert_bit_exact_vs_ffmpeg",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_buffers_pass_with_zero_tolerance() {
        assert!(assert_pcm_equal(&[0.0, 1.0, -1.0], &[0.0, 1.0, -1.0], 0.0, "t").is_ok());
    }

    #[test]
    fn epsilon_tolerance_absorbs_float_noise() {
        assert!(assert_pcm_equal(&[1.0, 2.0], &[1.0000001, 1.9999999], 1e-6, "t").is_ok());
    }

    #[test]
    fn drift_beyond_tolerance_is_rejected() {
        match assert_pcm_equal(&[1.0, 2.0], &[1.0, 2.5], 1e-6, "t") {
            Err(ReferenceError::PcmMismatch { sample_index, .. }) => assert_eq!(sample_index, 1),
            other => panic!("expected PcmMismatch at index 1, got {other:?}"),
        }
    }

    #[test]
    fn nan_is_rejected_even_between_buffers() {
        assert!(assert_pcm_equal(&[f32::NAN, 2.0], &[0.0, 2.0], 0.0, "t").is_err());
    }
}
