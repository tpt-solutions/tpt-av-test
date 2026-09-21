# tpt-av-test-mock

Mock hardware and network infrastructure for the [TPT AV Stack](https://opensource.tptsolutions.co.nz/).

Lets every repository exercise audio, MIDI, network, and filesystem code
paths in CI **without physical hardware**: virtual MIDI ports, sine-wave
audio devices, a deterministic mock WebRTC/WebSocket transport on a virtual
clock, and an in-memory virtual filesystem.

This crate is `[dev-dependencies]` / `cfg(test)` only.

## Modules

- `midi_port` — virtual MIDI in/out (`MockMidiDevice`, `inject`/`try_recv`).
- `audio_device` — `MockAudioDevice` (sine wave generator, accepts buffers).
- `network` — `MockNetworkTransport` (mock WebRTC/WebSocket transport).
- `filesystem` — in-memory virtual filesystem.

## Usage

```toml
[dev-dependencies]
tpt-av-test-mock = { git = "https://github.com/tpt-solutions/tpt-av-test" }
```

```rust,ignore
use tpt_av_test_mock::midi_port::MockMidiDevice;

let mut device = MockMidiDevice::new();
device.inject(&[0x90, 60, 127]); // note-on, middle C, full velocity
let received = device.try_recv().expect("message queued");
```

## License

Dual-licensed under [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE), at your option.

See the [workspace README](../README.md) for the full project vision and layout.
