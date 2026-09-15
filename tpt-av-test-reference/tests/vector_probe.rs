//! Integration tests exercising the committed golden-master vectors through
//! the `ffprobe` CLI wrapper. Skips silently when `ffprobe` is unavailable.

use std::path::PathBuf;

use tpt_av_test_reference::libsndfile::probe;

fn vector(rel: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.join("..").join("tpt-av-test-vectors").join(rel)
}

fn ffprobe_available() -> bool {
    std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .is_ok()
}

#[test]
fn committed_s16_mono_wav_probes_correctly() {
    if !ffprobe_available() {
        eprintln!("ffprobe not found on PATH; skipping");
        return;
    }
    let info = probe(&vector("audio/wav/sine-440-48k-s16-mono.wav")).unwrap();
    assert_eq!(info.codec_name, "pcm_s16le");
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.channels, 1);
    assert_eq!(info.bit_depth, Some(16));
}

#[test]
fn committed_s24_stereo_wav_probes_correctly() {
    if !ffprobe_available() {
        eprintln!("ffprobe not found on PATH; skipping");
        return;
    }
    let info = probe(&vector("audio/wav/sine-440-48k-s24-stereo.wav")).unwrap();
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.channels, 2);
    assert_eq!(info.bit_depth, Some(24));
}

#[test]
fn committed_flac_vector_probes_as_flac() {
    if !ffprobe_available() {
        eprintln!("ffprobe not found on PATH; skipping");
        return;
    }
    let info = probe(&vector("audio/flac/sine-440-48k-s16-stereo.flac")).unwrap();
    assert_eq!(info.codec_name, "flac");
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.channels, 2);
}
