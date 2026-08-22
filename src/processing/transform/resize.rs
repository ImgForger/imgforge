//! Scaling: the resizing types, the enlargement cap, and the two scale-only
//! options that run after them.

use super::geometry::{calc_position, smart_crop};
use super::{resize_with_algorithm, vips, TransformError, SCALE_EPSILON};
use crate::processing::options::{Gravity, Resize, ResizingType, Zoom};
use crate::processing::utils::is_portrait;
use libvips::{ops, VipsImage};
use tracing::debug;

/// Resolves target resize dimensions, filling in zero values according to imgproxy rules.
pub fn resolve_resize_dimensions(
    resize: &Resize,
    src_width: u32,
    src_height: u32,
) -> Result<(u32, u32), TransformError> {
    let mut width = resize.width;
    let mut height = resize.height;

    if width == 0 && height == 0 {
        return Err(TransformError::invalid(
            "resize",
            "resize requires at least one non-zero dimension",
        ));
    }

    if src_width == 0 || src_height == 0 {
        return Err(TransformError::invalid("resize", "source image has a zero dimension"));
    }

    let aspect = f64::from(src_width) / f64::from(src_height);

    if resize.resizing_type.fills_zero_axis_from_source() {
        if width == 0 {
            width = src_width;
        }
        if height == 0 {
            height = src_height;
        }
    } else {
        if width == 0 {
            width = (f64::from(height) * aspect).round() as u32;
        }
        if height == 0 {
            height = (f64::from(width) / aspect).round() as u32;
        }
    }

    if width == 0 || height == 0 {
        return Err(TransformError::invalid("resize", "resize resolved to zero dimension"));
    }

    Ok((width, height))
}

/// Caps scaling so nothing is enlarged, following imgproxy: the resizing type
/// settles the scale first, then the cap divides every axis by the largest
/// scale when that exceeds 1. The axis that would have been enlarged lands
/// exactly at 1 and the others keep their relative proportion — which is not
/// the same as refusing the whole operation, because a fit whose box is taller
/// than the source still has to shrink the width.
fn cap_enlargement(scales: &mut [f64; 2], enlarge: bool) {
    if enlarge {
        return;
    }
    let largest = scales[0].max(scales[1]);
    if largest > 1.0 {
        scales[0] /= largest;
        scales[1] /= largest;
    }
}

/// What the scale stage should do, and what the crop after it should keep.
///
/// imgproxy computes this before touching pixels, and it has to: the scale is
/// applied to the image as stored, while the target size and the crop window
/// describe the image the caller will get back — and a `rotate:90` between them
/// swaps the two. Deriving both from one plan is what keeps
/// `resize:fill:800:600/rotate:90` returning 800x600 rather than 600x800.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResizePlan {
    /// Horizontal scale, already swapped for the stored orientation.
    pub hscale: f64,
    /// Vertical scale, already swapped for the stored orientation.
    pub vscale: f64,
    /// The box the request asked for, in the final orientation.
    pub target: (u32, u32),
    /// Window to keep after rotation, in the final orientation. `None` for the
    /// resizing types that do not crop.
    pub result_crop: Option<(u32, u32)>,
}

impl ResizePlan {
    /// Whether the scale would change anything.
    fn is_identity(&self) -> bool {
        (self.hscale - 1.0).abs() < SCALE_EPSILON && (self.vscale - 1.0).abs() < SCALE_EPSILON
    }
}

