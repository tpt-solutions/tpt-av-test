//! Mock hardware and network infrastructure for the TPT AV Stack.
//!
//! Lets every repository exercise audio, MIDI, network, and filesystem code
//! paths in CI **without physical hardware**: virtual MIDI ports, sine-wave
//! audio devices, fake WebRTC/WebSocket transports, and an in-memory virtual
//! filesystem.
//!
//! This crate is intended for use via `[dev-dependencies]` and `cfg(test)`
//! only.
//!
//! ## Modules (Phase 4)
//!
//! - [`midi_port`] — virtual MIDI in/out (`MockMidiDevice`, `inject`/`try_recv`).
//! - [`audio_device`] — `MockAudioDevice` (sine wave generator, accepts buffers).
//! - [`network`] — `MockNetworkTransport` (mock WebRTC/WebSocket transport).
//! - [`filesystem`] — in-memory virtual filesystem.

/// Placeholder version marker; replaced by full module wiring in Phase 4.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
