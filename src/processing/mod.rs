pub mod options;
pub mod presets;
pub mod save;
pub mod transform;
pub mod utils;
pub mod watermark;

use crate::monitoring::{increment_processed_images, observe_image_processing_duration};
use crate::processing::options::ParsedOptions;
use crate::processing::watermark::CachedWatermark;
use bytes::Bytes;
use libvips::VipsImage;
use std::time::Instant;
use thiserror::Error;
use tracing::debug;

/// Errors produced by the image processing pipeline.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProcessingError {
    #[error(transparent)]
    Transform(#[from] transform::TransformError),
    #[error(transparent)]
    Watermark(#[from] watermark::WatermarkError),
    #[error(transparent)]
    Save(#[from] save::SaveError),
    #[error("processed image would be {width}x{height}, over the {limit}px result dimension limit")]
    ResultTooLarge { width: i32, height: i32, limit: u32 },
}

/// The JPEG loader can decode at 1/2, 1/4 or 1/8 scale, skipping the work
/// rather than doing it and throwing the result away.
const MAX_LOAD_SHRINK: u32 = 8;

/// Picks a power-of-two shrink to apply while decoding, so a large source is
/// never unpacked at full resolution to produce a small result.
///
/// Follows imgproxy: take the *least* shrink any axis needs, so the decoded
/// image is still at least as large as the target on both axes and the real
/// resize can finish the job. Overshooting here would hand the pipeline a
/// source smaller than the request, which `enlarge:false` would then refuse to
/// scale back up.
///
/// Returns 1 when nothing should change.
pub fn load_shrink_factor(parsed_options: &ParsedOptions, src_width: u32, src_height: u32) -> u32 {
    // `raw` returns the source untouched, and a crop addresses source pixels by
    // coordinate — shrinking underneath it would move the region being cut.
    if parsed_options.raw || parsed_options.crop.is_some() {
        return 1;
    }
    let Some(resize) = parsed_options.resize.as_ref() else {
        return 1;
    };
    if src_width == 0 || src_height == 0 {
        return 1;
    }

    // Anything that can grow the target after this point has to be folded in,
    // or the shrink could drop the source below what the pipeline still needs.
    let grow =
        f64::from(parsed_options.dpr.unwrap_or(1.0).max(1.0)) * f64::from(parsed_options.zoom.unwrap_or(1.0).max(1.0));
    // `force` fills a zero axis from the *source* dimension, so that axis needs
    // the source at full size — shrinking would shrink the target with it. Every
    // other type derives a zero axis from the aspect ratio, which survives a
    // shrink unchanged.
    let forced = resize.resizing_type == "force";
    let width_follows_source = forced && resize.width == 0;
    let height_follows_source = forced && resize.height == 0;

    let target_width = if width_follows_source {
        f64::from(src_width)
    } else {
        (f64::from(resize.width) * grow).max(f64::from(parsed_options.min_width.unwrap_or(0)))
    };
    let target_height = if height_follows_source {
        f64::from(src_height)
    } else {
        (f64::from(resize.height) * grow).max(f64::from(parsed_options.min_height.unwrap_or(0)))
    };

    let mut shrink = f64::INFINITY;
    if target_width >= 1.0 {
        shrink = shrink.min(f64::from(src_width) / target_width);
    }
    if target_height >= 1.0 {
        shrink = shrink.min(f64::from(src_height) / target_height);
    }
    if !shrink.is_finite() || shrink < 2.0 {
        return 1;
    }

    let mut factor = 1;
    while factor * 2 <= MAX_LOAD_SHRINK && f64::from(factor * 2) <= shrink {
        factor *= 2;
    }
    factor
}

/// Processes an image by applying the given `ParsedOptions`.
///
/// This function takes a decoded `VipsImage`, the original source bytes, and a set of parsed options,
/// applies transformations like resizing, cropping, blurring, and format conversion, then returns the
/// processed image bytes.
///
/// # Arguments
///
/// * `img` - The decoded source image to transform.
/// * `parsed_options` - A `ParsedOptions` struct containing the desired transformations.
/// * `source_bytes` - The original image bytes used for EXIF and metadata-driven operations.
/// * `watermark` - Optional cached watermark to overlay on the source image.
///
/// # Returns
///
/// A `Result` containing the processed image bytes on success, or a typed processing error.
pub fn process_image(
    mut img: VipsImage,
    mut parsed_options: ParsedOptions,
    source_bytes: &Bytes,
    watermark: Option<&CachedWatermark>,
) -> Result<Bytes, ProcessingError> {
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

    debug!("Loaded image: {}x{}", img.get_width(), img.get_height());

    // Apply EXIF autorotation if enabled
    if parsed_options.auto_rotate {
        debug!("Applying EXIF auto-rotation");
        img = transform::apply_exif_rotation(source_bytes.as_ref(), img)?;
    }

    // Apply crop if specified
    if let Some(crop) = parsed_options.crop {
        debug!("Applying crop: {:?}", crop);
        img = transform::crop_image(img, crop)?;
    }

    // Apply resize if specified
    let mut resolved_resize_dims: Option<(u32, u32)> = None;
    if let Some(ref resize) = parsed_options.resize {
        let src_width = img.get_width() as u32;
        let src_height = img.get_height() as u32;
        let (target_w, target_h) = transform::resolve_resize_dimensions(resize, src_width, src_height)?;
        debug!(
            "Applying resize {:?} resolved to {}x{} from source {}x{}",
            resize, target_w, target_h, src_width, src_height
        );
        resolved_resize_dims = Some((target_w, target_h));

        // The enlargement cap lives inside apply_resize, per resizing type. It
        // used to be here, comparing the requested box against the source and
        // skipping the whole resize when either side was larger — which threw
        // away downscales that never enlarged anything.
        img = transform::apply_resize(
            img,
            resize,
            &parsed_options.gravity,
            parsed_options.resizing_algorithm.as_deref(),
            parsed_options.enlarge,
        )?;
    }

    // Apply min dimensions if specified
    if parsed_options.min_width.is_some() || parsed_options.min_height.is_some() {
        debug!(
            "Applying min dimensions: min_width={:?}, min_height={:?}",
            parsed_options.min_width, parsed_options.min_height
        );
        img = transform::apply_min_dimensions(
            img,
            parsed_options.min_width,
            parsed_options.min_height,
            parsed_options.resizing_algorithm.as_deref(),
        )?;
    }

    // Apply zoom if specified
    if let Some(zoom) = parsed_options.zoom {
        debug!("Applying zoom: {}", zoom);
        img = transform::apply_zoom(img, zoom, parsed_options.resizing_algorithm.as_deref())?;
    }

    // Apply extend if specified
    if parsed_options.extend {
        debug!("Applying extend option");
        if let Some((target_w, target_h)) = resolved_resize_dims {
            if img.get_width() < target_w as i32 || img.get_height() < target_h as i32 {
                let extend_w = target_w.max(img.get_width() as u32);
                let extend_h = target_h.max(img.get_height() as u32);
                img = transform::extend_image(
                    img,
                    extend_w,
                    extend_h,
                    &parsed_options.gravity,
                    &parsed_options.background,
                )?;
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

    // Apply flip if specified
    if let Some(flip) = parsed_options.flip {
        debug!("Applying flip: {:?}", flip);
        img = transform::apply_flip(img, flip)?;
    }

    // Apply color adjustments if specified
    if let Some(adjust) = parsed_options.adjust {
        debug!("Applying color adjustments: {:?}", adjust);
        img = transform::apply_adjust(img, adjust)?;
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
        img = transform::apply_pixelate(img, amount, parsed_options.resizing_algorithm.as_deref())?;
    }

    // Apply watermark if specified
    if let Some(ref watermark_opts) = parsed_options.watermark {
        if let Some(watermark) = watermark {
            debug!("Applying watermark with options: {:?}", watermark_opts);
            img = watermark::apply_watermark(
                img,
                watermark,
                watermark_opts,
                parsed_options.resizing_algorithm.as_deref(),
            )?;
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

    // Enforce the result-dimension ceiling before encoding. libvips has built a
    // pipeline but not materialised it yet, so the dimensions are already known
    // while the pixels are not — rejecting here avoids the allocation entirely
    // rather than reporting it afterwards.
    if let Some(limit) = parsed_options.max_result_dimension {
        let (width, height) = (img.get_width(), img.get_height());
        if width.max(height) as u32 > limit.get() {
            debug!(
                "Result {}x{} exceeds max_result_dimension {}",
                width,
                height,
                limit.get()
            );
            return Err(ProcessingError::ResultTooLarge {
                width,
                height,
                limit: limit.get(),
            });
        }
    }

    // Save image to bytes
    let quality = parsed_options
        .quality
        .or_else(|| parsed_options.save.format_quality.get(output_format).copied())
        .unwrap_or(85);
    let output_vec = save::save_image_with_options(img, output_format, quality, &parsed_options.save)?;
    let output_bytes = Bytes::from(output_vec);

    debug!("Image processing complete");

    let duration = start.elapsed().as_secs_f64();
    observe_image_processing_duration(output_format, duration);
    increment_processed_images(output_format);

    Ok(output_bytes)
}

#[cfg(test)]
mod tests;