/// The window a fill crops to once the image has been scaled.
///
/// Plain `fill` always crops to the requested box, clamped to what the scaled
/// image actually offers. `fill-down` instead keeps the requested *aspect
/// ratio* when the scaled image came out smaller than the box, so the result is
/// the largest crop of that shape the image can supply rather than a smaller
/// box padded out — which is the whole difference between the two types.
fn fill_window(
    scaled_w: u32,
    scaled_h: u32,
    target_w: u32,
    target_h: u32,
    fill_down: bool,
    enlarge: bool,
) -> (u32, u32) {
    if !fill_down || enlarge || scaled_w == 0 || scaled_h == 0 || target_w == 0 || target_h == 0 {
        return (target_w.min(scaled_w), target_h.min(scaled_h));
    }

    let diff_w = f64::from(target_w) / f64::from(scaled_w);
    let diff_h = f64::from(target_h) / f64::from(scaled_h);
    let aspect = f64::from(target_w) / f64::from(target_h);

    let (window_w, window_h) = if diff_w > diff_h && diff_w > 1.0 {
        (scaled_w, ((f64::from(scaled_w) / aspect).round() as u32).max(1))
    } else if diff_h > diff_w && diff_h > 1.0 {
        (((f64::from(scaled_h) * aspect).round() as u32).max(1), scaled_h)
    } else {
        (target_w, target_h)
    };

    (window_w.min(scaled_w), window_h.min(scaled_h))
}

/// Resolves `auto` against the source and target shapes.
fn effective_resizing_type(resize: &Resize, src: (u32, u32), target: (u32, u32)) -> ResizingType {
    match resize.resizing_type {
        ResizingType::Auto => {
            if is_portrait(src.0, src.1) == is_portrait(target.0, target.1) {
                debug!("Auto resize: orientations match, using fill");
                ResizingType::Fill
            } else {
                debug!("Auto resize: orientations differ, using fit");
                ResizingType::Fit
            }
        }
        other => other,
    }
}

/// Plans a resize against the dimensions the caller will see.
///
/// `src` is the source measured in the *final* orientation, so a request that
/// also rotates by a right angle passes the transposed size and gets scales
/// that have been swapped back for the stored image.
pub fn plan_resize(
    src: (u32, u32),
    resize: &Resize,
    enlarge: bool,
    transposes: bool,
) -> Result<ResizePlan, TransformError> {
    let (src_w, src_h) = src;
    let (target_w, target_h) = resolve_resize_dimensions(resize, src_w, src_h)?;
    let resizing_type = effective_resizing_type(resize, src, (target_w, target_h));

    let (width, height) = (f64::from(src_w), f64::from(src_h));
    let (fit_w, fit_h) = (f64::from(target_w) / width, f64::from(target_h) / height);

    let mut scales = match resizing_type {
        // Cover the box: scale by whichever axis needs the most.
        ResizingType::Fill | ResizingType::FillDown => [fit_w.max(fit_h); 2],
        ResizingType::Fit => [fit_w.min(fit_h); 2],
        ResizingType::Force => [fit_w, fit_h],
        ResizingType::Auto => unreachable!("auto is resolved above"),
    };
    cap_enlargement(&mut scales, enlarge);

    let scaled = (
        ((width * scales[0]).round() as u32).max(1),
        ((height * scales[1]).round() as u32).max(1),
    );

    let result_crop = match resizing_type {
        ResizingType::Fill | ResizingType::FillDown => Some(fill_window(
            scaled.0,
            scaled.1,
            target_w,
            target_h,
            resizing_type == ResizingType::FillDown,
            enlarge,
        )),
        _ => None,
    };

    // The scales were computed against the final orientation; the image they
    // will be applied to is still the stored one.
    let (hscale, vscale) = if transposes {
        (scales[1], scales[0])
    } else {
        (scales[0], scales[1])
    };

    Ok(ResizePlan {
        hscale,
        vscale,
        target: (target_w, target_h),
        result_crop,
    })
}

/// Applies the scale half of a plan.
pub fn apply_scale(
    img: VipsImage,
    plan: &ResizePlan,
    resizing_algorithm: Option<&str>,
) -> Result<VipsImage, TransformError> {
    if plan.is_identity() {
        return Ok(img);
    }

    // Bump the scale slightly so kernels that round down still cover the
    // target; a fill that lands a pixel short cannot be cropped to its box.
    let bump = if plan.result_crop.is_some() {
        1.0 + SCALE_EPSILON
    } else {
        1.0
    };

    resize_with_algorithm(
        &img,
        plan.hscale * bump,
        Some(plan.vscale * bump),
        resizing_algorithm,
        "Error resizing",
    )
}

