//! Positioning and canvas sizing: where a window sits, and how the image is
//! padded out to a larger one.

use super::{bg_color_for_bands, vips, TransformError, VIPS_MAX_COORD};
use crate::processing::options::{Crop, CropAspectRatio, Gravity, GravityType};
use libvips::{ops, VipsImage};
use tracing::debug;

/// Rounds to the nearest even integer.
///
/// imgproxy aligns every computed offset this way so that a crop never lands on
/// an odd boundary, which would shift the chroma planes of a subsampled source
/// by half a pixel and tint the edge.
fn round_to_even(value: f64) -> i64 {
    if !value.is_finite() {
        return 0;
    }
    ((value / 2.0).round() * 2.0) as i64
}

/// Scales `extent` by `factor` and rounds the result to an even integer.
fn scale_to_even(extent: i64, factor: f64) -> i64 {
    round_to_even(extent as f64 * factor)
}

/// Halves `value`, rounding toward zero and then down to an even number.
fn half_to_even(value: i64) -> i64 {
    let halved = value / 2;
    halved - (halved % 2)
}

/// Where an `inner_width` x `inner_height` window sits inside a
/// `width` x `height` canvas, given a gravity.
///
/// Mirrors imgproxy's `calcPosition`. Offsets are absolute pixels once their
/// magnitude reaches 1 and a fraction of the axis below it; `offset_scale`
/// scales the absolute form so a DPR-aware request nudges by the same visual
/// distance. `allow_overflow` lets the window hang off the canvas, which
/// watermarking needs and cropping must not have.
pub fn calc_position(
    width: i64,
    height: i64,
    inner_width: i64,
    inner_height: i64,
    gravity: &Gravity,
    offset_scale: f64,
    allow_overflow: bool,
) -> (i64, i64) {
    let (mut left, mut top) = if gravity.kind == GravityType::FocusPoint {
        (
            scale_to_even(width, gravity.x) - inner_width / 2,
            scale_to_even(height, gravity.y) - inner_height / 2,
        )
    } else {
        let offset_x = if gravity.x.abs() >= 1.0 {
            round_to_even(gravity.x * offset_scale)
        } else {
            scale_to_even(width, gravity.x)
        };
        let offset_y = if gravity.y.abs() >= 1.0 {
            round_to_even(gravity.y * offset_scale)
        } else {
            scale_to_even(height, gravity.y)
        };

        let left = match gravity.kind {
            GravityType::West | GravityType::NorthWest | GravityType::SouthWest => offset_x,
            GravityType::East | GravityType::NorthEast | GravityType::SouthEast => width - inner_width - offset_x,
            _ => half_to_even(width - inner_width + 1) + offset_x,
        };
        let top = match gravity.kind {
            GravityType::North | GravityType::NorthEast | GravityType::NorthWest => offset_y,
            GravityType::South | GravityType::SouthEast | GravityType::SouthWest => height - inner_height - offset_y,
            _ => half_to_even(height - inner_height + 1) + offset_y,
        };

        (left, top)
    };

    let (min_x, max_x, min_y, max_y) = if allow_overflow {
        (-inner_width + 1, width - 1, -inner_height + 1, height - 1)
    } else {
        (0, width - inner_width, 0, height - inner_height)
    };

    left = left.clamp(min_x.min(max_x), max_x.max(min_x));
    top = top.clamp(min_y.min(max_y), max_y.max(min_y));

    (left, top)
}

/// Crops to a window libvips chooses by looking at the image.
///
/// `smartcrop` scores the image for the region a viewer's eye would settle on,
/// which is the one thing a geometric gravity cannot do: a centre crop of a
/// portrait decapitates the subject, and no fixed anchor fixes that for every
/// image in a catalogue.
///
/// It has to see real pixels, so unlike every other window here it forces the
/// decode rather than composing into libvips' lazy pipeline.
pub fn smart_crop(img: &VipsImage, width: i32, height: i32) -> Result<VipsImage, TransformError> {
    let options = ops::SmartcropOptions {
        interesting: ops::Interesting::Attention,
        ..Default::default()
    };
    ops::smartcrop_with_opts(img, width, height, &options).map_err(vips("Error finding a smart crop"))
}

/// Crops an image to the region named by a [`Crop`].
///
/// A zero extent means "the whole axis", and an extent below 1 is a fraction of
/// the source, so the same URL crops the same proportion whatever size the
/// source turns out to be.
pub fn crop_image(
    img: VipsImage,
    crop: &Crop,
    gravity: &Gravity,
    aspect_ratio: Option<CropAspectRatio>,
) -> Result<VipsImage, TransformError> {
    let src_width = img.get_width().max(0) as u32;
    let src_height = img.get_height().max(0) as u32;
    let (requested_width, requested_height) = crop.resolve(src_width, src_height);

    // A zero extent means the whole axis, and an oversized one means the whole
    // axis too. Both are resolved against the source *before* the ratio is
    // corrected: correcting first and clamping afterwards throws the correction
    // away whenever the request was larger than the image, so `crop:1000:1000`
    // with `car:2` on a 100x100 source came back square instead of 100x50.
    let requested_width = if requested_width == 0 {
        src_width
    } else {
        requested_width
    };
    let requested_height = if requested_height == 0 {
        src_height
    } else {
        requested_height
    };
    let requested_width = requested_width.min(src_width).max(1);
    let requested_height = requested_height.min(src_height).max(1);

    let (width, height) = match aspect_ratio {
        Some(aspect_ratio) => aspect_ratio.correct(requested_width, requested_height),
        None => (requested_width, requested_height),
    };

    // An `enlarge` correction can still grow an axis past the source.
    let width = width.min(src_width).max(1);
    let height = height.min(src_height).max(1);

    // Nothing to cut: skipping keeps the image's own header rather than paying
    // for an extract that returns the same pixels.
    if width >= src_width && height >= src_height {
        return Ok(img);
    }

    if gravity.kind.is_content_aware() {
        return smart_crop(&img, width as i32, height as i32);
    }

    // The crop names source pixels, so DPR must not scale its offsets: it runs
    // before any DPR-aware scaling has happened.
    let (x, y) = calc_position(
        i64::from(src_width),
        i64::from(src_height),
        i64::from(width),
        i64::from(height),
        gravity,
        1.0,
        false,
    );

    ops::extract_area(&img, x as i32, y as i32, width as i32, height as i32).map_err(vips("Error cropping image"))
}

