//! Golden-master comparison harness for the TPT AV Stack.
//!
//! Proves that pure-Rust decoders and compositors match industry-standard
//! references (FFmpeg, libsndfile, sox, ffprobe) — **without linking** their
//! GPL/LGPL code. Every reference is invoked as a detached CLI subprocess and
//! its output is streamed over piped stdout.
//!
//! This crate is intended for use via `[dev-dependencies]` and `cfg(test)`
//! only. It must never appear in a production binary's dependency graph.
//!
//! ## Modules
//!
//! - [`ffmpeg`] — FFmpeg CLI subprocess wrapper (decodes to raw PCM on stdout).
//! - [`libsndfile`] — libsndfile/sox/ffprobe CLI wrapper.
//! - [`audio`] — PCM bit-exact comparison with float epsilon.
//! - [`image`] — pixel comparison via PSNR/SSIM.

mod subprocess;

pub mod audio;
pub mod ffmpeg;
pub mod image;
pub mod libsndfile;

/// Errors produced while invoking reference tools or comparing output.
#[derive(Debug)]
pub enum ReferenceError {
    /// The reference binary (ffmpeg/ffprobe/sox) is not on `PATH`.
    ReferenceBinaryNotFound { binary: &'static str },
    /// The reference subprocess could not be spawned.
    Spawn {
        binary: &'static str,
        source: std::io::Error,
    },
    /// The reference subprocess exited non-zero.
    ReferenceFailed {
        binary: &'static str,
        exit_code: i32,
        stderr: String,
    },
    /// The reference tool could not read/interrogate the given file.
    ReferenceIo {
        binary: &'static str,
        source: std::io::Error,
    },
    /// Raw PCM payload length was not a multiple of the sample width.
    PcmUnaligned { bytes: usize },
    /// The reference tool emitted output that could not be parsed.
    InvalidReferenceOutput {
        binary: &'static str,
        detail: String,
    },
    /// Decoded sample counts disagree between reference and TPT decoder.
    SampleCountMismatch {
        expected: usize,
        actual: usize,
        context: String,
    },
    /// Sample values exceeded the allowed tolerance.
    PcmMismatch {
        sample_index: usize,
        expected: f32,
        actual: f32,
        tolerance: f32,
        context: String,
    },
    /// Images had different dimensions and could not be compared.
    ImageDimensionMismatch {
        generated: (u32, u32),
        reference: (u32, u32),
    },
    /// Image similarity fell below the required PSNR threshold.
    ImagePsnr {
        psnr: f64,
        required: f64,
        path: std::path::PathBuf,
    },
    /// The reference image could not be decoded.
    ImageDecode(String),
    /// A frame claimed bit-exact differed from the reference.
    FrameNotExact {
        /// The reference image that was compared against.
        path: std::path::PathBuf,
        /// Coordinates of the first differing pixel.
        pixel: (u32, u32),
        /// RGB of the generated pixel.
        generated: [u8; 3],
        /// RGB of the reference pixel.
        reference: [u8; 3],
    },
}

impl std::fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferenceError::ReferenceBinaryNotFound { binary } => {
                write!(f, "reference binary '{binary}' not found on PATH")
            }
            ReferenceError::Spawn { binary, source } => {
                write!(f, "failed to spawn '{binary}': {source}")
            }
            ReferenceError::ReferenceFailed {
                binary,
                exit_code,
                stderr,
            } => {
                write!(f, "'{binary}' exited with code {exit_code}: {stderr}")
            }
            ReferenceError::ReferenceIo { binary, source } => {
                write!(f, "I/O error interacting with '{binary}': {source}")
            }
            ReferenceError::PcmUnaligned { bytes } => {
                write!(f, "raw PCM payload has {bytes} bytes, not a multiple of 4")
            }
            ReferenceError::InvalidReferenceOutput { binary, detail } => {
                write!(f, "unparseable output from '{binary}': {detail}")
            }
            ReferenceError::SampleCountMismatch {
                expected,
                actual,
                context,
            } => {
                write!(
                    f,
                    "{context}: sample count mismatch — reference yielded {expected}, \
                     TPT decoder yielded {actual}"
                )
            }
            ReferenceError::PcmMismatch {
                sample_index,
                expected,
                actual,
                tolerance,
                context,
            } => {
                write!(
                    f,
                    "{context}: sample {sample_index} differs by {:.6} (expected {expected}, \
                     got {actual}); tolerance was {tolerance}",
                    (expected - actual).abs()
                )
            }
            ReferenceError::ImageDimensionMismatch {
                generated,
                reference,
            } => write!(
                f,
                "image dimension mismatch — generated {generated:?}, \
                 reference {reference:?}"
            ),
            ReferenceError::ImagePsnr {
                psnr,
                required,
                path,
            } => write!(
                f,
                "image PSNR {psnr:.2} dB is below the required {required:.2} dB \
                 for reference '{}'",
                path.display()
            ),
            ReferenceError::ImageDecode(message) => {
                write!(f, "failed to decode reference image: {message}")
            }
            ReferenceError::FrameNotExact {
                path,
                pixel,
                generated,
                reference,
            } => write!(
                f,
                "frame is not bit-exact against '{}' — first difference at pixel \
                 ({}, {}): generated {:?}, reference {:?}",
                path.display(),
                pixel.0,
                pixel.1,
                generated,
                reference
            ),
        }
    }
}

impl std::error::Error for ReferenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReferenceError::Spawn { source, .. } | ReferenceError::ReferenceIo { source, .. } => {
                Some(source)
            }
            _ => None,
        }
    }
}

/// Result-alias used throughout the reference harness.
pub type ReferenceResult<T> = Result<T, ReferenceError>;
