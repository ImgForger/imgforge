//! The ordered sequence of transformations applied to a single frame.
//!
//! The order is imgproxy's `mainPipeline`, and it is not arbitrary. Three
//! places in it are load-bearing:
//!
//! - **Rotation sits between the scale and the result crop.** The crop window
//!   and the requested size describe the image the caller receives, so they
//!   have to be applied in the final orientation. Cropping first would make
//!   `resize:fill:800:600/rotate:90` return 600x800.
//! - **Filters run before extend and padding.** A blur applied afterwards
//!   convolves across the border, bleeding the pad colour into the image and
//!   the image into the pad.
//! - **Flattening happens before the watermark.** A semi-transparent watermark
//!   should composite onto the finished picture, not onto transparency that is
//!   about to be filled in behind it.
//!
//! Animated sources run every frame through [`transform_frame`] separately, so
//! this stays a pure image-in/image-out step with no knowledge of how many
//! frames there are.

use crate::processing::options::{Crop, Flip, ParsedOptions};
use crate::processing::transform::{self, ResizePlan, TransformError};
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
    #[error("processed image would be {width}x{height}, over the {limit}px result dimension limit")]
    ResultTooLarge { width: i32, height: i32, limit: u32 },
}

/// Everything a frame needs that is not a processing option.
///
/// The format-dependent decisions are resolved once by the caller, so this
/// module never has to know which encoder the result is headed for.
#[derive(Clone, Copy, Default)]
pub struct FrameContext<'a> {
    pub watermark: Option<&'a CachedWatermark>,
    /// Background to composite onto, when the output format has no alpha.
    pub flatten_background: Option<[u8; 4]>,
    /// Largest side the output container can address.
    pub max_dimension: Option<u32>,
}

/// Applies every geometry and pixel operation the request asks for.
pub fn transform_frame(
    mut img: VipsImage,
    options: &ParsedOptions,
    context: FrameContext<'_>,
) -> Result<VipsImage, PipelineError> {
    // Trim before anything that depends on the image's extent: the borders it
    // removes would otherwise skew the crop window and the resize target.
    if let Some(trim) = options.trim.as_ref() {
        debug!("Applying trim: {:?}", trim);
        img = transform::apply_trim(img, trim)?;
    }

    let rotation = options.rotation.unwrap_or(0);
    let flip = options.flip.unwrap_or_default();
    // A right-angle rotation swaps what the caller means by width and height.
    // EXIF orientation is already applied by this point, so the explicit
    // `rotate` is the only thing left that can transpose the image.
    let transposes = rotation % 180 == 90;

    img = apply_crop(img, options, rotation, flip, transposes)?;

    let plan = plan_and_scale(&mut img, options, transposes)?;

    if let Some(rotation) = options.rotation {
        debug!("Applying rotation: {}", rotation);
        img = transform::apply_rotation(img, rotation)?;
    }
    if let Some(flip) = options.flip {
        debug!("Applying flip: {:?}", flip);
        img = transform::apply_flip(img, flip)?;
    }

    // The result crop, the minimums and the zoom all describe the image the
    // caller receives, so they run once it is the right way up.
    if let Some(window) = plan.and_then(|plan| plan.result_crop) {
        debug!("Cropping to the result window {:?}", window);
        img = transform::crop_to_result(img, window, &options.fill_gravity(), f64::from(options.dpr_factor()))?;
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

    img = apply_filters(img, options)?;
    img = apply_canvas(img, options, plan)?;

    // Format limits and flattening close out the picture, before anything is
    // laid over the top of it.
    // The configured ceiling is policy, not fitting, and policy runs first: a
    // frame over `max_result_dimension` is refused. Letting the container cap
    // below quietly scale it down turned that refusal into acceptance — a
    // 20,000px result under an 18,000px ceiling came back as 16,383px instead
    // of the documented 400.
    if let Some(limit) = options.max_result_dimension {
        let (width, height) = (img.get_width(), img.get_height());
        if width.max(height) as u32 > limit.get() {
            return Err(PipelineError::ResultTooLarge {
                width,
                height,
                limit: limit.get(),
            });
        }
    }

    if let Some(limit) = context.max_dimension {
        img = fit_within(img, limit, options.resizing_algorithm.as_deref())?;
    }
    if let Some(background) = context.flatten_background {
        debug!("Flattening onto {:?}", background);
        img = transform::apply_background_color(img, background)?;
    }

    if let (Some(watermark_opts), Some(source)) = (options.watermark.as_ref(), context.watermark) {
        debug!("Applying watermark with options: {:?}", watermark_opts);
        let placement = watermark::WatermarkPlacement {
            size: options.watermark_size,
            rotate: options.watermark_rotate,
            offset_scale: f64::from(options.dpr_factor()),
            resizing_algorithm: options.resizing_algorithm.as_deref(),
        };
        img = watermark::apply_watermark(img, source, watermark_opts, placement)?;
    }

    Ok(img)
}

/// Applies the explicit `crop`, compensating for the rotation still to come.
///
/// Cropping before rotating is the cheap order — cutting pixels costs less than
/// turning them and then cutting — but the caller's extents and gravity name
/// the image they get back. Both are therefore rewritten into the stored
/// orientation, which is what makes the cheap order produce the expected
/// result. imgproxy does exactly this in its `crop` stage.
fn apply_crop(
    img: VipsImage,
    options: &ParsedOptions,
    rotation: u16,
    flip: Flip,
    transposes: bool,
) -> Result<VipsImage, PipelineError> {
    let Some(crop) = options.crop.as_ref() else {
        return Ok(img);
    };

    let mut gravity = options.crop_gravity();
    gravity.rotate_and_flip(rotation, flip.horizontal, flip.vertical);

    let crop = if transposes {
        Crop {
            width: crop.height,
            height: crop.width,
            gravity: crop.gravity,
        }
    } else {
        *crop
    };

    debug!("Applying crop: {:?} with gravity {:?}", crop, gravity);
    Ok(transform::crop_image(
        img,
        &crop,
        &gravity,
        options.crop_aspect_ratio.map(|ratio| ratio.transposed(transposes)),
    )?)
}

/// Plans the resize against the final orientation and applies its scale half.
fn plan_and_scale(
    img: &mut VipsImage,
    options: &ParsedOptions,
    transposes: bool,
) -> Result<Option<ResizePlan>, PipelineError> {
    let Some(resize) = options.resize.as_ref() else {
        return Ok(None);
    };

    let (width, height) = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);
    // Measured as the caller will see it, which is the transposed shape when a
    // right-angle rotation is still to come.
    let source = if transposes { (height, width) } else { (width, height) };

    let plan = transform::plan_resize(source, resize, options.enlarge, transposes)?;
    debug!("Resize {:?} planned as {:?} from source {:?}", resize, plan, source);

    let scaled = transform::apply_scale(
        std::mem::replace(img, VipsImage::new()),
        &plan,
        options.resizing_algorithm.as_deref(),
    )?;
    *img = scaled;

    Ok(Some(plan))
}

