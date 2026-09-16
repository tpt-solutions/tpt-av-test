//! Integration tests: the mock hardware working together, the way the TPT
//! AV Stack repositories will use it in CI.

use core::time::Duration;

use tpt_av_test_mock::audio_device::MockAudioDevice;
use tpt_av_test_mock::filesystem::VirtualFs;
use tpt_av_test_mock::midi_port::{MidiMessage, MockMidiDevice};
use tpt_av_test_mock::network::MockNetworkTransport;

/// `tpt-av-control` scenario: a MIDI controller plays notes, the synth
/// under test renders them, and the rendered output is captured.
#[test]
fn midi_to_audio_pipeline() {
    let device = MockMidiDevice::new();
    let mut synth_input = MockAudioDevice::sine(48_000, 2, 440.0).with_amplitude(0.25);

    // A performer plays middle C; the port hands it to the application.
    device.inject(MidiMessage::note_on(0, 60, 100));
    device.inject(MidiMessage::note_off(0, 60, 0));

    let mut rendered = Vec::new();
    while let Some(message) = device.try_recv().unwrap() {
        // "Render": on note-on produce a block of tone, on anything else
        // silence. In the real stack this is the app's job.
        let block = if message.is_note_on() {
            synth_input.generate(512).unwrap()
        } else {
            vec![0.0f32; 512 * 2]
        };
        rendered.push(block);
    }

    assert_eq!(rendered.len(), 2);
    assert!(
        rendered[0].iter().any(|sample| sample.abs() > 0.1),
        "the note-on block contains tone"
    );
    assert!(
        rendered[1].iter().all(|sample| *sample == 0.0),
        "the note-off block is silent"
    );

    // The rendering side is recorded through an output device.
    let mut sink = MockAudioDevice::new(48_000, 2);
    for block in &rendered {
        sink.accept(block).unwrap();
    }
    assert_eq!(sink.total_frames_accepted(), 1024);
    assert!(sink.peak_accepted() > 0.1);
}

/// `tpt-av-sync` scenario: two replicas exchange CRDT operations over an
/// impaired link; both must converge to the same operation set despite
/// latency, loss, and reordering.
#[test]
fn crdt_replication_over_impaired_link() {
    let transport = MockNetworkTransport::with_seed(0xC0FFEE);
    transport.set_latency(Duration::from_millis(20));
    transport.set_drop_rate(0.2);
    transport.set_reorder_rate(0.3);

    let (replica_a, replica_b) = transport.pair();
    for op in 0..20u8 {
        replica_a.send([op, 42]).unwrap();
    }
    transport.flush(16);

    let received: Vec<u8> = std::iter::from_fn(|| replica_b.try_recv())
        .map(|message| message.payload[0])
        .collect();
    let stats = transport.stats();
    assert_eq!(stats.sent, 20);
    assert_eq!(
        stats.sent - stats.dropped,
        received.len() as u64,
        "exactly the non-dropped ops arrive"
    );
    // Every op was sent once: no duplicates may be delivered, and CRDT
    // convergence tolerates whatever order the impaired link produced.
    let mut sorted = received.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), received.len(), "no duplicated deliveries");
    assert!(received.iter().all(|op| *op < 20));
}

/// Asset-management scenario: a virtual filesystem stores downloaded media
/// without touching the real disk.
#[test]
fn asset_cache_on_virtual_fs() {
    let mut fs = VirtualFs::new();
    fs.write_file("/cache/audio/sine-440.wav", b"RIFF....WAVEfmt ")
        .unwrap();
    fs.write_file("/cache/video/BA1_FT_C.264", [0, 0, 0, 1, 0x67])
        .unwrap();

    assert_eq!(fs.list_dir("/cache").unwrap(), vec!["audio", "video"]);
    assert_eq!(
        fs.read_file("/cache/video/BA1_FT_C.264").unwrap(),
        vec![0, 0, 0, 1, 0x67]
    );
    assert!(fs.file_size("/cache/audio/sine-440.wav").unwrap() > 0);

    // Downloading a fresh asset into an existing tree just works.
    fs.write_file("/cache/audio/sine-1k.wav", b"RIFF").unwrap();
    assert_eq!(
        fs.list_dir("/cache/audio").unwrap(),
        vec!["sine-1k.wav", "sine-440.wav"]
    );
}
