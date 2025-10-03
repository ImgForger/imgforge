//! Image processing module for imgforge.
//! This module contains functions and structs for parsing image processing options
//! and applying various transformations to images.

use crate::ProcessingOption;
use exif::{In, Tag};
use libvips::{ops, VipsImage};
use std::io::Cursor;
use tracing::{debug, error};

/// Option name for resizing.
const RESIZE: &str = "resize";
/// Shorthand for resize.
const RESIZE_SHORT: &str = "rs";
/// Option name for resizing type.
const RESIZING_TYPE: &str = "resizing_type";
/// Shorthand for resizing type.
const RESIZING_TYPE_SHORT: &str = "rt";
/// Option name for size.
const SIZE: &str = "size";
/// Shorthand for size.
const SIZE_SHORT: &str = "sz";
/// Option name for width.
const WIDTH: &str = "width";
/// Shorthand for width.
const WIDTH_SHORT: &str = "w";
/// Option name for height.
const HEIGHT: &str = "height";
/// Shorthand for height.
const HEIGHT_SHORT: &str = "h";
/// Option name for gravity.
const GRAVITY: &str = "gravity";
/// Shorthand for gravity.
const GRAVITY_SHORT: &str = "g";
/// Option name for quality.
const QUALITY: &str = "quality";
/// Shorthand for quality.
const QUALITY_SHORT: &str = "q";
/// Option name for auto_rotate.
const AUTO_ROTATE: &str = "auto_rotate";
/// Shorthand for auto_rotate.
const AUTO_ROTATE_SHORT: &str = "ar";
/// Option name for background.
const BACKGROUND: &str = "background";
/// Shorthand for background.
const BACKGROUND_SHORT: &str = "bg";
/// Option name for enlarge.
const ENLARGE: &str = "enlarge";
/// Shorthand for enlarge.
const ENLARGE_SHORT: &str = "el";
/// Option name for extend.
const EXTEND: &str = "extend";
/// Shorthand for extend.
const EXTEND_SHORT: &str = "ex";
/// Option name for padding.
const PADDING: &str = "padding";
/// Shorthand for padding.
const PADDING_SHORT: &str = "pd";
/// Option name for rotation.
const ROTATE: &str = "rotation";
/// Shorthand for rotation.
const ROTATE_SHORT: &str = "or";
/// Option name for raw.
const RAW: &str = "raw";
/// Option name for blur.
const BLUR: &str = "blur";
/// Shorthand for blur.
const BLUR_SHORT: &str = "bl";
/// Option name for crop.
const CROP: &str = "crop";
/// Option name for format.
const FORMAT: &str = "format";
/// Option name for max_src_resolution.
const MAX_SRC_RESOLUTION: &str = "max_src_resolution";
/// Option name for max_src_file_size.
const MAX_SRC_FILE_SIZE: &str = "max_src_file_size";
/// Option name for cache_buster.
const CACHE_BUSTER: &str = "cache_buster";
/// Option name for dpr.
const DPR: &str = "dpr";

/// Represents the parameters for a resize operation.
#[derive(Debug, Default)]
pub struct Resize {
    /// The type of resizing to perform (e.g., "fill", "fit", "force").
    pub resizing_type: String,
    /// The target width for the resize operation.
    pub width: u32,
    /// The target height for the resize operation.
    pub height: u32,
}

/// Represents the parameters for a crop operation.
#[derive(Debug, Default)]
pub struct Crop {
    /// The x-coordinate of the top-left corner of the crop area.
    pub x: u32,
    /// The y-coordinate of the top-left corner of the crop area.
    pub y: u32,
    /// The width of the crop area.
    pub width: u32,
    /// The height of the crop area.
    pub height: u32,
}

