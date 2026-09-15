//! Integration tests for `assert_image_similar` — no external binaries needed.

use std::path::Path;

use image::{Rgb, RgbImage};
use tpt_av_test_reference::image::assert_image_similar;
use tpt_av_test_reference::ReferenceError;

fn test_card() -> RgbImage {
    let (width, height) = (192, 108);
    let mut img = RgbImage::new(width, height);
    let bars = [
        Rgb([255, 255, 255]),
        Rgb([255, 255, 0]),
        Rgb([0, 255, 255]),
        Rgb([0, 255, 0]),
        Rgb([255, 0, 255]),
        Rgb([255, 0, 0]),
        Rgb([0, 0, 255]),
    ];
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let stripe = (x as usize * bars.len() / width as usize).min(bars.len() - 1);
        *pixel = bars[stripe];
        let _ = y;
    }
    img
}

fn write_png(path: &Path, img: &RgbImage) {
    img.save(path).unwrap();
}

#[test]
fn identical_image_passes_even_at_infinite_psnr() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference.png");
    let card = test_card();
    write_png(&reference, &card);

    assert_image_similar(&card.into(), &reference, 50.0).unwrap();
}

#[test]
fn slight_noise_still_passes_at_moderate_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference.png");
    let mut card = test_card();
    write_png(&reference, &card);

    for (_, _, pixel) in card.enumerate_pixels_mut() {
        let nudge = (pixel[0] as i16 + 3).clamp(0, 255) as u8;
        pixel[0] = nudge;
    }

    assert_image_similar(&card.into(), &reference, 40.0).unwrap();
}

#[test]
fn unrelated_image_fails_psnr_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference.png");
    let card = test_card();
    write_png(&reference, &card);

    let black = RgbImage::from_pixel(card.width(), card.height(), Rgb([0, 0, 0]));

    match assert_image_similar(&black.into(), &reference, 30.0) {
        Err(ReferenceError::ImagePsnr { required, .. }) => assert_eq!(required, 30.0),
        other => panic!("expected ImagePsnr, got {other:?}"),
    }
}

#[test]
fn dimension_mismatch_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference.png");
    let card = test_card();
    write_png(&reference, &card);

    let scaled = RgbImage::from_pixel(64, 64, Rgb([10, 20, 30]));

    match assert_image_similar(&scaled.into(), &reference, 30.0) {
        Err(ReferenceError::ImageDimensionMismatch {
            generated: (64, 64),
            reference: (192, 108),
        }) => {}
        other => panic!("expected ImageDimensionMismatch, got {other:?}"),
    }
}

#[test]
fn missing_reference_image_errors_cleanly() {
    let card = test_card();
    let missing = Path::new("does-not-exist.png");
    let err = assert_image_similar(&card.into(), missing, 30.0).unwrap_err();
    assert!(matches!(err, ReferenceError::ImageDecode(_)));
}
