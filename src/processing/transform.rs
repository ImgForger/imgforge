use crate::processing::options::{Crop, Resize};
use exif::{In, Tag};
use libvips::{ops, VipsImage};
use std::io::Cursor;
use tracing::debug;

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

/// Applies resize operation based on the resize type.
pub fn apply_resize(img: VipsImage, resize: &Resize, gravity: &Option<String>) -> Result<VipsImage, String> {
    let (w, h) = (resize.width, resize.height);

    match resize.resizing_type.as_str() {
        "fill" => {
            if w == 0 || h == 0 {
                return Err("resize:fill requires non-zero width and height".to_string());
            }
            resize_to_fill(img, w, h, gravity.as_deref().unwrap_or("center"))
        }
        "fit" => {
            if w == 0 && h == 0 {
                return Err("resize:fit requires non-zero width and height".to_string());
            }
            resize_to_fit(img, w, h)
        }
        "force" => {
            if w == 0 || h == 0 {
                return Err("resize:force requires non-zero width and height".to_string());
            }
            ops::resize(&img, w as f64 / img.get_width() as f64).map_err(|e| format!("Error force resizing: {}", e))
        }
        "auto" => {
            if w == 0 || h == 0 {
                return Err("resize:auto requires non-zero width and height".to_string());
            }
            let src_is_portrait = super::utils::is_portrait(img.get_width() as u32, img.get_height() as u32);
            let target_is_portrait = super::utils::is_portrait(w, h);

            if src_is_portrait == target_is_portrait {
                debug!("Auto resize: orientations match, using fill");
                resize_to_fill(img, w, h, gravity.as_deref().unwrap_or("center"))
            } else {
                debug!("Auto resize: orientations differ, using fit");
                resize_to_fit(img, w, h)
            }
        }
        _ => Err(format!("Unknown resize type: {}", resize.resizing_type)),
    }
}

/// Resizes an image to fill the target dimensions, cropping if necessary.
fn resize_to_fill(img: VipsImage, width: u32, height: u32, gravity: &str) -> Result<VipsImage, String> {
    let (img_w, img_h) = (img.get_width() as u32, img.get_height() as u32);
    let aspect_ratio = img_w as f32 / img_h as f32;
    let target_aspect_ratio = width as f32 / height as f32;

    let (resize_w, resize_h) = if aspect_ratio > target_aspect_ratio {
        ((height as f32 * aspect_ratio).round() as u32, height)
    } else {
        (width, (width as f32 / aspect_ratio).round() as u32)
    };

    let resized_img =
        ops::resize(&img, resize_w as f64 / img_w as f64).map_err(|e| format!("Error resizing for fill: {}", e))?;

    let (crop_x, crop_y) = match gravity {
        "center" => ((resize_w - width) / 2, (resize_h - height) / 2),
        "north" => ((resize_w - width) / 2, 0),
        "south" => ((resize_w - width) / 2, resize_h - height),
        "west" => (0, (resize_h - height) / 2),
        "east" => (resize_w - width, (resize_h - height) / 2),
        _ => ((resize_w - width) / 2, (resize_h - height) / 2), // Default to center
    };

    ops::extract_area(&resized_img, crop_x as i32, crop_y as i32, width as i32, height as i32)
        .map_err(|e| format!("Error cropping after fill resize: {}", e))
}

/// Resizes an image to fit within the target dimensions while maintaining aspect ratio.
fn resize_to_fit(img: VipsImage, width: u32, height: u32) -> Result<VipsImage, String> {
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
    ops::resize(&img, target_w as f64 / img_w as f64).map_err(|e| format!("Error fitting resize: {}", e))
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
