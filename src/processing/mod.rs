//! Image processing: what happens between a decoded source and encoded bytes.

pub mod animation;
pub mod colorspace;
pub mod metadata;
pub mod options;
pub mod pipeline;
pub mod presets;
pub mod save;
pub mod scale_on_load;
pub mod transform;
pub mod utils;
pub mod watermark;

use crate::monitoring::{increment_processed_images, observe_image_processing_duration};
use crate::processing::options::ParsedOptions;
use crate::processing::pipeline::PipelineError;
use crate::processing::watermark::CachedWatermark;
use bytes::Bytes;
use libvips::VipsImage;
use std::time::Instant;
use thiserror::Error;
use tracing::debug;

pub use scale_on_load::{load_scale_factor, load_shrink_factor, thumbnail_covers};

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
    #[error("animation frame is {width}x{height}, over the {limit} pixel frame limit")]
    FrameTooLarge { width: i32, height: i32, limit: u64 },
}

impl From<PipelineError> for ProcessingError {
    fn from(error: PipelineError) -> Self {
        match error {
            PipelineError::Transform(error) => Self::Transform(error),
            PipelineError::Watermark(error) => Self::Watermark(error),
        }
    }
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
    img: VipsImage,
    mut parsed_options: ParsedOptions,
    source_bytes: &Bytes,
    watermark: Option<&CachedWatermark>,
) -> Result<Bytes, ProcessingError> {
    let start = Instant::now();
    debug!("Starting image processing with options: {:?}", parsed_options);

    apply_dpr(&mut parsed_options);

    debug!("Loaded image: {}x{}", img.get_width(), img.get_height());

    let output_format = parsed_options.format.as_deref().unwrap_or("jpeg").to_string();

    // Colour management runs before the frames are split so a CMYK or
    // wide-gamut source is converted once rather than once per frame.
    let keep_high_bit_depth =
        parsed_options.save.preserve_hdr.unwrap_or(false) && save::format_supports_high_bit_depth(&output_format);
    let img = colorspace::to_processing(img, keep_high_bit_depth)?;

    let orientation = parsed_options
        .auto_rotate
        .then(|| crate::utils::read_exif_orientation(source_bytes))
        .flatten();

    let frames = animation::split(&img)?;
    enforce_frame_limit(&parsed_options, &frames, source_bytes)?;

    let processed = frames
        .images
        .into_iter()
        .map(|mut frame| {
            if let Some(orientation) = orientation {
                frame = transform::apply_exif_orientation(frame, orientation)?;
            }
            Ok(pipeline::transform_frame(frame, &parsed_options, watermark)?)
        })
        .collect::<Result<Vec<_>, ProcessingError>>()?;

    let (mut img, page_height) = animation::join(processed)?;

    img = colorspace::to_result(img, save::format_supports_color_profile(&output_format))?;

    // A format without an alpha channel needs the transparency resolved before
    // it reaches the encoder, or the alpha is dropped against whatever happens
    // to be underneath it.
    if let Some(bg_color) = parsed_options.background {
        if !save::format_supports_alpha(&output_format) {
            debug!("Flattening onto {:?} for {} output", bg_color, output_format);
            img = transform::apply_background_color(img, bg_color)?;
        }
    }

    enforce_result_dimension(&parsed_options, &img)?;

    let quality = parsed_options
        .quality
        .or_else(|| parsed_options.save.format_quality.get(&output_format).copied())
        .unwrap_or(85);
    let mut output_vec = save::save_image_with_options(
        img,
        &output_format,
        quality,
        &parsed_options.save,
        page_height.filter(|_| save::format_supports_animation(&output_format)),
    )?;

    if parsed_options.save.retains_copyright() {
        let copyright = metadata::read_copyright(source_bytes);
        if !copyright.is_empty() {
            debug!("Re-attaching copyright after metadata strip");
            output_vec = metadata::attach_copyright(output_vec, &copyright);
        }
    }

    let output_bytes = Bytes::from(output_vec);

    debug!("Image processing complete");

    let duration = start.elapsed().as_secs_f64();
    observe_image_processing_duration(&output_format, duration);
    increment_processed_images(&output_format);

    Ok(output_bytes)
}

