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

/// Applies resize operation based on the resize type.
pub fn apply_resize(
    img: VipsImage,
    resize: &Resize,
    gravity: &Gravity,
    resizing_algorithm: Option<&str>,
    enlarge: bool,
    offset_scale: f64,
) -> Result<VipsImage, TransformError> {
    let src_width = img.get_width().max(0) as u32;
    let src_height = img.get_height().max(0) as u32;
    let (target_w, target_h) = resolve_resize_dimensions(resize, src_width, src_height)?;

    let resizing_type = match resize.resizing_type {
        ResizingType::Auto => {
            let src_is_portrait = is_portrait(src_width, src_height);
            let target_is_portrait = is_portrait(target_w, target_h);
            if src_is_portrait == target_is_portrait {
                debug!("Auto resize: orientations match, using fill");
                ResizingType::Fill
            } else {
                debug!("Auto resize: orientations differ, using fit");
                ResizingType::Fit
            }
        }
        other => other,
    };

    match resizing_type {
        ResizingType::Fill | ResizingType::FillDown => resize_to_fill(
            img,
            target_w,
            target_h,
            gravity,
            resizing_algorithm,
            enlarge,
            resizing_type == ResizingType::FillDown,
            offset_scale,
        ),
        ResizingType::Fit => resize_to_fit(img, target_w, target_h, resizing_algorithm, enlarge),
        ResizingType::Force => resize_to_force(img, target_w, target_h, resizing_algorithm, enlarge),
        // `auto` was rewritten above; matching it here would be unreachable.
        ResizingType::Auto => unreachable!("auto resizing is resolved before dispatch"),
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

/// Resizes an image to cover the target dimensions, cropping the overhang.
#[allow(clippy::too_many_arguments)]
fn resize_to_fill(
    img: VipsImage,
    width: u32,
    height: u32,
    gravity: &Gravity,
    resizing_algorithm: Option<&str>,
    enlarge: bool,
    fill_down: bool,
    offset_scale: f64,
) -> Result<VipsImage, TransformError> {
    let (img_w, img_h) = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);
    let aspect_ratio = f64::from(img_w) / f64::from(img_h);
    let target_aspect_ratio = f64::from(width) / f64::from(height);

    // Cover the box: scale by whichever axis needs the most.
    let cover = if aspect_ratio > target_aspect_ratio {
        f64::from(height) / f64::from(img_h)
    } else {
        f64::from(width) / f64::from(img_w)
    };
    let mut scales = [cover; 2];
    cap_enlargement(&mut scales, enlarge);

    let resized_img = if (scales[0] - 1.0).abs() < SCALE_EPSILON {
        img
    } else {
        // Bump the scale slightly so kernels that round down still cover the target.
        resize_with_algorithm(
            &img,
            scales[0] * (1.0 + SCALE_EPSILON),
            None,
            resizing_algorithm,
            "Error resizing for fill",
        )?
    };

    let resized_w = resized_img.get_width().max(0) as u32;
    let resized_h = resized_img.get_height().max(0) as u32;

    let (crop_w, crop_h) = fill_window(resized_w, resized_h, width, height, fill_down, enlarge);

    if crop_w >= resized_w && crop_h >= resized_h {
        return Ok(resized_img);
    }

    if gravity.kind.is_content_aware() {
        return smart_crop(&resized_img, crop_w as i32, crop_h as i32);
    }

    // Cropping happens after the scale, so an absolute gravity offset is
    // measured against the scaled image — and DPR is what scaled it. Folding
    // DPR into the resize target is precisely why the offset has to grow with
    // it: at `dpr:2` the result is twice the size, so a 10px nudge that stayed
    // 10px would move the window half as far as it did at 1x. imgproxy passes
    // its DPR scale here for the same reason.
    let (crop_x, crop_y) = calc_position(
        i64::from(resized_w),
        i64::from(resized_h),
        i64::from(crop_w),
        i64::from(crop_h),
        gravity,
        offset_scale,
        false,
    );

    ops::extract_area(&resized_img, crop_x as i32, crop_y as i32, crop_w as i32, crop_h as i32)
        .map_err(vips("Error cropping after fill resize"))
}

/// Resizes an image to the exact target dimensions, allowing aspect ratio changes.
fn resize_to_force(
    img: VipsImage,
    width: u32,
    height: u32,
    resizing_algorithm: Option<&str>,
    enlarge: bool,
) -> Result<VipsImage, TransformError> {
    let (src_w, src_h) = (f64::from(img.get_width()), f64::from(img.get_height()));
    let mut scales = [f64::from(width) / src_w, f64::from(height) / src_h];
    cap_enlargement(&mut scales, enlarge);

    if (scales[0] - 1.0).abs() < SCALE_EPSILON && (scales[1] - 1.0).abs() < SCALE_EPSILON {
        return Ok(img);
    }
    resize_with_algorithm(
        &img,
        scales[0],
        Some(scales[1]),
        resizing_algorithm,
        "Error force resizing",
    )
}

/// Resizes an image to fit within the target dimensions while maintaining aspect ratio.
fn resize_to_fit(
    img: VipsImage,
    width: u32,
    height: u32,
    resizing_algorithm: Option<&str>,
    enlarge: bool,
) -> Result<VipsImage, TransformError> {
    let (img_w, img_h) = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);

    debug!("Resizing to fit from {}x{} to {}x{}", img_w, img_h, width, height);
    let scale_w = f64::from(width) / f64::from(img_w);
    let scale_h = f64::from(height) / f64::from(img_h);
    let mut scales = [scale_w.min(scale_h); 2];
    cap_enlargement(&mut scales, enlarge);

    if (scales[0] - 1.0).abs() < SCALE_EPSILON {
        return Ok(img);
    }

    resize_with_algorithm(&img, scales[0], None, resizing_algorithm, "Error fitting resize")
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