/// Holds all parsed image processing options.
#[derive(Debug)]
pub struct ParsedOptions {
    /// Optional resize operation parameters.
    pub resize: Option<Resize>,
    /// Optional blur sigma value.
    pub blur: Option<f32>,
    /// Optional crop operation parameters.
    pub crop: Option<Crop>,
    /// Optional output image format.
    pub format: Option<String>,
    /// Optional output image quality (1-100).
    pub quality: Option<u8>,
    /// Optional background color for transparent areas or extending.
    pub background: Option<[u8; 4]>, // RGBA array
    /// Optional target width (used with `resize` if no explicit resize type).
    pub width: Option<u32>,
    /// Optional target height (used with `resize` if no explicit resize type).
    pub height: Option<u32>,
    /// Optional gravity for cropping or extending (e.g., "center", "north").
    pub gravity: Option<String>,
    /// Whether to allow enlarging the image beyond its original dimensions.
    pub enlarge: bool,
    /// Whether to extend the image with a background if target dimensions are larger.
    pub extend: bool,
    /// Optional padding values (top, right, bottom, left).
    pub padding: Option<(u32, u32, u32, u32)>,
    /// Optional image rotation (rotation angle).
    pub rotation: Option<u16>,
    /// Whether to automatically rotate the image based on EXIF data.
    pub auto_rotate: bool,
    /// Whether to bypass processing limits (e.g., worker limits).
    pub raw: bool,
    /// Maximum allowed source image resolution in megapixels.
    pub max_src_resolution: Option<f32>,
    /// Maximum allowed source image file size in bytes.
    pub max_src_file_size: Option<usize>,
    /// Value to bypass cache (e.g., timestamp).
    pub cache_buster: Option<String>,
    /// Device pixel ratio factor to scale up dimensions.
    pub dpr: Option<f32>,
}

impl Default for ParsedOptions {
    fn default() -> Self {
        Self {
            resize: None,
            blur: None,
            crop: None,
            format: None,
            quality: None,
            background: None,
            width: None,
            height: None,
            gravity: None,
            enlarge: false,
            extend: false,
            padding: None,
            rotation: None,
            auto_rotate: true,
            raw: false,
            max_src_resolution: None,
            max_src_file_size: None,
            cache_buster: None,
            dpr: Some(1.0),
        }
    }
}

/// Parses a hexadecimal color string into an RGBA array.
///
/// # Arguments
///
/// * `hex` - The hexadecimal color string (e.g., "ffffff" or "#ffffff").
///
/// # Returns
///
/// A `Result` containing the RGBA array on success, or an error message as a `String`.
fn parse_hex_color(hex: &str) -> Result<[u8; 4], String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Invalid hex color format".to_string());
    }
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex color".to_string())?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex color".to_string())?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex color".to_string())?;
    Ok([r, g, b, 255])
}

/// Parses a string into a boolean value.
///
/// # Arguments
///
/// * `s` - The string to parse ("1", "true" for true, anything else for false).
///
/// # Returns
///
/// `true` if the string is "1" or "true" (case-sensitive), `false` otherwise.
fn parse_boolean(s: &str) -> bool {
    matches!(s, "1" | "true")
}

/// Determines if the given dimensions represent a portrait orientation.
///
/// # Arguments
///
/// * `width` - The width of the image.
/// * `height` - The height of the image.
///
/// # Returns
///
/// `true` if the height is greater than the width, `false` otherwise.
fn is_portrait(width: u32, height: u32) -> bool {
    height > width
}