/// Extends an image onto a larger canvas filled with the background colour.
pub fn extend_image(
    img: VipsImage,
    width: u32,
    height: u32,
    gravity: &Gravity,
    background: &Option<[u8; 4]>,
    offset_scale: f64,
) -> Result<VipsImage, TransformError> {
    let src_w = img.get_width().max(0) as u32;
    let src_h = img.get_height().max(0) as u32;

    // A canvas no larger than the image extends nothing. imgproxy returns the
    // image untouched here rather than treating it as an error, and so does
    // every caller of this function.
    if width <= src_w && height <= src_h {
        return Ok(img);
    }

    let width = width.max(src_w);
    let height = height.max(src_h);

    // The URL can ask for any width and height it likes, and the canvas is what
    // libvips has to allocate. Checking in i64 keeps a value past i32 from
    // wrapping negative, which `embed` would either reject or, worse, accept as
    // a canvas smaller than the source.
    let (canvas_w, canvas_h) = (i64::from(width), i64::from(height));
    if canvas_w > VIPS_MAX_COORD || canvas_h > VIPS_MAX_COORD {
        return Err(TransformError::invalid(
            "extend",
            format!("extended canvas {canvas_w}x{canvas_h} exceeds the maximum of {VIPS_MAX_COORD} pixels per side"),
        ));
    }

    let (x, y) = calc_position(
        canvas_w,
        canvas_h,
        i64::from(src_w),
        i64::from(src_h),
        gravity,
        offset_scale,
        false,
    );

    let bg_color = background.unwrap_or([0, 0, 0, 0]);
    let options = ops::EmbedOptions {
        extend: ops::Extend::Background,
        background: bg_color_for_bands(bg_color, img.get_bands()),
    };
    ops::embed_with_opts(&img, x as i32, y as i32, canvas_w as i32, canvas_h as i32, &options)
        .map_err(vips("Error extending image"))
}

/// Extends an image out to the aspect ratio of `target_width:target_height`,
/// growing whichever axis is short and leaving the other alone.
///
/// This is `extend_aspect_ratio`: the result keeps every source pixel at its
/// resized size and gains background on one axis only, where plain `extend`
/// pads out to the requested pixel dimensions.
pub fn extend_to_aspect_ratio(
    img: VipsImage,
    target_width: u32,
    target_height: u32,
    gravity: &Gravity,
    background: &Option<[u8; 4]>,
    offset_scale: f64,
) -> Result<VipsImage, TransformError> {
    if target_width == 0 || target_height == 0 {
        return Ok(img);
    }

    let src_w = img.get_width().max(0) as u32;
    let src_h = img.get_height().max(0) as u32;
    if src_w == 0 || src_h == 0 {
        return Ok(img);
    }

    let target_ratio = f64::from(target_width) / f64::from(target_height);
    let source_ratio = f64::from(src_w) / f64::from(src_h);

    let (width, height) = if target_ratio > source_ratio {
        // The requested shape is wider than what we have: grow the width.
        (((f64::from(src_h) * target_ratio).round() as u32).max(src_w), src_h)
    } else if target_ratio < source_ratio {
        (src_w, ((f64::from(src_w) / target_ratio).round() as u32).max(src_h))
    } else {
        return Ok(img);
    };

    debug!(
        "Extending {}x{} to aspect ratio {}:{} -> {}x{}",
        src_w, src_h, target_width, target_height, width, height
    );

    extend_image(img, width, height, gravity, background, offset_scale)
}

/// Applies padding to an image.
pub fn apply_padding(
    img: VipsImage,
    top: u32,
    right: u32,
    bottom: u32,
    left: u32,
    background: &Option<[u8; 4]>,
) -> Result<VipsImage, TransformError> {
    // Padding arrives from the URL as an unbounded u32, so the canvas is summed
    // in i64. Doing it in i32 wrapped: a value above i32::MAX turned negative,
    // which either produced a canvas smaller than the source — silently
    // returning a cropped image with a 200 — or panicked in a debug build.
    let width = i64::from(img.get_width()) + i64::from(left) + i64::from(right);
    let height = i64::from(img.get_height()) + i64::from(top) + i64::from(bottom);

    if width > VIPS_MAX_COORD || height > VIPS_MAX_COORD {
        return Err(TransformError::invalid(
            "padding",
            format!("padded canvas {width}x{height} exceeds the maximum of {VIPS_MAX_COORD} pixels per side"),
        ));
    }

    // Both offsets are bounded by the canvas checked above, so these fit.
    let (x, y) = (left as i32, top as i32);
    let bg_color = background.unwrap_or([0, 0, 0, 0]);
    let options = ops::EmbedOptions {
        extend: ops::Extend::Background,
        background: bg_color_for_bands(bg_color, img.get_bands()),
    };

    ops::embed_with_opts(&img, x, y, width as i32, height as i32, &options).map_err(vips("Error applying padding"))
}
