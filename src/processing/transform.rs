use crate::processing::options::{Adjust, Crop, Flip, Gravity, Resize};
use crate::utils::read_exif_orientation;
use libvips::{ops, VipsImage};
use thiserror::Error;
use tracing::debug;

const SCALE_EPSILON: f64 = 1e-6;

/// Largest coordinate libvips accepts for `embed`; anything beyond it is
/// rejected by the operation itself.
const VIPS_MAX_COORD: i64 = 1_000_000_000;

/// Errors produced while transforming an image.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransformError {
    #[error("{operation}: {source}")]
    Vips {
        operation: &'static str,
        #[source]
        source: libvips::error::Error,
    },
    #[error("{message}")]
    InvalidArgument { operation: &'static str, message: String },
}

impl TransformError {
    fn invalid(operation: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidArgument {
            operation,
            message: message.into(),
        }
    }
}

fn vips(operation: &'static str) -> impl FnOnce(libvips::error::Error) -> TransformError {
    move |source| TransformError::Vips { operation, source }
}

/// Converts a resizing algorithm string to a libvips Kernel enum.
fn get_resize_kernel(algorithm: Option<&str>) -> ops::Kernel {
    match algorithm.unwrap_or("lanczos3") {
        "nearest" => ops::Kernel::Nearest,
        "linear" => ops::Kernel::Linear,
        "cubic" => ops::Kernel::Cubic,
        "lanczos2" => ops::Kernel::Lanczos2,
        "lanczos3" => ops::Kernel::Lanczos3,
        _ => ops::Kernel::Lanczos3, // Default to lanczos3
    }
}

fn bg_color_for_bands(bg_color: [u8; 4], bands: i32) -> Vec<f64> {
    let luma = (0.299 * bg_color[0] as f64 + 0.587 * bg_color[1] as f64 + 0.114 * bg_color[2] as f64).round();
    match bands {
        4 => vec![
            bg_color[0] as f64,
            bg_color[1] as f64,
            bg_color[2] as f64,
            bg_color[3] as f64,
        ],
        3 => vec![bg_color[0] as f64, bg_color[1] as f64, bg_color[2] as f64],
        2 => vec![luma, bg_color[3] as f64],
        1 => vec![luma],
        _ => vec![bg_color[0] as f64, bg_color[1] as f64, bg_color[2] as f64],
    }
}

/// Helper to resize using the requested algorithm, defaulting to lanczos3.
pub fn resize_with_algorithm(
    img: &VipsImage,
    hscale: f64,
    vscale: Option<f64>,
    resizing_algorithm: Option<&str>,
    error_context: &'static str,
) -> Result<VipsImage, TransformError> {
    let options = ops::ResizeOptions {
        kernel: get_resize_kernel(resizing_algorithm),
        vscale: vscale.unwrap_or(hscale),
        ..Default::default()
    };

    ops::resize_with_opts(img, hscale, &options).map_err(vips(error_context))
}

/// Applies EXIF rotation to an image based on orientation data.
pub fn apply_exif_rotation(image_bytes: &[u8], mut img: VipsImage) -> Result<VipsImage, TransformError> {
    if let Some(orientation) = read_exif_orientation(image_bytes) {
        debug!("Found EXIF orientation: {:?}", orientation);
        img = apply_exif_orientation(img, orientation)?;
    }
    Ok(img)
}

pub(crate) fn apply_exif_orientation(mut img: VipsImage, orientation: u32) -> Result<VipsImage, TransformError> {
    match orientation {
        2 => img = ops::flip(&img, ops::Direction::Horizontal).map_err(vips("Error flipping horizontally"))?,
        3 => img = ops::rot(&img, ops::Angle::D180).map_err(vips("Error rotating 180"))?,
        4 => img = ops::flip(&img, ops::Direction::Vertical).map_err(vips("Error flipping vertically"))?,
        5 => {
            img = ops::flip(
                &ops::rot(&img, ops::Angle::D90).map_err(vips("Error rotating 90"))?,
                ops::Direction::Horizontal,
            )
            .map_err(vips("Error flipping after rotate"))?
        }
        6 => img = ops::rot(&img, ops::Angle::D90).map_err(vips("Error rotating 90"))?,
        7 => {
            img = ops::flip(
                &ops::rot(&img, ops::Angle::D270).map_err(vips("Error rotating 270"))?,
                ops::Direction::Horizontal,
            )
            .map_err(vips("Error flipping after rotate"))?
        }
        8 => img = ops::rot(&img, ops::Angle::D270).map_err(vips("Error rotating 270"))?,
        _ => {}
    }
    Ok(img)
}

