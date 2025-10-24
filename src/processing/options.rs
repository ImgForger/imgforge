//! Image processing module for imgforge.
//! This module contains functions and structs for parsing image processing options
//! and applying various transformations to images.

/// Represents a single image processing option from the URL path.
#[derive(Debug)]
pub struct ProcessingOption {
    /// The name of the processing option (e.g., "resize", "quality").
    pub name: String,
    /// Arguments for the processing option.
    pub args: Vec<String>,
}
use base64::engine::general_purpose;
use base64::Engine as _;
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
/// Alternate shorthand for size.
const SIZE_SHORT_ALT: &str = "s";
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
const ROTATE: &str = "rotate";
/// Shorthand for rotation.
const ROTATE_SHORT: &str = "rot";
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
/// Option name for min_width.
const MIN_WIDTH: &str = "min_width";
/// Shorthand for min_width.
const MIN_WIDTH_SHORT: &str = "mw";
/// Option name for min_height.
const MIN_HEIGHT: &str = "min_height";
/// Shorthand for min_height.
const MIN_HEIGHT_SHORT: &str = "mh";
/// Option name for zoom.
const ZOOM: &str = "zoom";
/// Shorthand for zoom.
const ZOOM_SHORT: &str = "z";
/// Option name for sharpen.
const SHARPEN: &str = "sharpen";
/// Shorthand for sharpen.
const SHARPEN_SHORT: &str = "sh";
/// Option name for pixelate.
const PIXELATE: &str = "pixelate";
/// Shorthand for pixelate.
const PIXELATE_SHORT: &str = "px";
/// Option name for watermark.
const WATERMARK: &str = "watermark";
/// Shorthand for watermark.
const WATERMARK_SHORT: &str = "wm";
/// Option name for watermark_url.
const WATERMARK_URL: &str = "watermark_url";
/// Shorthand for watermark_url.
const WATERMARK_URL_SHORT: &str = "wmu";
/// Option name for resizing_algorithm.
const RESIZING_ALGORITHM: &str = "resizing_algorithm";
/// Shorthand for resizing_algorithm.
const RESIZING_ALGORITHM_SHORT: &str = "ra";

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

