use crate::processing::options::{Crop, Resize};
use exif::{In, Tag};
use libvips::{ops, VipsImage};
use std::io::Cursor;
use tracing::debug;

const SCALE_EPSILON: f64 = 1e-6;

/// Converts a resizing algorithm string to a libvips Kernel enum.
fn get_resize_kernel(algorithm: &Option<String>) -> ops::Kernel {
    match algorithm.as_deref().unwrap_or("lanczos3") {
        "nearest" => ops::Kernel::Nearest,
        "linear" => ops::Kernel::Linear,
        "cubic" => ops::Kernel::Cubic,
        "lanczos2" => ops::Kernel::Lanczos2,
        "lanczos3" => ops::Kernel::Lanczos3,
        _ => ops::Kernel::Lanczos3, // Default to lanczos3
    }
}

/// Helper to resize using the requested algorithm, defaulting to lanczos3.
pub fn resize_with_algorithm(
    img: &VipsImage,
    hscale: f64,
    vscale: Option<f64>,
    resizing_algorithm: &Option<String>,
    error_context: &str,
) -> Result<VipsImage, String> {
    let options = ops::ResizeOptions {
        kernel: get_resize_kernel(resizing_algorithm),
        vscale: vscale.unwrap_or(hscale),
        ..Default::default()
    };

    ops::resize_with_opts(img, hscale, &options).map_err(|e| format!("{error_context}: {}", e))
}

/// Applies EXIF rotation to an image based on orientation data.
pub fn apply_exif_rotation(image_bytes: &[u8], mut img: VipsImage) -> Result<VipsImage, String> {
    let exif_reader = exif::Reader::new();
    if let Ok(exif) = exif_reader.read_from_container(&mut Cursor::new(image_bytes)) {
        if let Some(orientation) = exif.get_field(Tag::Orientation, In::PRIMARY) {
            debug!("Found EXIF orientation: {:?}", orientation.value.get_uint(0));
            match orientation.value.get_uint(0) {
                Some(2) => {
                    img = ops::flip(&img, ops::Direction::Horizontal)
                        .map_err(|e| format!("Error flipping horizontally: {}", e))?
                }
                Some(3) => img = ops::rot(&img, ops::Angle::D180).map_err(|e| format!("Error rotating 180: {}", e))?,
                Some(4) => {
                    img = ops::flip(&img, ops::Direction::Vertical)
                        .map_err(|e| format!("Error flipping vertically: {}", e))?
                }
                Some(5) => {
                    img = ops::flip(
                        &ops::rot(&img, ops::Angle::D90).map_err(|e| format!("Error rotating 90: {}", e))?,
                        ops::Direction::Horizontal,
                    )
                    .map_err(|e| format!("Error flipping after rotate: {}", e))?
                }
                Some(6) => img = ops::rot(&img, ops::Angle::D90).map_err(|e| format!("Error rotating 90: {}", e))?,
                Some(7) => {
                    img = ops::flip(
                        &ops::rot(&img, ops::Angle::D270).map_err(|e| format!("Error rotating 270: {}", e))?,
                        ops::Direction::Horizontal,
                    )
                    .map_err(|e| format!("Error flipping after rotate: {}", e))?
                }
                Some(8) => img = ops::rot(&img, ops::Angle::D270).map_err(|e| format!("Error rotating 270: {}", e))?,
                _ => {}
            }
        }
    }
    Ok(img)
}

/// Crops an image to the specified dimensions.
pub fn crop_image(img: VipsImage, crop: Crop) -> Result<VipsImage, String> {
    ops::extract_area(
        &img,
        crop.x as i32,
        crop.y as i32,
        crop.width as i32,
        crop.height as i32,
    )
    .map_err(|e| format!("Error cropping image: {}", e))
}

/// Resolves target resize dimensions, filling in zero values according to imgproxy rules.
pub fn resolve_resize_dimensions(resize: &Resize, src_width: u32, src_height: u32) -> Result<(u32, u32), String> {
    let mut width = resize.width;
    let mut height = resize.height;

    if width == 0 && height == 0 {
        return Err("resize requires at least one non-zero dimension".to_string());
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
        return Err("resize resolved to zero dimension".to_string());
    }

    Ok((width, height))
}

