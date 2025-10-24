use crate::processing::options::{Crop, Resize, Watermark};
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

    let resized_img = if resizing_algorithm.is_some() && resizing_algorithm.as_deref() != Some("lanczos3") {
        let kernel = get_resize_kernel(resizing_algorithm);
        let options = ops::ResizeOptions {
            kernel,
            vscale: scale,
            ..Default::default()
        };
        ops::resize_with_opts(&img, scale, &options).map_err(|e| format!("Error resizing for fill: {}", e))?
    } else {
        ops::resize(&img, scale).map_err(|e| format!("Error resizing for fill: {}", e))?
    };

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

    if resizing_algorithm.is_some() && resizing_algorithm.as_deref() != Some("lanczos3") {
        let kernel = get_resize_kernel(resizing_algorithm);
        let options = ops::ResizeOptions {
            kernel,
            vscale: scale_y,
            ..Default::default()
        };
        ops::resize_with_opts(&img, scale_x, &options).map_err(|e| format!("Error force resizing: {}", e))
    } else {
        let mut options = ops::ResizeOptions::default();
        options.vscale = scale_y;
        ops::resize_with_opts(&img, scale_x, &options).map_err(|e| format!("Error force resizing: {}", e))
    }
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

    if resizing_algorithm.is_some() && resizing_algorithm.as_deref() != Some("lanczos3") {
        let kernel = get_resize_kernel(resizing_algorithm);
        let options = ops::ResizeOptions {
            kernel,
            vscale: scale,
            ..Default::default()
        };
        ops::resize_with_opts(&img, scale, &options).map_err(|e| format!("Error fitting resize: {}", e))
    } else {
        ops::resize(&img, scale).map_err(|e| format!("Error fitting resize: {}", e))
    }
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
        current_img = if resizing_algorithm.is_some() && resizing_algorithm.as_deref() != Some("lanczos3") {
            let kernel = get_resize_kernel(resizing_algorithm);
            let options = ops::ResizeOptions {
                kernel,
                ..Default::default()
            };
            ops::resize_with_opts(&current_img, scale, &options)
                .map_err(|e| format!("Error applying min dimensions: {}", e))?
        } else {
            ops::resize(&current_img, scale).map_err(|e| format!("Error applying min dimensions: {}", e))?
        };
    }

    Ok(current_img)
}

/// Applies zoom to an image.
pub fn apply_zoom(img: VipsImage, zoom: f32, resizing_algorithm: &Option<String>) -> Result<VipsImage, String> {
    if resizing_algorithm.is_some() && resizing_algorithm.as_deref() != Some("lanczos3") {
        let kernel = get_resize_kernel(resizing_algorithm);
        let options = ops::ResizeOptions {
            kernel,
            ..Default::default()
        };
        ops::resize_with_opts(&img, zoom as f64, &options).map_err(|e| format!("Error applying zoom: {}", e))
    } else {
        ops::resize(&img, zoom as f64).map_err(|e| format!("Error applying zoom: {}", e))
    }
}

/// Sharpens an image. The sigma parameter controls the amount of sharpening.
pub fn apply_sharpen(img: VipsImage, sigma: f32) -> Result<VipsImage, String> {
    let clamped_sigma = sigma.clamp(0.1, 4.0);
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

    if resizing_algorithm.is_some() && resizing_algorithm.as_deref() != Some("lanczos3") {
        let kernel = get_resize_kernel(resizing_algorithm);
        let options = ops::ResizeOptions {
            kernel,
            ..Default::default()
        };
        let pixelated =
            ops::resize_with_opts(&img, factor, &options).map_err(|e| format!("Error pixelating (down): {}", e))?;
        ops::resize_with_opts(&pixelated, w as f64 / pixelated.get_width() as f64, &options)
            .map_err(|e| format!("Error pixelating (up): {}", e))
    } else {
        let pixelated = ops::resize(&img, factor).map_err(|e| format!("Error pixelating (down): {}", e))?;
        ops::resize(&pixelated, w as f64 / pixelated.get_width() as f64)
            .map_err(|e| format!("Error pixelating (up): {}", e))
    }
}