/// Represents the parameters for a watermark operation.
#[derive(Debug, Clone, Default)]
pub struct Watermark {
    /// The opacity of the watermark.
    pub opacity: f32,
    /// The position of the watermark.
    pub position: String,
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
    /// Minimum width for the image.
    pub min_width: Option<u32>,
    /// Minimum height for the image.
    pub min_height: Option<u32>,
    /// Zoom factor for the image.
    pub zoom: Option<f32>,
    /// Sharpen factor for the image.
    pub sharpen: Option<f32>,
    /// Pixelate factor for the image.
    pub pixelate: Option<u32>,
    pub watermark: Option<Watermark>,
    /// Optional URL for a watermark image.
    pub watermark_url: Option<String>,
    /// Resizing algorithm to use (nearest, linear, cubic, lanczos2, lanczos3).
    pub resizing_algorithm: Option<String>,
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
            min_width: None,
            min_height: None,
            zoom: None,
            sharpen: None,
            pixelate: None,
            watermark: None,
            watermark_url: None,
            resizing_algorithm: Some("lanczos3".to_string()),
        }
    }
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
                let mut store_resize = parsed_options.resize.is_some();
                let mut resize = parsed_options.resize.take().unwrap_or_default();

                if let Some(arg) = option.args.get(0) {
                    if !arg.is_empty() {
                        resize.resizing_type = arg.clone();
                        store_resize = true;
                    }
                }
                if let Some(arg) = option.args.get(1) {
                    if !arg.is_empty() {
                        resize.width = arg.parse::<u32>().map_err(|e: std::num::ParseIntError| {
                            error!("Invalid width for resize: {}", e);
                            e.to_string()
                        })?;
                        store_resize = true;
                    }
                }
                if let Some(arg) = option.args.get(2) {
                    if !arg.is_empty() {
                        resize.height = arg.parse::<u32>().map_err(|e: std::num::ParseIntError| {
                            error!("Invalid height for resize: {}", e);
                            e.to_string()
                        })?;
                        store_resize = true;
                    }
                }
                if let Some(arg) = option.args.get(3) {
                    if !arg.is_empty() {
                        parsed_options.enlarge = super::utils::parse_boolean(arg);
                    }
                }
                if let Some(arg) = option.args.get(4) {
                    if !arg.is_empty() {
                        parsed_options.extend = super::utils::parse_boolean(arg);
                    }
                }

                if store_resize {
                    parsed_options.resize = Some(resize);
                }
            }
            RESIZING_TYPE | RESIZING_TYPE_SHORT => {
                if parsed_options.resize.is_none() {
                    parsed_options.resize = Some(Resize::default());
                }
                if let Some(ref mut resize) = parsed_options.resize {
                    resize.resizing_type = option.args[0].clone();
                }
            }
            SIZE | SIZE_SHORT | SIZE_SHORT_ALT => {
                let mut store_resize = parsed_options.resize.is_some();
                let mut resize = parsed_options.resize.take().unwrap_or_default();
                let mut width_height_set = false;

                if let Some(arg) = option.args.get(0) {
                    if !arg.is_empty() {
                        resize.width = arg.parse::<u32>().map_err(|e: std::num::ParseIntError| {
                            error!("Invalid width for size: {}", e);
                            e.to_string()
                        })?;
                        store_resize = true;
                        width_height_set = true;
                    }
                }
                if let Some(arg) = option.args.get(1) {
                    if !arg.is_empty() {
                        resize.height = arg.parse::<u32>().map_err(|e: std::num::ParseIntError| {
                            error!("Invalid height for size: {}", e);
                            e.to_string()
                        })?;
                        store_resize = true;
                        width_height_set = true;
                    }
                }

                if let Some(arg) = option.args.get(2) {
                    if !arg.is_empty() {
                        parsed_options.enlarge = super::utils::parse_boolean(arg);
                    }
                }
                if let Some(arg) = option.args.get(3) {
                    if !arg.is_empty() {
                        parsed_options.extend = super::utils::parse_boolean(arg);
                    }
                }

                if store_resize && (width_height_set || resize.resizing_type.is_empty()) {
                    resize.resizing_type = "fit".to_string();
                }

                if store_resize {
                    parsed_options.resize = Some(resize);
                }
            }
            WIDTH | WIDTH_SHORT => {
                let width_arg = option.args.get(0).map(|s| s.as_str()).unwrap_or("0");
                let width = if width_arg.is_empty() {
                    0
                } else {
                    width_arg.parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid width: {}", e);
                        e.to_string()
                    })?
                };
                parsed_options.width = Some(width);
            }
            HEIGHT | HEIGHT_SHORT => {
                let height_arg = option.args.get(0).map(|s| s.as_str()).unwrap_or("0");
                let height = if height_arg.is_empty() {
                    0
                } else {
                    height_arg.parse::<u32>().map_err(|e: std::num::ParseIntError| {
                        error!("Invalid height: {}", e);
                        e.to_string()
                    })?
                };
                parsed_options.height = Some(height);
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
                parsed_options.enlarge = super::utils::parse_boolean(&option.args[0]);
            }
            EXTEND | EXTEND_SHORT => {
                if option.args.is_empty() {
                    error!("Extend option requires one argument");
                    return Err("extend option requires one argument".to_string());
                }
                parsed_options.extend = super::utils::parse_boolean(&option.args[0]);
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
                parsed_options.auto_rotate = super::utils::parse_boolean(&option.args[0]);
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
                parsed_options.background = Some(super::utils::parse_hex_color(&option.args[0]).map_err(|e| {
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
            MIN_WIDTH | MIN_WIDTH_SHORT => {
                if option.args.is_empty() {
                    error!("Min_width option requires one argument");
                    return Err("min_width option requires one argument".to_string());
                }
                parsed_options.min_width = Some(option.args[0].parse::<u32>().map_err(|e| {
                    error!("Invalid min_width: {}", e);
                    e.to_string()
                })?);
            }
            MIN_HEIGHT | MIN_HEIGHT_SHORT => {
                if option.args.is_empty() {
                    error!("Min_height option requires one argument");
                    return Err("min_height option requires one argument".to_string());
                }
                parsed_options.min_height = Some(option.args[0].parse::<u32>().map_err(|e| {
                    error!("Invalid min_height: {}", e);
                    e.to_string()
                })?);
            }
            ZOOM | ZOOM_SHORT => {
                if option.args.is_empty() {
                    error!("Zoom option requires one argument");
                    return Err("zoom option requires one argument".to_string());
                }
                parsed_options.zoom = Some(option.args[0].parse::<f32>().map_err(|e| {
                    error!("Invalid zoom: {}", e);
                    e.to_string()
                })?);
            }
            SHARPEN | SHARPEN_SHORT => {
                if option.args.is_empty() {
                    error!("Sharpen option requires one argument");
                    return Err("sharpen option requires one argument".to_string());
                }
                parsed_options.sharpen = Some(option.args[0].parse::<f32>().map_err(|e| {
                    error!("Invalid sharpen: {}", e);
                    e.to_string()
                })?);
            }
            PIXELATE | PIXELATE_SHORT => {
                if option.args.is_empty() {
                    error!("Pixelate option requires one argument");
                    return Err("pixelate option requires one argument".to_string());
                }
                parsed_options.pixelate = Some(option.args[0].parse::<u32>().map_err(|e| {
                    error!("Invalid pixelate: {}", e);
                    e.to_string()
                })?);
            }
            WATERMARK | WATERMARK_SHORT => {
                if option.args.len() < 2 {
                    error!("Watermark option requires two arguments: opacity, position");
                    return Err("watermark option requires two arguments: opacity, position".to_string());
                }
                parsed_options.watermark = Some(Watermark {
                    opacity: option.args[0].parse::<f32>().map_err(|e| {
                        error!("Invalid opacity for watermark: {}", e);
                        e.to_string()
                    })?,
                    position: option.args[1].clone(),
                });
            }
            WATERMARK_URL | WATERMARK_URL_SHORT => {
                if option.args.is_empty() {
                    error!("Watermark URL option requires one argument");
                    return Err("watermark_url option requires one argument".to_string());
                }
                let decoded_url = general_purpose::URL_SAFE_NO_PAD.decode(&option.args[0]).map_err(|e| {
                    error!("Invalid base64 for watermark_url: {}", e);
                    e.to_string()
                })?;
                let url = String::from_utf8(decoded_url).map_err(|e| {
                    error!("Invalid UTF-8 for watermark_url: {}", e);
                    e.to_string()
                })?;
                parsed_options.watermark_url = Some(url);
            }
            RESIZING_ALGORITHM | RESIZING_ALGORITHM_SHORT => {
                if option.args.is_empty() {
                    error!("Resizing algorithm option requires one argument");
                    return Err("resizing_algorithm option requires one argument".to_string());
                }
                let algorithm = option.args[0].to_lowercase();
                if !matches!(
                    algorithm.as_str(),
                    "nearest" | "linear" | "cubic" | "lanczos2" | "lanczos3"
                ) {
                    error!(
                        "Invalid resizing algorithm: {}. Must be one of: nearest, linear, cubic, lanczos2, lanczos3",
                        algorithm
                    );
                    return Err(format!(
                        "Invalid resizing algorithm: {}. Must be one of: nearest, linear, cubic, lanczos2, lanczos3",
                        algorithm
                    ));
                }
                parsed_options.resizing_algorithm = Some(algorithm);
            }
            _ => {
                debug!("Unknown option: {}", option.name);
            }
        }
    }

    // Default resize type is `fit`
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