/// Parses a vector of `ProcessingOption` into a `ParsedOptions` struct.
///
/// This function iterates through the raw processing options, validates their arguments,
/// and converts them into a structured `ParsedOptions` object.
///
/// # Arguments
///
/// * `options` - A `Vec<ProcessingOption>` containing the raw options from the URL.
///
/// # Returns
///
/// A `Result` containing the `ParsedOptions` on success, or an error message as a `String`.
pub fn parse_all_options(options: Vec<ProcessingOption>) -> Result<ParsedOptions, String> {
    let mut parsed_options = ParsedOptions::default();

    for option in options {
        debug!("Parsing option: {} with args: {:?}", option.name, option.args);
        match option.name.as_str() {
            RESIZE | RESIZE_SHORT => {
                if option.args.len() < 3 {
                    error!(
                        "Resize option requires at least 3 arguments, received: {}",
                        option.args.len()
                    );
                    return Err("resize option requires at least 3 arguments: type, width, height".to_string());
                }
                parsed_options.resize = Some(Resize {
                    resizing_type: option.args[0].clone(),
                    width: option.args[1].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid width for resize: {}", e);
                        e.to_string()
                    })?,
                    height: option.args[2].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid height for resize: {}", e);
                        e.to_string()
                    })?,
                });
            }
            RESIZING_TYPE | RESIZING_TYPE_SHORT => {
                if parsed_options.resize.is_none() {
                    parsed_options.resize = Some(Resize::default());
                }
                if let Some(ref mut resize) = parsed_options.resize {
                    resize.resizing_type = option.args[0].clone();
                }
            }
            SIZE | SIZE_SHORT => {
                if option.args.len() < 2 {
                    return Err("size option requires at least 2 arguments: width, height".to_string());
                }
                parsed_options.resize = Some(Resize {
                    resizing_type: "fit".to_string(),
                    width: option.args[0].parse::<u32>().map_err(|e| e.to_string())?,
                    height: option.args[1].parse::<u32>().map_err(|e| e.to_string())?,
                });
            }
            WIDTH | WIDTH_SHORT => {
                if option.args.is_empty() {
                    error!("Width option requires one argument");
                    return Err("width option requires one argument".to_string());
                }
                parsed_options.width = Some(option.args[0].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                    error!("Invalid width: {}", e);
                    e.to_string()
                })?);
            }
            HEIGHT | HEIGHT_SHORT => {
                if option.args.is_empty() {
                    error!("Height option requires one argument");
                    return Err("height option requires one argument".to_string());
                }
                parsed_options.height = Some(option.args[0].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                    error!("Invalid height: {}", e);
                    e.to_string()
                })?);
            }
            GRAVITY | GRAVITY_SHORT => {
                if option.args.is_empty() {
                    error!("Gravity option requires one argument");
                    return Err("gravity option requires one argument".to_string());
                }
                parsed_options.gravity = Some(option.args[0].clone());
            }
            ENLARGE | ENLARGE_SHORT => {
                if option.args.is_empty() {
                    error!("Enlarge option requires one argument");
                    return Err("enlarge option requires one argument".to_string());
                }
                parsed_options.enlarge = parse_boolean(&option.args[0]);
            }
            EXTEND | EXTEND_SHORT => {
                if option.args.is_empty() {
                    error!("Extend option requires one argument");
                    return Err("extend option requires one argument".to_string());
                }
                parsed_options.extend = parse_boolean(&option.args[0]);
            }
            PADDING | PADDING_SHORT => {
                if option.args.is_empty() {
                    error!("Padding option requires at least one argument");
                    return Err("padding option requires at least one argument".to_string());
                }
                let values: Vec<u32> = option
                    .args
                    .iter()
                    .map(|s| {
                        s.parse::<u32>().map_err(|e: std::num::ParseIntError| {
                            error!("Invalid padding value: {}", e);
                            e.to_string()
                        })
                    })
                    .collect::<Result<Vec<u32>, String>>()?;
                parsed_options.padding = Some(match values.len() {
                    1 => (values[0], values[0], values[0], values[0]),
                    2 => (values[0], values[1], values[0], values[1]),
                    4 => (values[0], values[1], values[2], values[3]),
                    _ => {
                        error!("Padding must have 1, 2, or 4 arguments, received: {}", values.len());
                        return Err("padding must have 1, 2, or 4 arguments".to_string());
                    }
                });
            }
            ROTATE | ROTATE_SHORT => {
                if option.args.is_empty() {
                    error!("Rotation option requires one argument");
                    return Err("rotation option requires one argument".to_string());
                }
                parsed_options.rotation =
                    Some(option.args[0].parse::<u16>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid rotation: {}", e);
                        e.to_string()
                    })?);
            }
            AUTO_ROTATE | AUTO_ROTATE_SHORT => {
                if option.args.is_empty() {
                    error!("Auto_rotate option requires one argument");
                    return Err("auto_rotate option requires one argument".to_string());
                }
                parsed_options.auto_rotate = parse_boolean(&option.args[0]);
            }
            RAW => {
                parsed_options.raw = true;
            }
            BLUR | BLUR_SHORT => {
                if option.args.is_empty() {
                    error!("Blur option requires one argument: sigma");
                    return Err("blur option requires one argument: sigma".to_string());
                }
                parsed_options.blur = Some(option.args[0].parse::<f32>().map_err(|e: std::num::ParseFloatError| {
                    error!("Invalid sigma for blur: {}", e);
                    e.to_string()
                })?);
            }
            CROP => {
                if option.args.len() < 4 {
                    error!("Crop option requires four arguments");
                    return Err("crop option requires four arguments: x, y, width, height".to_string());
                }
                parsed_options.crop = Some(Crop {
                    x: option.args[0].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid x for crop: {}", e);
                        e.to_string()
                    })?,
                    y: option.args[1].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid y for crop: {}", e);
                        e.to_string()
                    })?,
                    width: option.args[2].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid width for crop: {}", e);
                        e.to_string()
                    })?,
                    height: option.args[3].parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid height for crop: {}", e);
                        e.to_string()
                    })?,
                });
            }
            FORMAT => {
                if option.args.is_empty() {
                    error!("Format option requires one argument");
                    return Err("format option requires one argument".to_string());
                }
                parsed_options.format = Some(option.args[0].clone());
            }
            QUALITY | QUALITY_SHORT => {
                if option.args.is_empty() {
                    error!("Quality option requires one argument");
                    return Err("quality option requires one argument".to_string());
                }
                parsed_options.quality = Some(
                    option.args[0]
                        .parse::<u8>()
                        .map_err(|e| {
                            error!("Invalid quality: {}", e);
                            e.to_string()
                        })?
                        .clamp(1, 100),
                );
            }
            BACKGROUND | BACKGROUND_SHORT => {
                if option.args.is_empty() {
                    error!("Background option requires one argument");
                    return Err("background option requires one argument".to_string());
                }
                parsed_options.background = Some(parse_hex_color(&option.args[0]).map_err(|e| {
                    error!("Invalid hex color for background: {}", e);
                    e.to_string()
                })?);
            }
            MAX_SRC_RESOLUTION => {
                if option.args.is_empty() {
                    error!("Max_src_resolution option requires one argument");
                    return Err("max_src_resolution option requires one argument".to_string());
                }
                parsed_options.max_src_resolution =
                    Some(option.args[0].parse::<f32>().map_err(|e: std::num::ParseFloatError| {
                        error!("Invalid max_src_resolution: {}", e);
                        e.to_string()
                    })?);
            }
            MAX_SRC_FILE_SIZE => {
                if option.args.is_empty() {
                    error!("Max_src_file_size option requires one argument");
                    return Err("max_src_file_size option requires one argument".to_string());
                }
                parsed_options.max_src_file_size =
                    Some(option.args[0].parse::<usize>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid max_src_file_size: {}", e);
                        e.to_string()
                    })?);
            }
            CACHE_BUSTER => {
                if option.args.is_empty() {
                    error!("Cache_buster option requires one argument");
                    return Err("cache_buster option requires one argument".to_string());
                }
                parsed_options.cache_buster = Some(option.args[0].clone());
            }
            DPR => {
                if option.args.is_empty() {
                    error!("DPR option requires one argument");
                    return Err("dpr option requires one argument".to_string());
                }
                let dpr = option.args[0].parse::<f32>().map_err(|e| {
                    error!("Invalid dpr value: {}", e);
                    e.to_string()
                })?;
                if !(1.0..=5.0).contains(&dpr) {
                    error!("DPR value must be between 1.0 and 5.0, received: {}", dpr);
                    return Err("dpr value must be between 1.0 and 5.0".to_string());
                }
                parsed_options.dpr = Some(dpr);
            }
            _ => {
                debug!("Unknown option: {}", option.name);
            }
        }
    }

    if parsed_options.resize.is_none() && (parsed_options.width.is_some() || parsed_options.height.is_some()) {
        debug!("Applying default 'fit' resize due to width/height options");
        parsed_options.resize = Some(Resize {
            resizing_type: "fit".to_string(),
            width: parsed_options.width.unwrap_or(0),
            height: parsed_options.height.unwrap_or(0),
        });
    }

    Ok(parsed_options)
}

