//! The ordered sequence of transformations applied to a single frame.
//!
//! Animated sources run every frame through [`transform_frame`] separately, so
//! this stays a pure image-in/image-out step with no knowledge of how many
//! frames there are.

use crate::processing::options::{ParsedOptions, ResizingType};
use crate::processing::transform::{self, TransformError};
use crate::processing::watermark::{self, CachedWatermark, WatermarkError};
use libvips::VipsImage;
use thiserror::Error;
use tracing::debug;

/// Errors produced while transforming a frame.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PipelineError {
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Watermark(#[from] WatermarkError),
}

/// The dimensions the resize was asked for, kept so the padding steps that run
/// after it know what box the image was meant to fill.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameTargets {
    pub resize: Option<(u32, u32)>,
}

/// Applies every geometry and pixel operation the request asks for, in the
/// order imgproxy applies them.
pub fn transform_frame(
    mut img: VipsImage,
    options: &ParsedOptions,
    watermark_source: Option<&CachedWatermark>,
) -> Result<VipsImage, PipelineError> {
    let mut targets = FrameTargets::default();

    // Trim before anything that depends on the image's extent: the borders it
    // removes would otherwise skew the crop window and the resize target.
    if let Some(trim) = options.trim.as_ref() {
        debug!("Applying trim: {:?}", trim);
        img = transform::apply_trim(img, trim)?;
    }

    if let Some(crop) = options.crop.as_ref() {
        debug!("Applying crop: {:?}", crop);
        img = transform::crop_image(img, crop, &options.crop_gravity())?;
    }

    if let Some(resize) = options.resize.as_ref() {
        let src_width = img.get_width().max(0) as u32;
        let src_height = img.get_height().max(0) as u32;
        let (target_w, target_h) = transform::resolve_resize_dimensions(resize, src_width, src_height)?;
        debug!(
            "Applying resize {:?} resolved to {}x{} from source {}x{}",
            resize, target_w, target_h, src_width, src_height
        );
        targets.resize = Some((target_w, target_h));

        // The enlargement cap lives inside apply_resize, per resizing type. It
        // used to be here, comparing the requested box against the source and
        // skipping the whole resize when either side was larger — which threw
        // away downscales that never enlarged anything.
        img = transform::apply_resize(
            img,
            resize,
            &options.fill_gravity(),
            options.resizing_algorithm.as_deref(),
            options.enlarge,
            f64::from(options.dpr_factor()),
        )?;
    }

    if options.min_width.is_some() || options.min_height.is_some() {
        debug!(
            "Applying min dimensions: min_width={:?}, min_height={:?}",
            options.min_width, options.min_height
        );
        img = transform::apply_min_dimensions(
            img,
            options.min_width,
            options.min_height,
            options.resizing_algorithm.as_deref(),
        )?;
    }

    let zoom = options.zoom_factors();
    if !zoom.is_identity() {
        debug!("Applying zoom: {:?}", zoom);
        img = transform::apply_zoom(img, zoom, options.resizing_algorithm.as_deref())?;
    }

    img = apply_extend(img, options, &targets)?;

    if let Some((top, right, bottom, left)) = options.padding {
        debug!("Applying padding: {:?}", (top, right, bottom, left));
        img = transform::apply_padding(img, top, right, bottom, left, &options.background)?;
    }

    if let Some(rotation) = options.rotation {
        debug!("Applying rotation: {}", rotation);
        img = transform::apply_rotation(img, rotation)?;
    }

    if let Some(flip) = options.flip {
        debug!("Applying flip: {:?}", flip);
        img = transform::apply_flip(img, flip)?;
    }

    if let Some(adjust) = options.adjust.filter(|adjust| !adjust.is_identity()) {
        debug!("Applying color adjustments: {:?}", adjust);
        img = transform::apply_adjust(img, adjust)?;
    }

    if let Some(sigma) = options.blur {
        debug!("Applying blur with sigma: {}", sigma);
        img = transform::apply_blur(img, sigma)?;
    }

    if let Some(sigma) = options.sharpen {
        debug!("Applying sharpen with sigma: {}", sigma);
        img = transform::apply_sharpen(img, sigma)?;
    }

    if let Some(amount) = options.pixelate {
        debug!("Applying pixelate with amount: {}", amount);
        img = transform::apply_pixelate(img, amount)?;
    }

    if let (Some(watermark_opts), Some(source)) = (options.watermark.as_ref(), watermark_source) {
        debug!("Applying watermark with options: {:?}", watermark_opts);
        img = watermark::apply_watermark(img, source, watermark_opts, options.resizing_algorithm.as_deref())?;
    }

    Ok(img)
}

/// Runs `extend` and then `extend_aspect_ratio`.
///
/// `extend` pads out to the requested pixel box; `extend_aspect_ratio` pads out
/// to the requested *shape* without reaching that box. imgproxy runs both, in
/// this order, and a request may set either or both.
fn apply_extend(
    mut img: VipsImage,
    options: &ParsedOptions,
    targets: &FrameTargets,
) -> Result<VipsImage, PipelineError> {
    let Some((target_w, target_h)) = targets.resize else {
        return Ok(img);
    };

    // `force` already hit the box exactly, so there is nothing left to pad —
    // and its target is the source's own size on any zero axis, which would
    // make an aspect-ratio extend meaningless.
    let forced = options
        .resize
        .as_ref()
        .is_some_and(|resize| resize.resizing_type == ResizingType::Force);

    if options.extend.enabled {
        debug!("Extending to {}x{}", target_w, target_h);
        let gravity = options.extend.gravity.unwrap_or_default();
        img = transform::extend_image(
            img,
            target_w,
            target_h,
            &gravity,
            &options.background,
            f64::from(options.dpr_factor()),
        )?;
    }

    if options.extend_aspect_ratio.enabled && !forced {
        debug!("Extending to aspect ratio {}:{}", target_w, target_h);
        let gravity = options.extend_aspect_ratio.gravity.unwrap_or_default();
        img = transform::extend_to_aspect_ratio(
            img,
            target_w,
            target_h,
            &gravity,
            &options.background,
            f64::from(options.dpr_factor()),
        )?;
    }

    Ok(img)
}