/// Crops the scaled image to the window the resizing type asked for.
///
/// Runs after rotation, so the window and the gravity both describe the image
/// the caller receives.
pub fn crop_to_result(
    img: VipsImage,
    window: (u32, u32),
    gravity: &Gravity,
    offset_scale: f64,
) -> Result<VipsImage, TransformError> {
    let (img_w, img_h) = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);
    let (crop_w, crop_h) = (window.0.min(img_w), window.1.min(img_h));

    if crop_w == 0 || crop_h == 0 || (crop_w >= img_w && crop_h >= img_h) {
        return Ok(img);
    }

    if gravity.kind.is_content_aware() {
        return smart_crop(&img, crop_w as i32, crop_h as i32);
    }

    // An absolute gravity offset is measured against the scaled image — and DPR
    // is what scaled it. Folding DPR into the resize target is precisely why the
    // offset has to grow with it: at `dpr:2` the result is twice the size, so a
    // 10px nudge that stayed 10px would move the window half as far as it did at
    // 1x. imgproxy passes its DPR scale here for the same reason.
    let (crop_x, crop_y) = calc_position(
        i64::from(img_w),
        i64::from(img_h),
        i64::from(crop_w),
        i64::from(crop_h),
        gravity,
        offset_scale,
        false,
    );

    ops::extract_area(&img, crop_x as i32, crop_y as i32, crop_w as i32, crop_h as i32)
        .map_err(vips("Error cropping after resize"))
}

/// Scales and crops in one step, for callers with no rotation to interleave.
pub fn apply_resize(
    img: VipsImage,
    resize: &Resize,
    gravity: &Gravity,
    resizing_algorithm: Option<&str>,
    enlarge: bool,
    offset_scale: f64,
) -> Result<VipsImage, TransformError> {
    let src = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);
    let plan = plan_resize(src, resize, enlarge, false)?;
    let scaled = apply_scale(img, &plan, resizing_algorithm)?;

    match plan.result_crop {
        Some(window) => crop_to_result(scaled, window, gravity, offset_scale),
        None => Ok(scaled),
    }
}

/// Applies min-width and min-height constraints to an image.
pub fn apply_min_dimensions(
    img: VipsImage,
    min_width: Option<u32>,
    min_height: Option<u32>,
    resizing_algorithm: Option<&str>,
) -> Result<VipsImage, TransformError> {
    let (img_w, img_h) = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);
    if img_w == 0 || img_h == 0 {
        return Ok(img);
    }

    let scale_w = min_width
        .filter(|min| img_w < *min)
        .map(|min| f64::from(min) / f64::from(img_w))
        .unwrap_or(1.0);
    let scale_h = min_height
        .filter(|min| img_h < *min)
        .map(|min| f64::from(min) / f64::from(img_h))
        .unwrap_or(1.0);

    let scale = scale_w.max(scale_h);
    if scale <= 1.0 {
        return Ok(img);
    }

    resize_with_algorithm(&img, scale, None, resizing_algorithm, "Error applying min dimensions")
}

/// Applies zoom to an image.
pub fn apply_zoom(img: VipsImage, zoom: Zoom, resizing_algorithm: Option<&str>) -> Result<VipsImage, TransformError> {
    if !zoom.x.is_finite() || !zoom.y.is_finite() || zoom.x <= 0.0 || zoom.y <= 0.0 {
        return Err(TransformError::invalid(
            "zoom",
            "zoom factors must be finite positive numbers",
        ));
    }
    if zoom.is_identity() {
        return Ok(img);
    }
    resize_with_algorithm(
        &img,
        f64::from(zoom.x),
        Some(f64::from(zoom.y)),
        resizing_algorithm,
        "Error applying zoom",
    )
}