/// Applies resize operation based on the resize type.
pub fn apply_resize(
    img: VipsImage,
    resize: &Resize,
    gravity: &Option<String>,
    resizing_algorithm: &Option<String>,
) -> Result<VipsImage, String> {
    let src_width = img.get_width() as u32;
    let src_height = img.get_height() as u32;
    let (target_w, target_h) = resolve_resize_dimensions(resize, src_width, src_height)?;

    match resize.resizing_type.as_str() {
        "fill" => resize_to_fill(
            img,
            target_w,
            target_h,
            gravity.as_deref().unwrap_or("center"),
            resizing_algorithm,
        ),
        "fit" => resize_to_fit(img, target_w, target_h, resizing_algorithm),
        "force" => resize_to_force(img, target_w, target_h, resizing_algorithm),
        "auto" => {
            let src_is_portrait = super::utils::is_portrait(src_width, src_height);
            let target_is_portrait = super::utils::is_portrait(target_w, target_h);

            if src_is_portrait == target_is_portrait {
                debug!("Auto resize: orientations match, using fill");
                resize_to_fill(
                    img,
                    target_w,
                    target_h,
                    gravity.as_deref().unwrap_or("center"),
                    resizing_algorithm,
                )
            } else {
                debug!("Auto resize: orientations differ, using fit");
                resize_to_fit(img, target_w, target_h, resizing_algorithm)
            }
        }
        _ => Err(format!("Unknown resize type: {}", resize.resizing_type)),
    }
}

/// Resizes an image to fill the target dimensions, cropping if necessary.
fn resize_to_fill(
    img: VipsImage,
    width: u32,
    height: u32,
    gravity: &str,
    resizing_algorithm: &Option<String>,
) -> Result<VipsImage, String> {
    let (img_w, img_h) = (img.get_width() as u32, img.get_height() as u32);
    let aspect_ratio = img_w as f32 / img_h as f32;
    let target_aspect_ratio = width as f32 / height as f32;

    let mut scale = if aspect_ratio > target_aspect_ratio {
        height as f64 / img_h as f64
    } else {
        width as f64 / img_w as f64
    };
    // Bump the scale slightly so kernels that round down still cover the target.
    scale *= 1.0 + SCALE_EPSILON;

    let resized_img = resize_with_algorithm(&img, scale, None, resizing_algorithm, "Error resizing for fill")?;

    let resized_w = resized_img.get_width() as u32;
    let resized_h = resized_img.get_height() as u32;

    if resized_w < width || resized_h < height {
        return Err(format!(
            "Resized image {}x{} is smaller than fill target {}x{}",
            resized_w, resized_h, width, height
        ));
    }

    let extra_w = resized_w - width;
    let extra_h = resized_h - height;

    let crop_x = match gravity {
        "west" => 0,
        "east" => extra_w,
        _ => extra_w / 2,
    };

    let crop_y = match gravity {
        "north" => 0,
        "south" => extra_h,
        _ => extra_h / 2,
    };

    ops::extract_area(&resized_img, crop_x as i32, crop_y as i32, width as i32, height as i32)
        .map_err(|e| format!("Error cropping after fill resize: {}", e))
}

/// Resizes an image to the exact target dimensions, allowing aspect ratio changes.
fn resize_to_force(
    img: VipsImage,
    width: u32,
    height: u32,
    resizing_algorithm: &Option<String>,
) -> Result<VipsImage, String> {
    let (src_w, src_h) = (img.get_width() as f64, img.get_height() as f64);
    let scale_x = width as f64 / src_w;
    let scale_y = height as f64 / src_h;

    if (scale_x - 1.0).abs() < SCALE_EPSILON && (scale_y - 1.0).abs() < SCALE_EPSILON {
        return Ok(img);
    }
    resize_with_algorithm(&img, scale_x, Some(scale_y), resizing_algorithm, "Error force resizing")
}

/// Resizes an image to fit within the target dimensions while maintaining aspect ratio.
fn resize_to_fit(
    img: VipsImage,
    width: u32,
    height: u32,
    resizing_algorithm: &Option<String>,
) -> Result<VipsImage, String> {
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
    let scale = scale_w.min(scale_h);

    resize_with_algorithm(&img, scale, None, resizing_algorithm, "Error fitting resize")
}

