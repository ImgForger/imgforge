pub mod options;
pub mod save;
pub mod transform;
pub mod utils;

use crate::monitoring::{IMAGE_PROCESSING_DURATION_SECONDS, PROCESSED_IMAGES_TOTAL};
use crate::processing::options::ParsedOptions;
use bytes::Bytes;
use libvips::VipsImage;
use std::time::Instant;
use tracing::{debug, error};

/// Processes an image by applying the given `ParsedOptions`.
///
/// This function takes raw image bytes and a set of parsed options, applies
/// transformations like resizing, cropping, blurring, and format conversion,
/// then returns the processed image bytes.
///
/// # Arguments
///
/// * `image_bytes` - The raw bytes of the image to process.
/// * `parsed_options` - A `ParsedOptions` struct containing the desired transformations.
///
/// # Returns
///
/// A `Result` containing the processed image bytes on success, or an error message as a `String`.
pub async fn process_image(
    image_bytes: Vec<u8>,
    mut parsed_options: ParsedOptions,
    watermark_bytes: Option<&Bytes>,
) -> Result<Vec<u8>, String> {
    let start = Instant::now();
    debug!("Starting image processing with options: {:?}", parsed_options);

    // Apply DPR scaling
    if let Some(dpr) = parsed_options.dpr {
        if dpr > 1.0 {
            debug!("Applying DPR scaling: {}", dpr);
            if let Some(ref mut resize) = parsed_options.resize {
                debug!(
                    "Scaling resize dimensions from {}x{} to {}x{}",
                    resize.width,
                    resize.height,
                    (resize.width as f32 * dpr).round() as u32,
                    (resize.height as f32 * dpr).round() as u32
                );
                resize.width = (resize.width as f32 * dpr).round() as u32;
                resize.height = (resize.height as f32 * dpr).round() as u32;
            }
            if let Some(ref mut padding) = parsed_options.padding {
                debug!(
                    "Scaling padding from {:?} to {:?}",
                    padding,
                    (
                        (padding.0 as f32 * dpr).round() as u32,
                        (padding.1 as f32 * dpr).round() as u32,
                        (padding.2 as f32 * dpr).round() as u32,
                        (padding.3 as f32 * dpr).round() as u32
                    )
                );
                padding.0 = (padding.0 as f32 * dpr).round() as u32;
                padding.1 = (padding.1 as f32 * dpr).round() as u32;
                padding.2 = (padding.2 as f32 * dpr).round() as u32;
                padding.3 = (padding.3 as f32 * dpr).round() as u32;
            }
        }
    }

    // Load image from bytes
    let mut img = VipsImage::new_from_buffer(&image_bytes, "").map_err(|e| {
        error!("Error loading image from memory: {}", e);
        format!("Error loading image from memory: {}", e)
    })?;

    debug!("Loaded image: {}x{}", img.get_width(), img.get_height());

    // Apply EXIF auto-rotation if enabled
    if parsed_options.auto_rotate {
        debug!("Applying EXIF auto-rotation");
        img = transform::apply_exif_rotation(&image_bytes, img)?;
    }

    // Apply crop if specified
    if let Some(crop) = parsed_options.crop {
        debug!("Applying crop: {:?}", crop);
        img = transform::crop_image(img, crop)?;
    }

    // Apply resize if specified
    if let Some(ref resize) = parsed_options.resize {
        debug!("Applying resize: {:?}", resize);
        let (w, h) = (resize.width, resize.height);

        if !parsed_options.enlarge && (w > img.get_width() as u32 || h > img.get_height() as u32) {
            debug!("Not enlarging image as enlarge is false and target dimensions are larger than source");
        } else {
            img = transform::apply_resize(img, resize, &parsed_options.gravity)?;
        }
    }

    // Apply min dimensions if specified
    if parsed_options.min_width.is_some() || parsed_options.min_height.is_some() {
        debug!(
            "Applying min dimensions: min_width={:?}, min_height={:?}",
            parsed_options.min_width, parsed_options.min_height
        );
        img = transform::apply_min_dimensions(img, parsed_options.min_width, parsed_options.min_height)?;
    }

    // Apply zoom if specified
    if let Some(zoom) = parsed_options.zoom {
        debug!("Applying zoom: {}", zoom);
        img = transform::apply_zoom(img, zoom)?;
    }

    // Apply extend if specified
    if parsed_options.extend {
        debug!("Applying extend option");
        if let Some(resize) = &parsed_options.resize {
            let (w, h) = (resize.width, resize.height);
            if img.get_width() < w as i32 || img.get_height() < h as i32 {
                img = transform::extend_image(img, w, h, &parsed_options.gravity, &parsed_options.background)?;
            }
        }
    }

    // Apply padding if specified
    if let Some((top, right, bottom, left)) = parsed_options.padding {
        debug!("Applying padding: {:?}", (top, right, bottom, left));
        img = transform::apply_padding(img, top, right, bottom, left, &parsed_options.background)?;
    }

    // Apply rotation if specified
    if let Some(rotation) = parsed_options.rotation {
        debug!("Applying rotation: {}", rotation);
        img = transform::apply_rotation(img, rotation)?;
    }

    // Apply blur if specified
    if let Some(sigma) = parsed_options.blur {
        debug!("Applying blur with sigma: {}", sigma);
        img = transform::apply_blur(img, sigma)?;
    }

    // Apply sharpen if specified
    if let Some(sigma) = parsed_options.sharpen {
        debug!("Applying sharpen with sigma: {}", sigma);
        img = transform::apply_sharpen(img, sigma)?;
    }

    // Apply pixelate if specified
    if let Some(amount) = parsed_options.pixelate {
        debug!("Applying pixelate with amount: {}", amount);
        img = transform::apply_pixelate(img, amount)?;
    }

    // Apply watermark if specified
    if let Some(ref watermark_opts) = parsed_options.watermark {
        if let Some(watermark_bytes) = watermark_bytes {
            debug!("Applying watermark with options: {:?}", watermark_opts);
            img = transform::apply_watermark(img, watermark_bytes, watermark_opts)?;
        }
    }

    // Apply background color for JPEG if needed
    let output_format = parsed_options.format.as_deref().unwrap_or("jpeg");
    if let Some(bg_color) = parsed_options.background {
        if output_format == "jpeg" {
            debug!("Applying background color for JPEG output: {:?}", bg_color);
            img = transform::apply_background_color(img, bg_color)?;
        }
    }

    // Save image to bytes
    let quality = parsed_options.quality.unwrap_or(85);
    let output_bytes = save::save_image(img, output_format, quality)?;

    debug!("Image processing complete");

    let duration = start.elapsed().as_secs_f64();
    IMAGE_PROCESSING_DURATION_SECONDS
        .with_label_values(&[output_format])
        .observe(duration);
    PROCESSED_IMAGES_TOTAL.with_label_values(&[output_format]).inc();

    Ok(output_bytes)
}

#[cfg(test)]
mod tests;