/// Crops an image to the specified dimensions.
pub fn crop_image(img: VipsImage, crop: Crop) -> Result<VipsImage, TransformError> {
    let src_width = img.get_width() as u32;
    let src_height = img.get_height() as u32;
    let width = if crop.width == 0 {
        src_width
    } else {
        crop.width.min(src_width)
    };
    let height = if crop.height == 0 {
        src_height
    } else {
        crop.height.min(src_height)
    };
    let (x, y) = if let Some(gravity) = crop.gravity {
        crop_origin_for_gravity(src_width, src_height, width, height, gravity)
    } else {
        (crop.x, crop.y)
    };

    ops::extract_area(&img, x as i32, y as i32, width as i32, height as i32).map_err(vips("Error cropping image"))
}

fn crop_origin_for_gravity(src_width: u32, src_height: u32, width: u32, height: u32, gravity: Gravity) -> (u32, u32) {
    let extra_w = src_width.saturating_sub(width);
    let extra_h = src_height.saturating_sub(height);

    let x = match gravity {
        Gravity::West | Gravity::NorthWest | Gravity::SouthWest => 0,
        Gravity::East | Gravity::NorthEast | Gravity::SouthEast => extra_w,
        _ => extra_w / 2,
    };

    let y = match gravity {
        Gravity::North | Gravity::NorthEast | Gravity::NorthWest => 0,
        Gravity::South | Gravity::SouthEast | Gravity::SouthWest => extra_h,
        _ => extra_h / 2,
    };

    (x, y)
}

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

    let aspect = src_width as f64 / src_height as f64;

    if resize.resizing_type == "force" {
        if width == 0 {
            width = src_width;
        }
        if height == 0 {
            height = src_height;
        }
    } else {
        if width == 0 {
            width = ((height as f64) * aspect).round() as u32;
        }
        if height == 0 {
            height = ((width as f64) / aspect).round() as u32;
        }
    }

    if width == 0 || height == 0 {
        return Err(TransformError::invalid("resize", "resize resolved to zero dimension"));
    }

    Ok((width, height))
}

/// Applies resize operation based on the resize type.
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

pub fn apply_resize(
    img: VipsImage,
    resize: &Resize,
    gravity: &Option<Gravity>,
    resizing_algorithm: Option<&str>,
    enlarge: bool,
) -> Result<VipsImage, TransformError> {
    let src_width = img.get_width() as u32;
    let src_height = img.get_height() as u32;
    let (target_w, target_h) = resolve_resize_dimensions(resize, src_width, src_height)?;

    match resize.resizing_type.as_str() {
        "fill" => resize_to_fill(
            img,
            target_w,
            target_h,
            gravity.unwrap_or(Gravity::Center),
            resizing_algorithm,
            enlarge,
        ),
        "fit" => resize_to_fit(img, target_w, target_h, resizing_algorithm, enlarge),
        "force" => resize_to_force(img, target_w, target_h, resizing_algorithm, enlarge),
        "auto" => {
            let src_is_portrait = super::utils::is_portrait(src_width, src_height);
            let target_is_portrait = super::utils::is_portrait(target_w, target_h);

            if src_is_portrait == target_is_portrait {
                debug!("Auto resize: orientations match, using fill");
                resize_to_fill(
                    img,
                    target_w,
                    target_h,
                    gravity.unwrap_or(Gravity::Center),
                    resizing_algorithm,
                    enlarge,
                )
            } else {
                debug!("Auto resize: orientations differ, using fit");
                resize_to_fit(img, target_w, target_h, resizing_algorithm, enlarge)
            }
        }
        _ => Err(TransformError::invalid(
            "resize",
            format!("Unknown resize type: {}", resize.resizing_type),
        )),
    }
}