/// Extends an image to the target dimensions with background color.
pub fn extend_image(
    img: VipsImage,
    width: u32,
    height: u32,
    gravity: &Option<String>,
    background: &Option<[u8; 4]>,
) -> Result<VipsImage, String> {
    let _bg_color = background.unwrap_or([0, 0, 0, 0]);
    let gravity = gravity.as_deref().unwrap_or("center");

    let (x, y) = match gravity {
        "center" => (
            (width - img.get_width() as u32) / 2,
            (height - img.get_height() as u32) / 2,
        ),
        "north" => ((width - img.get_width() as u32) / 2, 0),
        "south" => ((width - img.get_width() as u32) / 2, height - img.get_height() as u32),
        "west" => (0, (height - img.get_height() as u32) / 2),
        "east" => (width - img.get_width() as u32, (height - img.get_height() as u32) / 2),
        _ => (
            (width - img.get_width() as u32) / 2,
            (height - img.get_height() as u32) / 2,
        ),
    };

    ops::embed(&img, x as i32, y as i32, width as i32, height as i32)
        .map_err(|e| format!("Error extending image: {}", e))
}

/// Applies padding to an image.
pub fn apply_padding(
    img: VipsImage,
    top: u32,
    right: u32,
    bottom: u32,
    left: u32,
    background: &Option<[u8; 4]>,
) -> Result<VipsImage, String> {
    let _bg_color = background.unwrap_or([0, 0, 0, 0]);

    ops::embed(
        &img,
        -(left as i32),
        -(top as i32),
        img.get_width() + left as i32 + right as i32,
        img.get_height() + top as i32 + bottom as i32,
    )
    .map_err(|e| format!("Error applying padding: {}", e))
}

/// Applies rotation to an image.
pub fn apply_rotation(img: VipsImage, rotation: u16) -> Result<VipsImage, String> {
    match rotation {
        90 => ops::rot(&img, ops::Angle::D90).map_err(|e| format!("Error rotating 90: {}", e)),
        180 => ops::rot(&img, ops::Angle::D180).map_err(|e| format!("Error rotating 180: {}", e)),
        270 => ops::rot(&img, ops::Angle::D270).map_err(|e| format!("Error rotating 270: {}", e)),
        _ => Ok(img), // No rotation
    }
}

/// Applies blur to an image.
pub fn apply_blur(img: VipsImage, sigma: f32) -> Result<VipsImage, String> {
    ops::gaussblur(&img, sigma as f64).map_err(|e| format!("Error applying blur: {}", e))
}

/// Applies background color to an image (useful for JPEG output).
pub fn apply_background_color(img: VipsImage, _bg_color: [u8; 4]) -> Result<VipsImage, String> {
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
    ops::flatten_with_opts(&img, &opts).map_err(|e| format!("Error applying background color: {}", e))
}

/// Applies min-width and min-height constraints to an image.
pub fn apply_min_dimensions(
    img: VipsImage,
    min_width: Option<u32>,
    min_height: Option<u32>,
    resizing_algorithm: &Option<String>,
) -> Result<VipsImage, String> {
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
pub fn apply_zoom(img: VipsImage, zoom: f32, resizing_algorithm: &Option<String>) -> Result<VipsImage, String> {
    resize_with_algorithm(&img, zoom as f64, None, resizing_algorithm, "Error applying zoom")
}

/// Sharpens an image.
pub fn apply_sharpen(img: VipsImage, sigma: f32) -> Result<VipsImage, String> {
    let clamped_sigma = sigma.clamp(0.1, 10.0);
    let opts = ops::SharpenOptions {
        sigma: clamped_sigma as f64,
        ..Default::default()
    };
    ops::sharpen_with_opts(&img, &opts).map_err(|e| format!("Error applying sharpen: {}", e))
}

/// Pixelates an image.
pub fn apply_pixelate(img: VipsImage, amount: u32, resizing_algorithm: &Option<String>) -> Result<VipsImage, String> {
    if amount == 0 {
        return Ok(img);
    }
    let (w, _h) = (img.get_width(), img.get_height());
    let factor = 1.0 / amount as f64;
    let pixelated = resize_with_algorithm(&img, factor, None, resizing_algorithm, "Error pixelating (down)")?;
    resize_with_algorithm(
        &pixelated,
        w as f64 / pixelated.get_width() as f64,
        None,
        resizing_algorithm,
        "Error pixelating (up)",
    )
}

/// Adjusts the brightness of an image.
pub fn apply_brightness(img: VipsImage, brightness: i32) -> Result<VipsImage, String> {
    if brightness == 0 {
        return Ok(img);
    }

    let mult = 1.0;
    let offset = brightness as f64 / 255.0;

    ops::linear(&img, &mut [mult], &mut [offset]).map_err(|e| format!("Error applying brightness: {}", e))
}
