//! Mock hardware and network infrastructure for the TPT AV Stack.
//!
//! Lets every repository exercise audio, MIDI, network, and filesystem code
//! paths in CI **without physical hardware**: virtual MIDI ports, sine-wave
//! audio devices, a deterministic mock WebRTC/WebSocket transport on a
//! virtual clock, and an in-memory virtual filesystem.
//!
//! This crate is intended for use via `[dev-dependencies]` and `cfg(test)`
//! only.
//!
//! ## Modules
//!
//! - [`midi_port`] — virtual MIDI in/out (`MockMidiDevice`, `inject`/`try_recv`).
//! - [`audio_device`] — `MockAudioDevice` (sine wave generator, accepts buffers).
//! - [`network`] — `MockNetworkTransport` (mock WebRTC/WebSocket transport).
//! - [`filesystem`] — in-memory virtual filesystem.

pub mod audio_device;
pub mod filesystem;
pub mod midi_port;
pub mod network;