/// Resizes an image to fill the target dimensions, cropping if necessary.
fn resize_to_fill(
    img: VipsImage,
    width: u32,
    height: u32,
    gravity: Gravity,
    resizing_algorithm: Option<&str>,
    enlarge: bool,
) -> Result<VipsImage, TransformError> {
    let (img_w, img_h) = (img.get_width() as u32, img.get_height() as u32);
    let aspect_ratio = img_w as f32 / img_h as f32;
    let target_aspect_ratio = width as f32 / height as f32;

    // Cover the box: scale by whichever axis needs the most.
    let cover = if aspect_ratio > target_aspect_ratio {
        height as f64 / img_h as f64
    } else {
        width as f64 / img_w as f64
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

    let resized_w = resized_img.get_width() as u32;
    let resized_h = resized_img.get_height() as u32;

    // With enlargement capped the image can be smaller than the requested box,
    // so the window is what is actually available. Cropping to the full box
    // would ask libvips for pixels that do not exist.
    let crop_w = width.min(resized_w);
    let crop_h = height.min(resized_h);
    let extra_w = resized_w - crop_w;
    let extra_h = resized_h - crop_h;

    let crop_x = match gravity {
        Gravity::West | Gravity::NorthWest | Gravity::SouthWest => 0,
        Gravity::East | Gravity::NorthEast | Gravity::SouthEast => extra_w,
        _ => extra_w / 2,
    };

    let crop_y = match gravity {
        Gravity::North | Gravity::NorthEast | Gravity::NorthWest => 0,
        Gravity::South | Gravity::SouthEast | Gravity::SouthWest => extra_h,
        _ => extra_h / 2,
    };

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
    let (src_w, src_h) = (img.get_width() as f64, img.get_height() as f64);
    let mut scales = [width as f64 / src_w, height as f64 / src_h];
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
    let (img_w, img_h) = (img.get_width() as u32, img.get_height() as u32);
    let aspect_ratio = img_w as f32 / img_h as f32;

    let (target_w, target_h) = if height == 0 {
        (width, (width as f32 / aspect_ratio).round() as u32)
    } else if width == 0 {
        ((height as f32 * aspect_ratio).round() as u32, height)
    } else {
        (width, height)
    };

    debug!("Resizing to fit from {}x{} to {}x{}", img_w, img_h, target_w, target_h);
    let scale_w = target_w as f64 / img_w as f64;
    let scale_h = target_h as f64 / img_h as f64;
    let mut scales = [scale_w.min(scale_h); 2];
    cap_enlargement(&mut scales, enlarge);

    if (scales[0] - 1.0).abs() < SCALE_EPSILON {
        return Ok(img);
    }

    resize_with_algorithm(&img, scales[0], None, resizing_algorithm, "Error fitting resize")
}

/// Extends an image to the target dimensions with background color.
pub fn extend_image(
    img: VipsImage,
    width: u32,
    height: u32,
    gravity: &Option<Gravity>,
    background: &Option<[u8; 4]>,
) -> Result<VipsImage, TransformError> {
    let bg_color = background.unwrap_or([0, 0, 0, 0]);
    let src_w = img.get_width() as u32;
    let src_h = img.get_height() as u32;
    if width < src_w || height < src_h {
        return Err(TransformError::invalid(
            "extend",
            format!(
                "extend target {}x{} must be at least source {}x{}",
                width, height, src_w, src_h
            ),
        ));
    }

    let gravity = gravity.unwrap_or(Gravity::Center);

    let (x, y) = match gravity {
        Gravity::Center => ((width - src_w) / 2, (height - src_h) / 2),
        Gravity::North => ((width - src_w) / 2, 0),
        Gravity::South => ((width - src_w) / 2, height - src_h),
        Gravity::West => (0, (height - src_h) / 2),
        Gravity::East => (width - src_w, (height - src_h) / 2),
        Gravity::NorthEast => (width - src_w, 0),
        Gravity::NorthWest => (0, 0),
        Gravity::SouthEast => (width - src_w, height - src_h),
        Gravity::SouthWest => (0, height - src_h),
    };

    let options = ops::EmbedOptions {
        extend: ops::Extend::Background,
        background: bg_color_for_bands(bg_color, img.get_bands()),
    };
    ops::embed_with_opts(&img, x as i32, y as i32, width as i32, height as i32, &options)
        .map_err(vips("Error extending image"))
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

/// Applies rotation to an image.
pub fn apply_rotation(img: VipsImage, rotation: u16) -> Result<VipsImage, TransformError> {
    match rotation {
        0 => Ok(img),
        90 => ops::rot(&img, ops::Angle::D90).map_err(vips("Error rotating 90")),
        180 => ops::rot(&img, ops::Angle::D180).map_err(vips("Error rotating 180")),
        270 => ops::rot(&img, ops::Angle::D270).map_err(vips("Error rotating 270")),
        _ => Err(TransformError::invalid(
            "rotation",
            format!("Unsupported rotation angle: {rotation}"),
        )),
    }
}

/// Applies horizontal and/or vertical flips to an image.
pub fn apply_flip(mut img: VipsImage, flip: Flip) -> Result<VipsImage, TransformError> {
    if flip.horizontal {
        img = ops::flip(&img, ops::Direction::Horizontal).map_err(vips("Error flipping horizontally"))?;
    }
    if flip.vertical {
        img = ops::flip(&img, ops::Direction::Vertical).map_err(vips("Error flipping vertically"))?;
    }
    Ok(img)
}

/// Applies blur to an image.
pub fn apply_blur(img: VipsImage, sigma: f32) -> Result<VipsImage, TransformError> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(TransformError::invalid(
            "blur",
            "blur sigma must be a finite positive number",
        ));
    }
    ops::gaussblur(&img, sigma as f64).map_err(vips("Error applying blur"))
}

