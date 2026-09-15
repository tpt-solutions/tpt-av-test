//! Regenerates the deterministic reference images committed under
//! `tpt-av-test-vectors/images/reference/`.
//!
//! Usage:
//!
//! ```sh
//! cargo run -p tpt-av-test-reference --example generate_reference_images -- \
//!   tpt-av-test-vectors/images/reference
//! ```
//!
//! Every image is purely deterministic (no RNG, no timestamps), so re-running
//! this generator reproduces byte-identical golden masters.

use image::{Rgb, RgbImage};
use std::env;
use std::path::Path;

const BAR_COLORS: [Rgb<u8>; 7] = [
    Rgb([255, 255, 255]),
    Rgb([255, 255, 0]),
    Rgb([0, 255, 255]),
    Rgb([0, 255, 0]),
    Rgb([255, 0, 255]),
    Rgb([255, 0, 0]),
    Rgb([0, 0, 255]),
];

fn smpte_bars(width: u32, height: u32) -> RgbImage {
    let mut image = RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let stripe = (x as usize * BAR_COLORS.len() / width as usize).min(BAR_COLORS.len() - 1);
        *pixel = BAR_COLORS[stripe];
        if y > height * 3 / 4 {
            let _ = y; // full-height bars for simplicity
        }
    }
    image
}

fn smooth_gradient(width: u32, height: u32) -> RgbImage {
    let mut image = RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let t = x as f64 / (width - 1) as f64;
        let y_t = y as f64 / height as f64;
        let r = (255.0 * t) as u8;
        let g = (255.0 * (1.0 - t)) as u8;
        let b = (255.0 * y_t) as u8;
        *pixel = Rgb([r, g, b]);
    }
    image
}

fn checkerboard(width: u32, height: u32, cell: u32) -> RgbImage {
    let mut image = RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let even = ((x / cell) + (y / cell)) % 2 == 0;
        *pixel = if even {
            Rgb([255, 255, 255])
        } else {
            Rgb([0, 0, 0])
        };
    }
    image
}

fn main() {
    let out = env::args()
        .nth(1)
        .unwrap_or_else(|| "tpt-av-test-vectors/images/reference".to_string());
    let dir = Path::new(&out);
    std::fs::create_dir_all(dir).expect("create output directory");

    let images: [(&str, RgbImage); 3] = [
        ("smpte_bars_1080p.png", smpte_bars(1920, 1080)),
        ("smooth_gradient_1080p.png", smooth_gradient(1920, 1080)),
        ("checkerboard_16px_1024.png", checkerboard(1024, 1024, 16)),
    ];

    for (name, image) in images {
        let path = dir.join(name);
        image.save(&path).expect("save reference image");
        println!("wrote {}", path.display());
    }
}