/// Applies EXIF rotation to an image based on orientation data.
fn apply_exif_rotation(image_bytes: &[u8], mut img: VipsImage) -> Result<VipsImage, String> {
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
fn crop_image(img: VipsImage, crop: Crop) -> Result<VipsImage, String> {
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
fn apply_resize(img: VipsImage, resize: &Resize, gravity: &Option<String>) -> Result<VipsImage, String> {
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
            let src_is_portrait = is_portrait(img.get_width() as u32, img.get_height() as u32);
            let target_is_portrait = is_portrait(w, h);

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
fn extend_image(
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
fn apply_padding(
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
        (img.get_width() + left as i32 + right as i32) as i32,
        (img.get_height() + top as i32 + bottom as i32) as i32,
    )
    .map_err(|e| format!("Error applying padding: {}", e))
}

/// Applies rotation to an image.
fn apply_rotation(img: VipsImage, rotation: u16) -> Result<VipsImage, String> {
    match rotation {
        90 => ops::rot(&img, ops::Angle::D90).map_err(|e| format!("Error rotating 90: {}", e)),
        180 => ops::rot(&img, ops::Angle::D180).map_err(|e| format!("Error rotating 180: {}", e)),
        270 => ops::rot(&img, ops::Angle::D270).map_err(|e| format!("Error rotating 270: {}", e)),
        _ => Ok(img), // No rotation
    }
}

/// Applies blur to an image.
fn apply_blur(img: VipsImage, sigma: f32) -> Result<VipsImage, String> {
    ops::gaussblur(&img, sigma as f64).map_err(|e| format!("Error applying blur: {}", e))
}

/// Applies background color to an image (useful for JPEG output).
fn apply_background_color(img: VipsImage, _bg_color: [u8; 4]) -> Result<VipsImage, String> {
    // Use libvips flatten to composite over a solid background, dropping alpha.
    // Only RGB is used; input alpha is ignored for the background color itself.
    let bg = vec![
        _bg_color[0] as f64,
        _bg_color[1] as f64,
        _bg_color[2] as f64,
    ];
    let mut opts = ops::FlattenOptions::default();
    opts.background = bg;
    ops::flatten_with_opts(&img, &opts).map_err(|e| format!("Error applying background color: {}", e))
}

/// Saves an image to bytes in the specified format.
fn save_image(img: VipsImage, format: &str, _quality: u8) -> Result<Vec<u8>, String> {
    match format {
        "jpeg" | "jpg" => ops::jpegsave_buffer(&img).map_err(|e| format!("Error encoding JPEG: {}", e)),
        "png" => ops::pngsave_buffer(&img).map_err(|e| format!("Error encoding PNG: {}", e)),
        "webp" => ops::webpsave_buffer(&img).map_err(|e| format!("Error encoding WebP: {}", e)),
        "tiff" => ops::tiffsave_buffer(&img).map_err(|e| format!("Error encoding TIFF: {}", e)),
        "gif" => ops::gifsave_buffer(&img).map_err(|e| format!("Error encoding GIF: {}", e)),
        _ => Err(format!("Unsupported output format: {}", format)),
    }
}

/// Processes an image by applying the given `ParsedOptions`.
///
/// This function takes raw image bytes and a set of parsed options, applies
/// transformations like resizing, cropping, blurring, and format conversion,
/// then returns the processed image bytes.
///
/// # Arguments
///
/// * `image_bytes` - The raw bytes of the image to process.
/// * `parsed_options` - A `ParsedOptions` struct containing the desired transformations.
///
/// # Returns
///
/// A `Result` containing the processed image bytes on success, or an error message as a `String`.
pub async fn process_image(image_bytes: Vec<u8>, mut parsed_options: ParsedOptions) -> Result<Vec<u8>, String> {
    debug!("Starting image processing with options: {:?}", parsed_options);

    // Apply DPR scaling
    if let Some(dpr) = parsed_options.dpr {
        if dpr > 1.0 {
            debug!("Applying DPR scaling: {}", dpr);
            if let Some(ref mut resize) = parsed_options.resize {
                debug!(
                    "Scaling resize dimensions from {}x{} to {}x{}",
                    resize.width,
                    resize.height,
                    (resize.width as f32 * dpr).round() as u32,
                    (resize.height as f32 * dpr).round() as u32
                );
                resize.width = (resize.width as f32 * dpr).round() as u32;
                resize.height = (resize.height as f32 * dpr).round() as u32;
            }
            if let Some(ref mut padding) = parsed_options.padding {
                debug!(
                    "Scaling padding from {:?} to {:?}",
                    padding,
                    (
                        (padding.0 as f32 * dpr).round() as u32,
                        (padding.1 as f32 * dpr).round() as u32,
                        (padding.2 as f32 * dpr).round() as u32,
                        (padding.3 as f32 * dpr).round() as u32
                    )
                );
                padding.0 = (padding.0 as f32 * dpr).round() as u32;
                padding.1 = (padding.1 as f32 * dpr).round() as u32;
                padding.2 = (padding.2 as f32 * dpr).round() as u32;
                padding.3 = (padding.3 as f32 * dpr).round() as u32;
            }
        }
    }

    // Load image from bytes
    let mut img = VipsImage::new_from_buffer(&image_bytes, "").map_err(|e| {
        error!("Error loading image from memory: {}", e);
        format!("Error loading image from memory: {}", e)
    })?;

    debug!("Loaded image: {}x{}", img.get_width(), img.get_height());

    // Apply EXIF auto-rotation if enabled
    if parsed_options.auto_rotate {
        debug!("Applying EXIF auto-rotation");
        img = apply_exif_rotation(&image_bytes, img)?;
    }

    // Apply crop if specified
    if let Some(crop) = parsed_options.crop {
        debug!("Applying crop: {:?}", crop);
        img = crop_image(img, crop)?;
    }

    // Apply resize if specified
    if let Some(ref resize) = parsed_options.resize {
        debug!("Applying resize: {:?}", resize);
        let (w, h) = (resize.width, resize.height);

        if !parsed_options.enlarge && (w > img.get_width() as u32 || h > img.get_height() as u32) {
            debug!("Not enlarging image as enlarge is false and target dimensions are larger than source");
        } else {
            img = apply_resize(img, resize, &parsed_options.gravity)?;
        }
    }

    // Apply extend if specified
    if parsed_options.extend {
        debug!("Applying extend option");
        if let Some(resize) = &parsed_options.resize {
            let (w, h) = (resize.width, resize.height);
            if img.get_width() < w as i32 || img.get_height() < h as i32 {
                img = extend_image(img, w, h, &parsed_options.gravity, &parsed_options.background)?;
            }
        }
    }

    // Apply padding if specified
    if let Some((top, right, bottom, left)) = parsed_options.padding {
        debug!("Applying padding: {:?}", (top, right, bottom, left));
        img = apply_padding(img, top, right, bottom, left, &parsed_options.background)?;
    }

    // Apply rotation if specified
    if let Some(rotation) = parsed_options.rotation {
        debug!("Applying rotation: {}", rotation);
        img = apply_rotation(img, rotation)?;
    }

    // Apply blur if specified
    if let Some(sigma) = parsed_options.blur {
        debug!("Applying blur with sigma: {}", sigma);
        img = apply_blur(img, sigma)?;
    }

    // Apply background color for JPEG if needed
    let output_format = parsed_options.format.as_deref().unwrap_or("jpeg");
    if let Some(bg_color) = parsed_options.background {
        if output_format == "jpeg" {
            debug!("Applying background color for JPEG output: {:?}", bg_color);
            img = apply_background_color(img, bg_color)?;
        }
    }

    // Save image to bytes
    let quality = parsed_options.quality.unwrap_or(85);
    let output_bytes = save_image(img, output_format, quality)?;

    debug!("Image processing complete");
    Ok(output_bytes)
}
