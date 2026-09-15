//! Regression corpus of known-bad inputs.
//!
//! Each file under `tpt-av-test-fuzz/corpus/` is a byte pattern that once
//! crashed (or hangs) a TPT AV Stack parser. Every CI build re-runs the whole
//! corpus through every parser via [`run`]; a panic or a hang is a hard
//! failure. Files are embedded at compile time so the corpus can never be
//! silently skipped.

/// Immutable, compile-time-embedded corpus payloads.
pub mod data {
    /// `(file name, bytes)` pairs for every known-bad input.
    pub const FILES: &[(&str, &[u8])] = &[
        (
            "wav_truncated_header.bin",
            include_bytes!("../corpus/wav_truncated_header.bin"),
        ),
        (
            "wav_channels_zero.bin",
            include_bytes!("../corpus/wav_channels_zero.bin"),
        ),
        (
            "flac_streaminfo_length_overflow.bin",
            include_bytes!("../corpus/flac_streaminfo_length_overflow.bin"),
        ),
        (
            "opus_opushead_version_ff.bin",
            include_bytes!("../corpus/opus_opushead_version_ff.bin"),
        ),
        (
            "h264_short_nal.bin",
            include_bytes!("../corpus/h264_short_nal.bin"),
        ),
        ("zeros_4096.bin", include_bytes!("../corpus/zeros_4096.bin")),
    ];
}

/// Signature of a parser under regression test. The returned label is a
/// human-readable failure reason.
pub type ParserFn = dyn Fn(&[u8]) -> Result<(), String>;

/// Runs every corpus entry through `parser`. Panics (including hangs) surface
/// from the caller framework; a reported `Err` is collected and returned as
/// `(file, reason)`.
pub fn run(parser: &ParserFn) -> Result<(), Vec<(String, String)>> {
    let mut failures = Vec::new();
    for (name, bytes) in data::FILES {
        if let Err(reason) = parser(bytes) {
            failures.push(((*name).to_string(), reason));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_contains_six_known_bad_inputs() {
        assert_eq!(data::FILES.len(), 6);
        for (_, bytes) in data::FILES {
            assert!(
                !bytes.is_empty(),
                "corpus entries must be non-empty byte patterns"
            );
        }
    }

    #[test]
    fn corpus_run_reports_each_rejection() {
        // A parser that rejects every entry must be reported for each file,
        // while remaining total (no panics).
        let strict = |bytes: &[u8]| -> Result<(), String> {
            let _ = bytes.len();
            Err("rejected".into())
        };
        let failures = run(&strict).unwrap_err();
        assert_eq!(failures.len(), data::FILES.len());
    }
}
