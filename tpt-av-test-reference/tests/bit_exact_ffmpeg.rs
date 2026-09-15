//! Integration tests for `assert_bit_exact_vs_ffmpeg`.
//!
//! These exercises run the real `ffmpeg` binary via subprocess. When `ffmpeg`
//! is not installed on the host (e.g. a bare dev workstation) the tests skip;
//! CI runners provision `ffmpeg`, so the gate is enforced there.

use std::io::Write;
use std::path::Path;

use tpt_av_test_reference::audio::assert_bit_exact_vs_ffmpeg;
use tpt_av_test_reference::ReferenceError;

fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok()
}

fn sine_i16(frames: usize) -> Vec<i16> {
    let freq = 440.0;
    let sample_rate = 44_100.0;
    (0..frames)
        .map(|t| {
            let phase = 2.0 * std::f64::consts::PI * freq * (t as f64 / sample_rate);
            (phase.sin() * 32_767.0).round() as i16
        })
        .collect()
}

fn write_wav(path: &Path, samples: &[i16], sample_rate: u32, channels: u16) {
    let block_align = channels * 2;
    let data_size = samples.len() as u32 * block_align as u32;
    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for &sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(&bytes).unwrap();
}

#[test]
fn mono_pcm16_matches_ffmpeg_bit_for_bit() {
    if !ffmpeg_available() {
        eprintln!("ffmpeg not found on PATH; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let wav = &dir.path().join("sine.wav");
    let sample_rate = 44_100u32;
    let channels = 1u16;
    let samples = sine_i16(sample_rate as usize / 4);

    write_wav(wav, &samples, sample_rate, channels);
    let expected: Vec<f32> = samples.iter().map(|&s| s as f32 / 32_768.0).collect();

    assert_bit_exact_vs_ffmpeg(wav, &expected, sample_rate, channels, 0.0).unwrap();
}

#[test]
fn stereo_pcm16_matches_ffmpeg_bit_for_bit() {
    if !ffmpeg_available() {
        eprintln!("ffmpeg not found on PATH; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let wav = &dir.path().join("stereo.wav");
    let sample_rate = 48_000u32;
    let channels = 2u16;
    let mono = sine_i16(sample_rate as usize / 8);
    let mut interleaved = Vec::with_capacity(mono.len() * 2);
    for &sample in &mono {
        interleaved.push(sample);
        interleaved.push((sample / 2).wrapping_sub(1)); // distinct right channel
    }

    write_wav(wav, &interleaved, sample_rate, channels);
    let expected: Vec<f32> = interleaved.iter().map(|&s| s as f32 / 32_768.0).collect();

    assert_bit_exact_vs_ffmpeg(wav, &expected, sample_rate, channels, 0.0).unwrap();
}

#[test]
fn inserted_error_is_detected() {
    if !ffmpeg_available() {
        eprintln!("ffmpeg not found on PATH; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let wav = &dir.path().join("sine.wav");
    let sample_rate = 44_100u32;
    let channels = 1u16;
    let samples = sine_i16(sample_rate as usize / 4);

    write_wav(wav, &samples, sample_rate, channels);
    let mut expected: Vec<f32> = samples.iter().map(|&s| s as f32 / 32_768.0).collect();
    expected[1000] += 0.5; // disturb one sample

    match assert_bit_exact_vs_ffmpeg(wav, &expected, sample_rate, channels, 0.0) {
        Err(ReferenceError::PcmMismatch { sample_index, .. }) => assert_eq!(sample_index, 1000),
        other => panic!("expected PcmMismatch at index 1000, got {other:?}"),
    }
}

#[test]
fn garbage_input_surfaces_ffmpeg_failure() {
    if !ffmpeg_available() {
        eprintln!("ffmpeg not found on PATH; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let bogus = &dir.path().join("bogus.wav");
    std::fs::write(bogus, b"this is definitely not a wav file").unwrap();

    let err = assert_bit_exact_vs_ffmpeg(bogus, &[0.0; 64], 44_100, 1, 0.0).unwrap_err();
    assert!(matches!(
        err,
        ReferenceError::ReferenceFailed {
            binary: "ffmpeg",
            ..
        }
    ));
}
