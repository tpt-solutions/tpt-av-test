//! FFmpeg CLI subprocess wrapper.
//!
//! `ffmpeg` is spawned as a detached process — never linked. The GPL license
//! of FFmpeg therefore never infects the MIT-licensed TPT code; we only read
//! raw PCM off its stdout pipe.

use std::ffi::OsStr;
use std::path::Path;

use crate::subprocess::run_captured;
use crate::{ReferenceError, ReferenceResult};

/// The binary name looked up on `PATH`.
pub const FFMPEG_BIN: &str = "ffmpeg";

/// Decodes `input` to interleaved, little-endian IEEE-754 `f32` samples at
/// the requested sample rate and channel count.
///
/// Implemented as
/// `ffmpeg -v error -hide_banner -nostdin -i <input> -f f32le
/// -acodec pcm_f32le -ar <sr> -ac <ch> -`.
///
/// Returns an error if `ffmpeg` is missing, fails, or emits output that is
/// not a whole number of `f32` frames.
pub fn decode_to_f32le(input: &Path, sample_rate: u32, channels: u16) -> ReferenceResult<Vec<f32>> {
    let sample_rate_arg = sample_rate.to_string();
    let channels_arg = channels.to_string();
    let args: [&OsStr; 15] = [
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-hide_banner"),
        OsStr::new("-nostdin"),
        OsStr::new("-i"),
        input.as_os_str(),
        OsStr::new("-f"),
        OsStr::new("f32le"),
        OsStr::new("-acodec"),
        OsStr::new("pcm_f32le"),
        OsStr::new("-ar"),
        OsStr::new(&sample_rate_arg),
        OsStr::new("-ac"),
        OsStr::new(&channels_arg),
        OsStr::new("-"),
    ];

    let (exit_code, stdout, stderr) = run_captured(FFMPEG_BIN, &args)?;
    if exit_code != 0 {
        return Err(ReferenceError::ReferenceFailed {
            binary: FFMPEG_BIN,
            exit_code,
            stderr,
        });
    }

    pcm_bytes_to_f32(stdout)
}

/// Converts a raw byte buffer of little-endian `f32` samples into a `Vec<f32>`.
pub fn pcm_bytes_to_f32(raw: Vec<u8>) -> ReferenceResult<Vec<f32>> {
    if raw.len() % 4 != 0 {
        return Err(ReferenceError::PcmUnaligned { bytes: raw.len() });
    }
    let mut samples = Vec::with_capacity(raw.len() / 4);
    for chunk in raw.chunks_exact(4) {
        let bytes: [u8; 4] = chunk.try_into().expect("chunks_exact guarantees length 4");
        let bits = u32::from_le_bytes(bytes);
        samples.push(f32::from_bits(bits));
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_bytes_to_f32_parses_interleaved_le() {
        let raw = vec![
            0x00, 0x00, 0x00, 0x3f, // 0.5_f32
            0x00, 0x00, 0x80, 0xbf, // -1.0_f32
            0x00, 0x00, 0x00, 0x00, // 0.0_f32
        ];
        let samples = pcm_bytes_to_f32(raw).unwrap();
        assert_eq!(samples, vec![0.5, -1.0, 0.0]);
    }

    #[test]
    fn pcm_bytes_to_f32_rejects_unaligned_payload() {
        let err = pcm_bytes_to_f32(vec![0, 0, 0]).unwrap_err();
        assert!(matches!(err, ReferenceError::PcmUnaligned { bytes: 3 }));
    }
}