/// Applies a watermark to an image.
pub fn apply_watermark(
    img: VipsImage,
    watermark_bytes: &[u8],
    watermark_opts: &Watermark,
    resizing_algorithm: &Option<String>,
) -> Result<VipsImage, String> {
    let watermark_img = VipsImage::new_from_buffer(watermark_bytes, "")
        .map_err(|e| format!("Failed to load watermark image from buffer: {}", e))?;

    // Resize watermark to be 1/4 of the main image's width, maintaining aspect ratio
    let factor = (img.get_width() as f64 / 4.0) / watermark_img.get_width() as f64;
    let watermark_resized = if resizing_algorithm.is_some() && resizing_algorithm.as_deref() != Some("lanczos3") {
        let kernel = get_resize_kernel(resizing_algorithm);
        let options = ops::ResizeOptions {
            kernel,
            ..Default::default()
        };
        ops::resize_with_opts(&watermark_img, factor, &options)
            .map_err(|e| format!("Failed to resize watermark: {}", e))?
    } else {
        ops::resize(&watermark_img, factor).map_err(|e| format!("Failed to resize watermark: {}", e))?
    };

    // Add alpha channel to watermark if it doesn't have one
    let watermark_with_alpha = if watermark_resized.get_bands() == 4 || watermark_resized.get_bands() == 2 {
        watermark_resized
    } else {
        ops::bandjoin_const(&watermark_resized, &mut [255.0])
            .map_err(|e| format!("Failed to add alpha to watermark: {}", e))?
    };

    // Apply opacity
    let multipliers = &mut [1.0, 1.0, 1.0, watermark_opts.opacity as f64];
    let adders = &mut [0.0, 0.0, 0.0, 0.0];
    let watermark_with_opacity = ops::linear(&watermark_with_alpha, multipliers, adders)
        .map_err(|e| format!("Failed to apply opacity to watermark: {}", e))?;

    // Calculate position
    let (x, y) = calculate_watermark_position(&img, &watermark_with_opacity, &watermark_opts.position);

    // Composite watermark
    let bg = &mut [0.0, 0.0, 0.0, 0.0]; // transparent
    let options = ops::EmbedOptions {
        extend: ops::Extend::Background,
        background: bg.to_vec(),
    };

    let watermark_on_canvas = ops::embed_with_opts(
        &watermark_with_opacity,
        x as i32,
        y as i32,
        img.get_width(),
        img.get_height(),
        &options,
    )
    .map_err(|e| format!("Failed to embed watermark on canvas: {}", e))?;

    ops::composite_2(&img, &watermark_on_canvas, ops::BlendMode::Over)
        .map_err(|e| format!("Failed to composite watermark: {}", e))
}

fn calculate_watermark_position(main_img: &VipsImage, watermark_img: &VipsImage, position: &str) -> (u32, u32) {
    let main_w = main_img.get_width() as u32;
    let main_h = main_img.get_height() as u32;
    let wm_w = watermark_img.get_width() as u32;
    let wm_h = watermark_img.get_height() as u32;
    let margin = (main_w.min(main_h) as f32 * 0.05).round() as u32; // 5% margin

    match position {
        "north" => ((main_w - wm_w) / 2, margin),
        "south" => ((main_w - wm_w) / 2, main_h - wm_h - margin),
        "east" => (main_w - wm_w - margin, (main_h - wm_h) / 2),
        "west" => (margin, (main_h - wm_h) / 2),
        "north_west" => (margin, margin),
        "north_east" => (main_w - wm_w - margin, margin),
        "south_west" => (margin, main_h - wm_h - margin),
        "south_east" => (main_w - wm_w - margin, main_h - wm_h - margin),
        "center" => ((main_w - wm_w) / 2, (main_h - wm_h) / 2),
        _ => ((main_w - wm_w) / 2, (main_h - wm_h) / 2),
    }
}
