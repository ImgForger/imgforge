//! Colour and pixel effects: adjustment, blur, sharpen, pixelate, and
//! flattening onto a background.

use super::{resize_with_algorithm, vips, TransformError};
use crate::processing::options::Adjust;
use libvips::{ops, VipsImage};

/// Applies brightness, contrast, and saturation adjustments.
///
/// Brightness and contrast go through a single `vips_linear`, which computes
/// `a * in + b` per band. Contrast pivots around mid-grey so it darkens shadows
/// and brightens highlights rather than shifting the whole image, and the
/// brightness offset is folded into the same `b` — the result is contrast
/// applied first, then brightness, in one pass over the pixels.
pub fn apply_adjust(img: VipsImage, adjust: Adjust) -> Result<VipsImage, TransformError> {
    let mut current = img;

    if adjust.brightness != 0 || (adjust.contrast - 1.0).abs() > f32::EPSILON {
        current = apply_brightness_contrast(current, adjust.brightness, f64::from(adjust.contrast))?;
    }

    if (adjust.saturation - 1.0).abs() > f32::EPSILON {
        current = apply_saturation(current, adjust.saturation)?;
    }

    Ok(current)
}

/// How many units of the band format make up one 8-bit step.
///
/// The URL speaks in 8-bit terms — `brightness:64` means a quarter of the way
/// up the range — so a 16-bit source has to have that scaled up, or the same
/// URL would nudge a 16-bit image 256 times less than an 8-bit one.
fn channel_scale(img: &VipsImage) -> f64 {
    match img.get_format() {
        Ok(ops::BandFormat::Ushort) | Ok(ops::BandFormat::Short) => 257.0,
        _ => 1.0,
    }
}

fn apply_brightness_contrast(img: VipsImage, brightness: i16, contrast: f64) -> Result<VipsImage, TransformError> {
    let bands = usize::try_from(img.get_bands()).unwrap_or(0);
    if bands == 0 {
        return Ok(img);
    }

    let scale = channel_scale(&img);
    let midpoint = 128.0 * scale;
    let offset = midpoint * (1.0 - contrast) + f64::from(brightness) * scale;

    let mut multipliers = vec![contrast; bands];
    let mut adders = vec![offset; bands];

    // Alpha is opacity, not colour: brightening it would fade the image in or
    // out instead of lightening it.
    if img.image_hasalpha() {
        multipliers[bands - 1] = 1.0;
        adders[bands - 1] = 0.0;
    }

    let format = img.get_format();
    let adjusted =
        ops::linear(&img, &mut multipliers, &mut adders).map_err(vips("Error applying brightness and contrast"))?;

    // `linear` promotes to float to hold values that fall outside the input
    // range. Casting back clips them and keeps the rest of the pipeline, and
    // the encoder, on the format the source arrived in.
    match format {
        Ok(format) => ops::cast(&adjusted, format).map_err(vips("Error applying brightness and contrast")),
        Err(_) => Ok(adjusted),
    }
}

fn apply_saturation(img: VipsImage, saturation: f32) -> Result<VipsImage, TransformError> {
    if !saturation.is_finite() || saturation <= 0.0 {
        return Err(TransformError::invalid(
            "saturation",
            "saturation must be a finite positive number",
        ));
    }

    let bands = img.get_bands();
    if bands != 3 && bands != 4 {
        return Ok(img);
    }

    let s = f64::from(saturation);
    let inv = 1.0 - s;
    let rw = 0.2126;
    let gw = 0.7152;
    let bw = 0.0722;

    let (width, matrix) = if bands == 4 {
        (
            4,
            vec![
                rw * inv + s,
                gw * inv,
                bw * inv,
                0.0,
                rw * inv,
                gw * inv + s,
                bw * inv,
                0.0,
                rw * inv,
                gw * inv,
                bw * inv + s,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ],
        )
    } else {
        (
            3,
            vec![
                rw * inv + s,
                gw * inv,
                bw * inv,
                rw * inv,
                gw * inv + s,
                bw * inv,
                rw * inv,
                gw * inv,
                bw * inv + s,
            ],
        )
    };

    let matrix = VipsImage::image_new_matrix_from_array(width, width, &matrix)
        .map_err(vips("Error creating saturation matrix"))?;
    ops::recomb(&img, &matrix).map_err(vips("Error applying saturation"))
}

/// Composites an image with alpha over a solid background, dropping alpha.
///
/// Only RGB is used; the background's own alpha is ignored, because the point
/// of flattening is to produce an image that no longer has any.
pub fn flatten_onto_background(img: VipsImage, bg_color: [u8; 4]) -> Result<VipsImage, TransformError> {
    // Nothing to flatten without an alpha channel (4 bands for RGBA, 2 for
    // greyscale plus alpha).
    let bands = img.get_bands();
    if bands != 4 && bands != 2 {
        return Ok(img);
    }

    let bg = vec![f64::from(bg_color[0]), f64::from(bg_color[1]), f64::from(bg_color[2])];
    let opts = ops::FlattenOptions {
        background: bg,
        ..Default::default()
    };
    ops::flatten_with_opts(&img, &opts).map_err(vips("Error applying background color"))
}

/// Applies background color to an image (useful for JPEG output).
pub fn apply_background_color(img: VipsImage, bg_color: [u8; 4]) -> Result<VipsImage, TransformError> {
    flatten_onto_background(img, bg_color)
}

/// Applies blur to an image.
pub fn apply_blur(img: VipsImage, sigma: f32) -> Result<VipsImage, TransformError> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(TransformError::invalid(
            "blur",
            "blur sigma must be a finite positive number",
        ));
    }
    ops::gaussblur(&img, f64::from(sigma)).map_err(vips("Error applying blur"))
}

/// Sharpens an image.
pub fn apply_sharpen(img: VipsImage, sigma: f32) -> Result<VipsImage, TransformError> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(TransformError::invalid(
            "sharpen",
            "sharpen sigma must be a finite positive number",
        ));
    }
    let clamped_sigma = sigma.clamp(0.1, 10.0);
    let opts = ops::SharpenOptions {
        sigma: f64::from(clamped_sigma),
        ..Default::default()
    };
    ops::sharpen_with_opts(&img, &opts).map_err(vips("Error applying sharpen"))
}

/// Pixelates an image.
pub fn apply_pixelate(img: VipsImage, amount: u32) -> Result<VipsImage, TransformError> {
    if amount <= 1 {
        return Ok(img);
    }
    let (w, h) = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);
    if w == 0 || h == 0 {
        return Ok(img);
    }

    let target_w = (w / amount).max(1);
    let target_h = (h / amount).max(1);
    let pixelated = resize_with_algorithm(
        &img,
        f64::from(target_w) / f64::from(w),
        Some(f64::from(target_h) / f64::from(h)),
        Some("nearest"),
        "Error pixelating (down)",
    )?;
    resize_with_algorithm(
        &pixelated,
        f64::from(w) / f64::from(pixelated.get_width()),
        Some(f64::from(h) / f64::from(pixelated.get_height())),
        Some("nearest"),
        "Error pixelating (up)",
    )
}