/// Scales everything the device pixel ratio applies to.
///
/// DPR multiplies the *requested* geometry rather than the result, so it has to
/// land on the resize target and the padding before either is used. Applying it
/// afterwards would scale the padding along with the image, which is not what a
/// 2x display asks for.
fn apply_dpr(parsed_options: &mut ParsedOptions) {
    let dpr = parsed_options.dpr_factor();
    if dpr <= 1.0 {
        return;
    }

    debug!("Applying DPR scaling: {}", dpr);
    if let Some(resize) = parsed_options.resize.as_mut() {
        resize.width = (resize.width as f32 * dpr).round() as u32;
        resize.height = (resize.height as f32 * dpr).round() as u32;
    }
    if let Some(padding) = parsed_options.padding.as_mut() {
        padding.0 = (padding.0 as f32 * dpr).round() as u32;
        padding.1 = (padding.1 as f32 * dpr).round() as u32;
        padding.2 = (padding.2 as f32 * dpr).round() as u32;
        padding.3 = (padding.3 as f32 * dpr).round() as u32;
    }
}

/// Rejects an animation whose individual frames are too large.
///
/// The source-resolution limit measures the whole stack, which for an animation
/// is the frame size multiplied by the frame count; this bounds what a single
/// frame may cost, which is what imgproxy's `max_animation_frame_resolution`
/// does.
fn enforce_frame_limit(
    parsed_options: &ParsedOptions,
    frames: &animation::Frames,
    source_bytes: &Bytes,
) -> Result<(), ProcessingError> {
    let Some(limit) = parsed_options.max_animation_frame_resolution else {
        return Ok(());
    };
    let Some(frame) = frames.images.first() else {
        return Ok(());
    };

    // How many frames are in hand is not the question. `disable_animation`, a
    // still output format and an explicit `pages:1` all collapse an animated
    // source to a single frame, and that frame is still an animation frame —
    // treating it as a still image would let any of the three ask for the first
    // frame of an enormous animation and be handed it. Only the source can say
    // whether this is an animation, so when one frame is in hand it is the
    // source that gets asked.
    if frames.images.len() <= 1 && !source_is_animated(source_bytes) {
        return Ok(());
    }

    let (width, height) = (frame.get_width(), frame.get_height());
    let pixels = u64::try_from(width)
        .unwrap_or(0)
        .saturating_mul(u64::try_from(height).unwrap_or(0));
    if pixels > limit.pixels() {
        return Err(ProcessingError::FrameTooLarge {
            width,
            height,
            limit: limit.pixels(),
        });
    }

    Ok(())
}

/// Whether the source itself carries more than one page.
///
/// Reopening reads the header and decodes nothing, and this is only reached
/// once the operator has configured the per-frame limit, so the cost lands on
/// the deployments that asked for the check. A source that will not reopen is
/// not treated as an animation: it is about to fail for its own reasons, and
/// guessing here would turn a decode error into a limit error.
fn source_is_animated(source_bytes: &Bytes) -> bool {
    VipsImage::new_from_buffer(source_bytes, "n=-1")
        .map(|img| img.get_n_pages() > 1)
        .unwrap_or(false)
}

/// Enforces the result-dimension ceiling before encoding.
///
/// libvips has built a pipeline but not materialised it yet, so the dimensions
/// are already known while the pixels are not — rejecting here avoids the
/// allocation entirely rather than reporting it afterwards.
fn enforce_result_dimension(parsed_options: &ParsedOptions, img: &VipsImage) -> Result<(), ProcessingError> {
    let Some(limit) = parsed_options.max_result_dimension else {
        return Ok(());
    };

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

    Ok(())
}

#[cfg(test)]
mod tests;
