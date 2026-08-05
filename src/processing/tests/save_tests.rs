use crate::processing::options::SaveOptions;
use crate::processing::save;
use image::{ImageBuffer, Rgb};
use libvips::VipsImage;

use super::tests_support::*;

/// Deterministic textured image — flat colors encode to the same few
/// bytes at any quality, so quality assertions need detail to discard.
fn create_textured_image(width: u32, height: u32) -> Vec<u8> {
    let mut state: u32 = 0x2545_f491;
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (_x, _y, pixel) in img.enumerate_pixels_mut() {
        // xorshift32 — deterministic, no rand dependency
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *pixel = Rgb([
            (state & 0xff) as u8,
            ((state >> 8) & 0xff) as u8,
            ((state >> 16) & 0xff) as u8,
        ]);
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

#[test]
fn test_webp_save_honors_quality() {
    init_vips();
    // Quality must change the encoded output: a low-quality save has to
    // be smaller than a high-quality one (issue #46 — quality was
    // silently dropped for WebP).
    let base = create_textured_image(400, 400);
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let low = save::save_image(img, "webp", 20).unwrap();
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let high = save::save_image(img, "webp", 95).unwrap();

    assert!(!low.is_empty());
    assert!(
        low.len() < high.len(),
        "webp quality ignored: q20 is {} bytes, q95 is {} bytes",
        low.len(),
        high.len()
    );
}

#[test]
fn test_webp_save_lossless_applies() {
    init_vips();
    // Lossless output of a noisy source is much larger than a lossy
    // encode — if the option were dropped, the sizes would match.
    let base = create_textured_image(400, 400);
    let mut options = SaveOptions::default();
    options.webp.lossless = Some(true);

    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let lossless = save::save_image_with_options(img, "webp", 80, &options).unwrap();
    let img = VipsImage::new_from_buffer(&base, "").unwrap();
    let lossy = save::save_image(img, "webp", 80).unwrap();

    assert!(!lossless.is_empty());
    assert!(
        lossless.len() > lossy.len(),
        "webp lossless option had no effect: lossless {} bytes, lossy {} bytes",
        lossless.len(),
        lossy.len()
    );
}
