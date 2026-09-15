//! Pixel comparison via PSNR and SSIM.
//!
//! Used by `tpt-visual` to prove GPU compositor output matches reference
//! images (golden masters) without linking any GPL image library.

use std::path::Path;

use image::{DynamicImage, RgbImage};

use crate::{ReferenceError, ReferenceResult};

/// Extracts the luma (perceived brightness) plane of an RGB image, in
/// `[0.0, 255.0]`, using ITU-R BT.601 coefficients.
fn luma(img: &RgbImage) -> Vec<f64> {
    let mut luma = Vec::with_capacity(img.len());
    for p in img.pixels() {
        let r = p[0] as f64;
        let g = p[1] as f64;
        let b = p[2] as f64;
        luma.push(0.299 * r + 0.587 * g + 0.114 * b);
    }
    luma
}

/// Mean squared error over all three RGB channels, in `[0, 65025]` units.
pub fn mse(a: &RgbImage, b: &RgbImage) -> f64 {
    debug_assert_eq!(a.dimensions(), b.dimensions());
    let mut sum = 0.0;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        for c in 0..3 {
            let d = pa[c] as f64 - pb[c] as f64;
            sum += d * d;
        }
    }
    sum / (a.len() as f64)
}

/// Peak Signal-to-Noise Ratio in decibels. Identical images yield +infinity.
pub fn psnr(a: &RgbImage, b: &RgbImage) -> f64 {
    let error = mse(a, b);
    if error == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0_f64.powi(2) / error).log10()
    }
}

const K1: f64 = 0.01;
const K2: f64 = 0.03;
const L: f64 = 255.0;
const WINDOW: usize = 11;

fn gaussian_weights() -> [f64; WINDOW] {
    let sigma = 1.5;
    let center = (WINDOW - 1) as f64 / 2.0;
    let mut weights = [0.0; WINDOW];
    let mut sum = 0.0;
    for (i, w) in weights.iter_mut().enumerate() {
        let d = i as f64 - center;
        *w = (-(d * d) / (2.0 * sigma * sigma)).exp();
        sum += *w;
    }
    for w in &mut weights {
        *w /= sum;
    }
    weights
}

fn mirror(idx: isize, len: isize) -> usize {
    if idx < 0 {
        -(idx) as usize % len as usize
    } else if idx >= len {
        (2 * len - idx - 2) as usize
    } else {
        idx as usize
    }
}

/// Structural SIMilarity index in `(0.0, 1.0]`; identical images yield 1.0.
///
/// Classic sliding 11x11 gaussian-window SSIM with mirror padding (tuned for
/// the smooth test cards used as golden masters).
pub fn ssim(a: &RgbImage, b: &RgbImage) -> f64 {
    debug_assert_eq!(a.dimensions(), b.dimensions());
    let width = a.width() as usize;
    let height = a.height() as usize;
    let la = luma(a);
    let lb = luma(b);
    let weights = gaussian_weights();
    let half = WINDOW / 2;

    let c1 = (K1 * L).powi(2);
    let c2 = (K2 * L).powi(2);

    let mut total = 0.0;
    for y in 0..height {
        for x in 0..width {
            let mut sum_a = 0.0;
            let mut sum_b = 0.0;
            let mut sum_a2 = 0.0;
            let mut sum_b2 = 0.0;
            let mut sum_ab = 0.0;
            for wy in 0..WINDOW {
                let yy = mirror(y as isize + wy as isize - half as isize, height as isize);
                for wx in 0..WINDOW {
                    let wweight = weights[wx] * weights[wy];
                    let xx = mirror(x as isize + wx as isize - half as isize, width as isize);
                    let i = yy * width + xx;
                    let a = la[i];
                    let b = lb[i];
                    sum_a += wweight * a;
                    sum_b += wweight * b;
                    sum_a2 += wweight * a * a;
                    sum_b2 += wweight * b * b;
                    sum_ab += wweight * a * b;
                }
            }
            let mean_a = sum_a;
            let mean_b = sum_b;
            let var_a = sum_a2 - mean_a * mean_a;
            let var_b = sum_b2 - mean_b * mean_b;
            let cov = sum_ab - mean_a * mean_b;
            let numerator = (2.0 * mean_a * mean_b + c1) * (2.0 * cov + c2);
            let denominator = (mean_a * mean_a + mean_b * mean_b + c1) * (var_a + var_b + c2);
            total += numerator / denominator;
        }
    }
    total / (width * height) as f64
}

/// Asserts that `generated` visually matches the image at `reference_path`.
///
/// `required_psnr_db` is the minimum acceptable PSNR in decibels (identical
/// images score +infinity). Returns `Err` on decodable-but-unmatched images.
pub fn assert_image_similar(
    generated: &DynamicImage,
    reference_path: &Path,
    required_psnr_db: f64,
) -> ReferenceResult<()> {
    let reference = match image::open(reference_path) {
        Ok(loaded) => loaded,
        Err(err) => return Err(ReferenceError::ImageDecode(err.to_string())),
    };
    image_dimensions_check(generated, &reference)?;

    let generated_rgb = generated.to_rgb8();
    let reference_rgb = reference.to_rgb8();
    let score = psnr(&generated_rgb, &reference_rgb);
    if score < required_psnr_db {
        return Err(ReferenceError::ImagePsnr {
            psnr: score,
            required: required_psnr_db,
            path: reference_path.to_path_buf(),
        });
    }
    Ok(())
}

fn image_dimensions_check(
    generated: &DynamicImage,
    reference: &DynamicImage,
) -> ReferenceResult<()> {
    let gw = generated.width();
    let gh = generated.height();
    let rw = reference.width();
    let rh = reference.height();
    if (gw, gh) != (rw, rh) {
        return Err(ReferenceError::ImageDimensionMismatch {
            generated: (gw, gh),
            reference: (rw, rh),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn solid(color: [u8; 3], w: u32, h: u32) -> RgbImage {
        RgbImage::from_pixel(w, h, Rgb(color))
    }

    #[test]
    fn identical_images_score_infinite_psnr_and_one_ssim() {
        let a = solid([100, 150, 200], 64, 64);
        assert!(psnr(&a, &a).is_infinite());
        assert!((ssim(&a, &a) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn mse_is_correct_for_known_offset() {
        let a = solid([0, 0, 0], 4, 4);
        let b = solid([10, 0, 0], 4, 4);
        assert!((mse(&a, &b) - 100.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn wildly_different_images_score_low_ssim() {
        let a = solid([0, 0, 0], 32, 32);
        let b = solid([255, 255, 255], 32, 32);
        assert!(ssim(&a, &b) < 0.05);
    }
}
