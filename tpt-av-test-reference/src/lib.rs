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
//! ## Modules (Phase 1)
//!
//! - [`ffmpeg`] — FFmpeg CLI subprocess wrapper (decodes to raw PCM on stdout).
//! - [`libsndfile`] — libsndfile/sox/ffprobe CLI wrapper.
//! - [`audio`] — PCM bit-exact comparison with float epsilon.
//! - [`image`] — pixel comparison via PSNR/SSIM.

/// Placeholder version marker; replaced by full module wiring in Phase 1.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");