/// Applies brightness, contrast, and saturation adjustments.
pub fn apply_adjust(img: VipsImage, adjust: Adjust) -> Result<VipsImage, TransformError> {
    let mut current = img;

    // The generated libvips bindings in this crate do not expose `linear`, so
    // brightness/contrast are parsed for compatibility and saturation is applied
    // where libvips exposes a stable operation.
    let _ = (adjust.brightness, adjust.contrast);

    if (adjust.saturation - 1.0).abs() > f32::EPSILON {
        current = apply_saturation(current, adjust.saturation)?;
    }

    Ok(current)
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

    let s = saturation as f64;
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

/// Applies background color to an image (useful for JPEG output).
pub fn apply_background_color(img: VipsImage, _bg_color: [u8; 4]) -> Result<VipsImage, TransformError> {
    // Only flatten if the image has an alpha channel (bands == 4 for RGBA or bands == 2 for grayscale+alpha)
    let bands = img.get_bands();
    if bands != 4 && bands != 2 {
        // No alpha channel, nothing to flatten - return as-is
        return Ok(img);
    }

    // Use libvips flatten to composite over a solid background, dropping alpha.
    // Only RGB is used; input alpha is ignored for the background color itself.
    let bg = vec![_bg_color[0] as f64, _bg_color[1] as f64, _bg_color[2] as f64];
    let opts = ops::FlattenOptions {
        background: bg,
        ..Default::default()
    };
    ops::flatten_with_opts(&img, &opts).map_err(vips("Error applying background color"))
}

/// Applies min-width and min-height constraints to an image.
pub fn apply_min_dimensions(
    img: VipsImage,
    min_width: Option<u32>,
    min_height: Option<u32>,
    resizing_algorithm: Option<&str>,
) -> Result<VipsImage, TransformError> {
    let mut current_img = img;
    let (img_w, img_h) = (current_img.get_width() as u32, current_img.get_height() as u32);

    let mut scale_w = 1.0;
    if let Some(mw) = min_width {
        if img_w < mw {
            scale_w = mw as f64 / img_w as f64;
        }
    }

    let mut scale_h = 1.0;
    if let Some(mh) = min_height {
        if img_h < mh {
            scale_h = mh as f64 / img_h as f64;
        }
    }

    let scale = scale_w.max(scale_h);
    if scale > 1.0 {
        current_img = resize_with_algorithm(
            &current_img,
            scale,
            None,
            resizing_algorithm,
            "Error applying min dimensions",
        )?;
    }

    Ok(current_img)
}

/// Applies zoom to an image.
pub fn apply_zoom(img: VipsImage, zoom: f32, resizing_algorithm: Option<&str>) -> Result<VipsImage, TransformError> {
    if !zoom.is_finite() || zoom <= 0.0 {
        return Err(TransformError::invalid("zoom", "zoom must be a finite positive number"));
    }
    resize_with_algorithm(&img, zoom as f64, None, resizing_algorithm, "Error applying zoom")
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
        sigma: clamped_sigma as f64,
        ..Default::default()
    };
    ops::sharpen_with_opts(&img, &opts).map_err(vips("Error applying sharpen"))
}

/// Pixelates an image.
pub fn apply_pixelate(
    img: VipsImage,
    amount: u32,
    _resizing_algorithm: Option<&str>,
) -> Result<VipsImage, TransformError> {
    if amount == 0 {
        return Ok(img);
    }
    let (w, h) = (img.get_width() as u32, img.get_height() as u32);
    let target_w = (w / amount).max(1);
    let target_h = (h / amount).max(1);
    let pixelated = resize_with_algorithm(
        &img,
        target_w as f64 / w as f64,
        Some(target_h as f64 / h as f64),
        Some("nearest"),
        "Error pixelating (down)",
    )?;
    resize_with_algorithm(
        &pixelated,
        w as f64 / pixelated.get_width() as f64,
        Some(h as f64 / pixelated.get_height() as f64),
        Some("nearest"),
        "Error pixelating (up)",
    )
}
