# Changelog

All notable changes to `tpt-av-test-mock` are documented in this file.

## [0.1.0] — 2026-09-21

Initial release.

- `MockMidiDevice`: virtual MIDI in/out with `inject`/`try_recv` (`midi_port`).
- `MockAudioDevice`: sine-wave generator accepting arbitrary buffers (`audio_device`).
- `MockNetworkTransport`: deterministic mock WebRTC/WebSocket transport on a
  virtual clock (`network`).
- In-memory virtual filesystem (`filesystem`).
