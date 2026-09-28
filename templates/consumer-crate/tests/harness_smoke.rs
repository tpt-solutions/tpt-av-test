//! Starter tests proving each `tpt-av-test-*` harness crate is wired up.
//! Replace these with real conformance/fuzz/benchmark/mock coverage for
//! `{{project-name}}`.

use tpt_av_test_fuzz::seed::Seed;
use tpt_av_test_mock::midi_port::{MidiMessage, MockMidiDevice};

#[test]
fn fuzz_seed_is_deterministic() {
    let a = Seed::from_bytes(b"{{project-name}}").next();
    let b = Seed::from_bytes(b"{{project-name}}").next();
    assert_eq!(a, b, "the same input bytes must always produce the same seed");
}

#[test]
fn mock_midi_round_trips_a_message() {
    let device = MockMidiDevice::new();
    device.inject(MidiMessage::note_on(0, 60, 100));
    let received = device.try_recv().unwrap().expect("message was injected");
    assert!(received.is_note_on());
}

// `tpt-av-test-reference` and `tpt-av-test-benchmark` need real media
// fixtures / real-time callbacks to exercise meaningfully — see
// https://github.com/tpt-solutions/tpt-av-test for
// `assert_bit_exact_vs_ffmpeg`, `assert_image_similar`, and
// `assert_real_time_safe!` usage examples once {{project-name}} has a
// decoder or audio callback to test.