/// Colour and pixel effects, in the order imgproxy's `applyFilters` runs them.
fn apply_filters(mut img: VipsImage, options: &ParsedOptions) -> Result<VipsImage, PipelineError> {
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

    // The tone effects recolour the finished picture, so they close out the
    // filter stage.
    if let Some(monochrome) = options.monochrome {
        debug!("Applying monochrome: {:?}", monochrome);
        img = transform::apply_monochrome(img, monochrome)?;
    }

    if let Some(duotone) = options.duotone {
        debug!("Applying duotone: {:?}", duotone);
        img = transform::apply_duotone(img, duotone)?;
    }

    if let Some(colorize) = options.colorize {
        debug!("Applying colorize: {:?}", colorize);
        img = transform::apply_colorize(img, colorize)?;
    }

    Ok(img)
}

/// Grows the canvas: `extend`, then `extend_aspect_ratio`, then `padding`.
///
/// `extend` pads out to the requested pixel box; `extend_aspect_ratio` pads out
/// to the requested *shape* without reaching that box. A request may set either
/// or both, and with both set the second has nothing left to do.
fn apply_canvas(
    mut img: VipsImage,
    options: &ParsedOptions,
    plan: Option<ResizePlan>,
) -> Result<VipsImage, PipelineError> {
    if let Some(plan) = plan {
        let (target_w, target_h) = plan.target;

        // `force` already hit the box exactly, so there is nothing left to pad,
        // and its target is the source's own size on any zero axis — which
        // would make an aspect-ratio extend meaningless.
        let forced = options
            .resize
            .as_ref()
            .is_some_and(|resize| resize.resizing_type.fills_zero_axis_from_source());

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
    }

    if let Some((top, right, bottom, left)) = options.padding {
        debug!("Applying padding: {:?}", (top, right, bottom, left));
        img = transform::apply_padding(img, top, right, bottom, left, &options.background)?;
    }

    Ok(img)
}

/// Scales a frame down to what the output container can address.
///
/// WebP cannot represent a side over 16383 and the HEIF family stops at 16384.
/// Handing the encoder something larger fails at the very end, with a message
/// about the codec rather than about the size, so a request that is merely too
/// big for its chosen format looks like a server fault.
fn fit_within(img: VipsImage, limit: u32, resizing_algorithm: Option<&str>) -> Result<VipsImage, PipelineError> {
    let largest = img.get_width().max(img.get_height());
    let Ok(largest) = u32::try_from(largest) else {
        return Ok(img);
    };
    if largest <= limit {
        return Ok(img);
    }

    let scale = f64::from(limit) / f64::from(largest);
    debug!(
        "Rescaling by {:.4} so a {}px frame fits the {}px limit",
        scale, largest, limit
    );
    Ok(transform::resize_with_algorithm(
        &img,
        scale,
        None,
        resizing_algorithm,
        "Error fitting the result to the output format",
    )?)
}
