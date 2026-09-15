//! libsndfile-style metadata probing via the `ffprobe`/`sox` CLI binaries.
//!
//! Like everything else in this harness, libsndfile's GPL/LGPL tooling is
//! invoked as a subprocess — never linked. Here we wrap `ffprobe` (from the
//! FFmpeg build) to read container/stream metadata so tests can confirm a
//! decoder produced the right rate, channel count, and bit depth.

use std::ffi::OsStr;
use std::path::Path;

use crate::subprocess::run_captured;
use crate::{ReferenceError, ReferenceResult};

/// The `ffprobe` binary name looked up on `PATH`.
pub const FFPROBE_BIN: &str = "ffprobe";
/// The `sox` binary name looked up on `PATH`.
pub const SOX_BIN: &str = "sox";

/// Metadata describing the first audio stream of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    /// Codec short name, e.g. `pcm_s16le`, `flac`, `opus`.
    pub codec_name: String,
    /// Sample rate in hertz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u16,
    /// Bits per sample when the codec exposes it (e.g. `16`, `24`, `32`).
    pub bit_depth: Option<u16>,
}

/// Probes `input` with `ffprobe` and returns the first audio stream's info.
pub fn probe(input: &Path) -> ReferenceResult<StreamInfo> {
    let args: Vec<&OsStr> = vec![
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-select_streams"),
        OsStr::new("a:0"),
        OsStr::new("-show_entries"),
        OsStr::new("stream=codec_name,sample_rate,channels,bits_per_sample,bits_per_raw_sample"),
        OsStr::new("-of"),
        OsStr::new("default=noprint_wrappers=1"),
        input.as_os_str(),
    ];
    let (exit_code, stdout, stderr) = run_captured(FFPROBE_BIN, &args)?;
    if exit_code != 0 {
        return Err(ReferenceError::ReferenceFailed {
            binary: FFPROBE_BIN,
            exit_code,
            stderr,
        });
    }
    parse_stream_info(stdout.as_slice())
}

/// Probes `input` with the `sox` binary (`sox --i input`), returning the raw
/// human-readable summary for diagnostics or embedding in failure messages.
pub fn sox_summary(input: &Path) -> ReferenceResult<String> {
    let args: [&OsStr; 2] = [OsStr::new("--i"), input.as_os_str()];
    let (exit_code, stdout, stderr) = run_captured(SOX_BIN, &args)?;
    if exit_code != 0 {
        return Err(ReferenceError::ReferenceFailed {
            binary: SOX_BIN,
            exit_code,
            stderr,
        });
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

fn parse_stream_info(stdout: &[u8]) -> ReferenceResult<StreamInfo> {
    let text = String::from_utf8_lossy(stdout);
    let mut codec_name = None;
    let mut sample_rate = None;
    let mut channels = None;
    let mut bit_depth = None;

    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        match key {
            "codec_name" | "streams.stream.0.codec_name" => codec_name = Some(value.to_string()),
            "sample_rate" | "streams.stream.0.sample_rate" => sample_rate = value.parse().ok(),
            "channels" | "streams.stream.0.channels" => channels = value.parse().ok(),
            "bits_per_sample" | "streams.stream.0.bits_per_sample" => {
                bit_depth = value.parse().ok().or(bit_depth)
            }
            "bits_per_raw_sample" | "streams.stream.0.bits_per_raw_sample"
                if bit_depth.is_none() =>
            {
                bit_depth = value.parse().ok();
            }
            _ => {}
        }
    }

    let codec_name = codec_name.ok_or_else(|| invalid(FFPROBE_BIN, text.trim()))?;
    let sample_rate = sample_rate.ok_or_else(|| invalid(FFPROBE_BIN, text.trim()))?;
    let channels = channels.ok_or_else(|| invalid(FFPROBE_BIN, text.trim()))?;

    Ok(StreamInfo {
        codec_name,
        sample_rate,
        channels,
        bit_depth,
    })
}

fn invalid(binary: &'static str, text: &str) -> ReferenceError {
    ReferenceError::InvalidReferenceOutput {
        binary,
        detail: format!("expected stream metadata, got: {text:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FFPROBE_OUT: &str = concat!(
        "bits_per_raw_sample=24\n",
        "codec_name=pcm_s24le\n",
        "sample_rate=48000\n",
        "channels=2\n",
        "r_frame_rate=0/0\n",
    );

    #[test]
    fn parses_flat_ffprobe_output() {
        let info = parse_stream_info(FFPROBE_OUT.as_bytes()).unwrap();
        assert_eq!(info.codec_name, "pcm_s24le");
        assert_eq!(info.sample_rate, 48000);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bit_depth, Some(24));
    }

    #[test]
    fn refuses_junk_output() {
        let err = parse_stream_info(b"not metadata".as_slice()).unwrap_err();
        assert!(matches!(
            err,
            ReferenceError::InvalidReferenceOutput {
                binary: FFPROBE_BIN,
                ..
            }
        ));
    }
}
