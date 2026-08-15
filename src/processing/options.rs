//! Image processing module for imgforge.
//! This module contains functions and structs for parsing image processing options
//! and applying various transformations to images.

/// Represents a single image processing option from the URL path.
#[derive(Debug, Clone)]
pub struct ProcessingOption {
    /// The name of the processing option (e.g., "resize", "quality").
    pub name: String,
    /// Arguments for the processing option.
    pub args: Vec<String>,
}
use crate::limits::{MaxResultDimension, MaxSourceFileSize, MaxSourceResolution};
use base64::engine::general_purpose;
use base64::Engine as _;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;
use tracing::debug;

/// Errors produced while parsing image processing options.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OptionParseError {
    #[error("invalid {option} value {value:?}")]
    Integer {
        option: String,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("invalid {option} value {value:?}")]
    Float {
        option: String,
        value: String,
        #[source]
        source: std::num::ParseFloatError,
    },
    #[error("invalid Base64 for {option}")]
    Base64 {
        option: String,
        #[source]
        source: base64::DecodeError,
    },
    #[error("invalid UTF-8 for {option}")]
    Utf8 {
        option: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("invalid {option}: {source}")]
    SecurityLimit {
        option: String,
        #[source]
        source: crate::limits::SecurityLimitError,
    },
    #[error("invalid background color")]
    Color(#[source] super::utils::ColorParseError),
    #[error("{0}")]
    InvalidValue(String),
}

impl OptionParseError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidValue(message.into())
    }
}

fn parse_integer<T>(value: &str, option: &str) -> Result<T, OptionParseError>
where
    T: FromStr<Err = std::num::ParseIntError>,
{
    value.parse().map_err(|source| OptionParseError::Integer {
        option: option.to_string(),
        value: value.to_string(),
        source,
    })
}

fn parse_float(value: &str, option: &str) -> Result<f32, OptionParseError> {
    value.parse().map_err(|source| OptionParseError::Float {
        option: option.to_string(),
        value: value.to_string(),
        source,
    })
}

fn decode_base64(value: &str, option: &str) -> Result<Vec<u8>, OptionParseError> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|source| OptionParseError::Base64 {
            option: option.to_string(),
            source,
        })
}

fn decode_utf8(value: Vec<u8>, option: &str) -> Result<String, OptionParseError> {
    String::from_utf8(value).map_err(|source| OptionParseError::Utf8 {
        option: option.to_string(),
        source,
    })
}

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
const SIZE_SHORT: &str = "s";
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
/// Option name for format-specific quality.
const FORMAT_QUALITY: &str = "format_quality";
/// Shorthand for format_quality.
const FORMAT_QUALITY_SHORT: &str = "fq";
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
/// Option name for flip.
const FLIP: &str = "flip";
/// Shorthand for flip.
const FLIP_SHORT: &str = "fl";
/// Option name for raw.
const RAW: &str = "raw";
/// Option name for blur.
const BLUR: &str = "blur";
/// Shorthand for blur.
const BLUR_SHORT: &str = "bl";
/// Option name for crop.
const CROP: &str = "crop";
/// Shorthand for crop.
const CROP_SHORT: &str = "c";
/// Option name for format.
const FORMAT: &str = "format";
/// Shorthand for format.
const FORMAT_SHORT: &str = "f";
/// Alternate shorthand for format.
const FORMAT_EXT: &str = "ext";
/// Option name for max_src_resolution.
const MAX_SRC_RESOLUTION: &str = "max_src_resolution";
/// Shorthand for max_src_resolution.
const MAX_SRC_RESOLUTION_SHORT: &str = "msr";
/// Option name for max_result_dimension.
const MAX_RESULT_DIMENSION: &str = "max_result_dimension";
/// Shorthand for max_result_dimension.
const MAX_RESULT_DIMENSION_SHORT: &str = "mrd";
/// Option name for max_src_file_size.
const MAX_SRC_FILE_SIZE: &str = "max_src_file_size";
/// Shorthand for max_src_file_size.
const MAX_SRC_FILE_SIZE_SHORT: &str = "msfs";
/// Option name for cache buster.
const CACHEBUSTER: &str = "cachebuster";
/// Shorthand for cachebuster.
const CACHEBUSTER_SHORT: &str = "cb";
/// Option name for dpr.
const DPR: &str = "dpr";
/// Option name for min-width.
const MIN_WIDTH: &str = "min-width";
/// Shorthand for min_width.
const MIN_WIDTH_SHORT: &str = "mw";
/// Option name for min-height.
const MIN_HEIGHT: &str = "min-height";
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
const PIXELATE_SHORT: &str = "pix";
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
/// Option name for background_alpha.
const BACKGROUND_ALPHA: &str = "background_alpha";
/// Shorthand for background_alpha.
const BACKGROUND_ALPHA_SHORT: &str = "bga";
/// Option name for adjust.
const ADJUST: &str = "adjust";
/// Shorthand for adjust.
const ADJUST_SHORT: &str = "a";
/// Option name for brightness.
const BRIGHTNESS: &str = "brightness";
/// Shorthand for brightness.
const BRIGHTNESS_SHORT: &str = "br";
/// Option name for contrast.
const CONTRAST: &str = "contrast";
/// Shorthand for contrast.
const CONTRAST_SHORT: &str = "co";
/// Option name for saturation.
const SATURATION: &str = "saturation";
/// Shorthand for saturation.
const SATURATION_SHORT: &str = "sa";
/// Option name for max_bytes.
const MAX_BYTES: &str = "max_bytes";
/// Shorthand for max_bytes.
const MAX_BYTES_SHORT: &str = "mb";
/// Option name for strip_metadata.
const STRIP_METADATA: &str = "strip_metadata";
/// Shorthand for strip_metadata.
const STRIP_METADATA_SHORT: &str = "sm";
/// Option name for strip_color_profile.
const STRIP_COLOR_PROFILE: &str = "strip_color_profile";
/// Shorthand for strip_color_profile.
const STRIP_COLOR_PROFILE_SHORT: &str = "scp";
/// Option name for JPEG options.
const JPEG_OPTIONS: &str = "jpeg_options";
/// Shorthand for JPEG options.
const JPEG_OPTIONS_SHORT: &str = "jpgo";
/// Option name for PNG options.
const PNG_OPTIONS: &str = "png_options";
/// Shorthand for PNG options.
const PNG_OPTIONS_SHORT: &str = "pngo";
/// Option name for WebP options.
const WEBP_OPTIONS: &str = "webp_options";
/// Shorthand for WebP options.
const WEBP_OPTIONS_SHORT: &str = "webpo";
/// Option name for AVIF options.
const AVIF_OPTIONS: &str = "avif_options";
/// Shorthand for AVIF options.
const AVIF_OPTIONS_SHORT: &str = "avifo";
/// Option name for page.
const PAGE: &str = "page";
/// Shorthand for page.
const PAGE_SHORT: &str = "pg";
/// Option name for pages.
const PAGES: &str = "pages";
/// Shorthand for pages.
const PAGES_SHORT: &str = "pgs";
/// Option name for disable_animation.
const DISABLE_ANIMATION: &str = "disable_animation";
/// Shorthand for disable_animation.
const DISABLE_ANIMATION_SHORT: &str = "da";
/// Option name for skip_processing.
const SKIP_PROCESSING: &str = "skip_processing";
/// Shorthand for skip_processing.
const SKIP_PROCESSING_SHORT: &str = "skp";
/// Option name for expires.
const EXPIRES: &str = "expires";
/// Shorthand for expires.
const EXPIRES_SHORT: &str = "exp";
/// Option name for filename.
const FILENAME: &str = "filename";
/// Shorthand for filename.
const FILENAME_SHORT: &str = "fn";
/// Option name for return_attachment.
const RETURN_ATTACHMENT: &str = "return_attachment";
/// Shorthand for return_attachment.
const RETURN_ATTACHMENT_SHORT: &str = "att";

const VALID_ROTATIONS: [u16; 4] = [0, 90, 180, 270];
const VALID_RESIZING_TYPES: [&str; 4] = ["fill", "fit", "force", "auto"];

fn is_valid_rotation(rotation: u16) -> bool {
    VALID_ROTATIONS.contains(&rotation)
}

fn is_valid_resizing_type(resizing_type: &str) -> bool {
    VALID_RESIZING_TYPES.contains(&resizing_type)
}

fn parse_positive_f32(value: &str, option_name: &str) -> Result<f32, OptionParseError> {
    let parsed = parse_float(value, option_name)?;

    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(OptionParseError::invalid(format!(
            "{} must be a finite positive number",
            option_name
        )));
    }

    Ok(parsed)
}

fn parse_unit_f32(value: &str, option_name: &str) -> Result<f32, OptionParseError> {
    let parsed = parse_float(value, option_name)?;

    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        return Err(OptionParseError::invalid(format!(
            "{} must be a finite number between 0 and 1",
            option_name
        )));
    }

    Ok(parsed)
}

fn parse_quality(value: &str, option_name: &str) -> Result<u8, OptionParseError> {
    Ok(parse_integer::<u8>(value, option_name)?.clamp(1, 100))
}

fn parse_optional_bool(args: &[String], index: usize) -> Option<bool> {
    args.get(index)
        .filter(|arg| !arg.is_empty())
        .map(|arg| super::utils::parse_boolean(arg))
}

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

/// Controls how an image is aligned when cropping or extending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravity {
    Center,
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Gravity {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ce" => Some(Self::Center),
            "no" => Some(Self::North),
            "so" => Some(Self::South),
            "ea" => Some(Self::East),
            "we" => Some(Self::West),
            "noea" => Some(Self::NorthEast),
            "nowe" => Some(Self::NorthWest),
            "soea" => Some(Self::SouthEast),
            "sowe" => Some(Self::SouthWest),
            _ => None,
        }
    }
}

/// Represents the parameters for a flip operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct Flip {
    pub horizontal: bool,
    pub vertical: bool,
}

/// Represents the parameters for color adjustment.
#[derive(Debug, Clone, Copy)]
pub struct Adjust {
    pub brightness: i16,
    pub contrast: f32,
    pub saturation: f32,
}

impl Default for Adjust {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 1.0,
            saturation: 1.0,
        }
    }
}

/// Encoder-specific output controls.
#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    pub format_quality: HashMap<String, u8>,
    pub max_bytes: Option<usize>,
    pub strip_metadata: Option<bool>,
    pub strip_color_profile: Option<bool>,
    pub jpeg: JpegOptions,
    pub png: PngOptions,
    pub webp: WebpOptions,
    pub avif: AvifOptions,
}

/// JPEG encoder controls.
#[derive(Debug, Clone, Default)]
pub struct JpegOptions {
    pub progressive: Option<bool>,
    pub no_subsample: Option<bool>,
    pub trellis_quant: Option<bool>,
    pub overshoot_deringing: Option<bool>,
    pub optimize_scans: Option<bool>,
    pub quant_table: Option<i32>,
}

/// PNG encoder controls.
#[derive(Debug, Clone, Default)]
pub struct PngOptions {
    pub interlaced: Option<bool>,
    pub quantize: Option<bool>,
    pub quantization_colors: Option<u16>,
}

/// WebP encoder controls.
#[derive(Debug, Clone, Default)]
pub struct WebpOptions {
    pub lossless: Option<bool>,
    pub smart_subsample: Option<bool>,
    pub preset: Option<String>,
}

/// AVIF/HEIF encoder controls.
#[derive(Debug, Clone, Default)]
pub struct AvifOptions {
    pub no_subsample: Option<bool>,
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
    /// Optional crop gravity.
    pub gravity: Option<Gravity>,
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
    /// Optional gravity for cropping or extending.
    pub gravity: Option<Gravity>,
    /// Whether to allow enlarging the image beyond its original dimensions.
    pub enlarge: bool,
    /// Whether to extend the image with a background if target dimensions are larger.
    pub extend: bool,
    /// Optional padding values (top, right, bottom, left).
    pub padding: Option<(u32, u32, u32, u32)>,
    /// Optional image rotation (rotation angle).
    pub rotation: Option<u16>,
    /// Optional flip operation.
    pub flip: Option<Flip>,
    /// Whether to automatically rotate the image based on EXIF data.
    pub auto_rotate: bool,
    /// Whether to bypass processing limits (e.g., worker limits).
    pub raw: bool,
    /// Maximum allowed source image resolution in megapixels.
    pub max_src_resolution: Option<MaxSourceResolution>,
    /// Ceiling for either dimension of the processed image.
    pub max_result_dimension: Option<MaxResultDimension>,
    /// Maximum allowed source image file size in bytes.
    pub max_src_file_size: Option<MaxSourceFileSize>,
    /// Value to bypass cache (e.g., timestamp).
    pub cache_buster: Option<String>,
    /// Optional unix timestamp after which the request expires.
    pub expires: Option<u64>,
    /// Optional response filename for Content-Disposition.
    pub filename: Option<String>,
    /// Whether to return Content-Disposition as attachment.
    pub return_attachment: bool,
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
    /// Optional alpha value applied to background.
    pub background_alpha: Option<f32>,
    /// Optional color adjustments.
    pub adjust: Option<Adjust>,
    /// Encoder-specific output options.
    pub save: SaveOptions,
    /// Optional page number for multi-page sources.
    pub page: Option<u32>,
    /// Optional page count for multi-page sources.
    pub pages: Option<u32>,
    /// Whether to disable animation handling.
    pub disable_animation: bool,
    /// Source formats that may bypass processing when output format matches.
    pub skip_processing: Vec<String>,
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
            flip: None,
            auto_rotate: true,
            raw: false,
            max_src_resolution: None,
            max_result_dimension: None,
            max_src_file_size: None,
            cache_buster: None,
            expires: None,
            filename: None,
            return_attachment: false,
            dpr: Some(1.0),
            min_width: None,
            min_height: None,
            zoom: None,
            sharpen: None,
            pixelate: None,
            watermark: None,
            watermark_url: None,
            resizing_algorithm: Some("lanczos3".to_string()),
            background_alpha: None,
            adjust: None,
            save: SaveOptions::default(),
            page: None,
            pages: None,
            disable_animation: false,
            skip_processing: Vec::new(),
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
/// A `Result` containing the `ParsedOptions` on success, or a typed parsing error.
pub fn parse_all_options(options: Vec<ProcessingOption>) -> Result<ParsedOptions, OptionParseError> {
    let mut parsed_options = ParsedOptions::default();

    for option in options {
        debug!("Parsing option: {} with args: {:?}", option.name, option.args);
        match option.name.as_str() {
            RESIZE | RESIZE_SHORT => {
                let mut store_resize = parsed_options.resize.is_some();
                let mut resize = parsed_options.resize.take().unwrap_or_default();

                if let Some(arg) = option.args.first() {
                    if !arg.is_empty() {
                        resize.resizing_type = arg.clone();
                        store_resize = true;
                    }
                }
                if let Some(arg) = option.args.get(1) {
                    if !arg.is_empty() {
                        resize.width = parse_integer(arg, "resize width")?;
                        store_resize = true;
                    }
                }
                if let Some(arg) = option.args.get(2) {
                    if !arg.is_empty() {
                        resize.height = parse_integer(arg, "resize height")?;
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
                let resizing_type = option
                    .args
                    .first()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| OptionParseError::invalid("resizing_type option requires one argument"))?;
                if !is_valid_resizing_type(resizing_type) {
                    return Err(OptionParseError::invalid(
                        "resizing_type must be one of: fill, fit, force, auto",
                    ));
                }
                parsed_options
                    .resize
                    .get_or_insert_with(Resize::default)
                    .resizing_type
                    .clone_from(resizing_type);
            }
            SIZE | SIZE_SHORT => {
                let mut store_resize = parsed_options.resize.is_some();
                let mut resize = parsed_options.resize.take().unwrap_or_default();
                let mut width_height_set = false;

                if let Some(arg) = option.args.first() {
                    if !arg.is_empty() {
                        resize.width = parse_integer(arg, "size width")?;
                        store_resize = true;
                        width_height_set = true;
                    }
                }
                if let Some(arg) = option.args.get(1) {
                    if !arg.is_empty() {
                        resize.height = parse_integer(arg, "size height")?;
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
                let width_arg = option.args.first().map(|s| s.as_str()).unwrap_or("0");
                let width = if width_arg.is_empty() {
                    0
                } else {
                    parse_integer(width_arg, "width")?
                };
                parsed_options.width = Some(width);
            }
            HEIGHT | HEIGHT_SHORT => {
                let height_arg = option.args.first().map(|s| s.as_str()).unwrap_or("0");
                let height = if height_arg.is_empty() {
                    0
                } else {
                    parse_integer(height_arg, "height")?
                };
                parsed_options.height = Some(height);
            }
            GRAVITY | GRAVITY_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("gravity option requires one argument"));
                }
                let gravity = option.args[0].as_str();
                let gravity = Gravity::parse(gravity).ok_or_else(|| {
                    OptionParseError::invalid("gravity must be one of: ce, no, so, ea, we, noea, nowe, soea, sowe")
                })?;
                parsed_options.gravity = Some(gravity);
            }
            ENLARGE | ENLARGE_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("enlarge option requires one argument"));
                }
                parsed_options.enlarge = super::utils::parse_boolean(&option.args[0]);
            }
            EXTEND | EXTEND_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("extend option requires one argument"));
                }
                parsed_options.extend = super::utils::parse_boolean(&option.args[0]);
                if let Some(gravity) = option.args.get(1).filter(|arg| !arg.is_empty()) {
                    parsed_options.gravity = Some(Gravity::parse(gravity).ok_or_else(|| {
                        OptionParseError::invalid(
                            "extend gravity must be one of: ce, no, so, ea, we, noea, nowe, soea, sowe",
                        )
                    })?);
                }
            }
            PADDING | PADDING_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid(
                        "padding option requires at least one argument",
                    ));
                }
                let values: Vec<u32> = option
                    .args
                    .iter()
                    .map(|value| parse_integer(value, "padding"))
                    .collect::<Result<Vec<_>, _>>()?;
                parsed_options.padding = Some(match values.len() {
                    1 => (values[0], values[0], values[0], values[0]),
                    2 => (values[0], values[1], values[0], values[1]),
                    4 => (values[0], values[1], values[2], values[3]),
                    _ => {
                        return Err(OptionParseError::invalid("padding must have 1, 2, or 4 arguments"));
                    }
                });
            }
            ROTATE | ROTATE_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("rotation option requires one argument"));
                }
                let rotation = parse_integer(&option.args[0], "rotation")?;
                if !is_valid_rotation(rotation) {
                    return Err(OptionParseError::invalid("rotation must be one of: 0, 90, 180, 270"));
                }
                parsed_options.rotation = Some(rotation);
            }
            FLIP | FLIP_SHORT => {
                parsed_options.flip = Some(Flip {
                    horizontal: parse_optional_bool(&option.args, 0).unwrap_or(false),
                    vertical: parse_optional_bool(&option.args, 1).unwrap_or(false),
                });
            }
            AUTO_ROTATE | AUTO_ROTATE_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("auto_rotate option requires one argument"));
                }
                parsed_options.auto_rotate = super::utils::parse_boolean(&option.args[0]);
            }
            RAW => {
                parsed_options.raw = option
                    .args
                    .first()
                    .filter(|arg| !arg.is_empty())
                    .map(|arg| super::utils::parse_boolean(arg))
                    .unwrap_or(true);
            }
            BLUR | BLUR_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("blur option requires one argument: sigma"));
                }
                parsed_options.blur = Some(parse_positive_f32(&option.args[0], "blur")?);
            }
            CROP | CROP_SHORT => {
                if option.args.len() < 2 {
                    return Err(OptionParseError::invalid(
                        "crop option requires at least two arguments: width, height",
                    ));
                }
                let gravity = option
                    .args
                    .get(2)
                    .filter(|arg| !arg.is_empty())
                    .map(|arg| {
                        Gravity::parse(arg).ok_or_else(|| {
                            OptionParseError::invalid(
                                "crop gravity must be one of: ce, no, so, ea, we, noea, nowe, soea, sowe",
                            )
                        })
                    })
                    .transpose()?;
                parsed_options.crop = Some(Crop {
                    x: 0,
                    y: 0,
                    width: parse_integer(&option.args[0], "crop width")?,
                    height: parse_integer(&option.args[1], "crop height")?,
                    gravity,
                });
            }
            FORMAT | FORMAT_SHORT | FORMAT_EXT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("format option requires one argument"));
                }
                parsed_options.format = Some(option.args[0].clone());
            }
            QUALITY | QUALITY_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("quality option requires one argument"));
                }
                parsed_options.quality = Some(parse_quality(&option.args[0], "quality")?);
            }
            FORMAT_QUALITY | FORMAT_QUALITY_SHORT => {
                if option.args.len() < 2 || option.args.len() % 2 != 0 {
                    return Err(OptionParseError::invalid(
                        "format_quality option requires format/quality pairs",
                    ));
                }

                for pair in option.args.chunks_exact(2) {
                    parsed_options
                        .save
                        .format_quality
                        .insert(pair[0].to_lowercase(), parse_quality(&pair[1], "format_quality")?);
                }
            }
            BACKGROUND | BACKGROUND_SHORT => {
                if option.args.is_empty() {
                    parsed_options.background = None;
                    continue;
                }
                let mut background = if option.args.len() >= 3 {
                    [
                        parse_integer(&option.args[0], "background red channel")?,
                        parse_integer(&option.args[1], "background green channel")?,
                        parse_integer(&option.args[2], "background blue channel")?,
                        255,
                    ]
                } else {
                    super::utils::parse_hex_color(&option.args[0]).map_err(OptionParseError::Color)?
                };
                if let Some(alpha) = parsed_options.background_alpha {
                    background[3] = (alpha * 255.0).round() as u8;
                }
                parsed_options.background = Some(background);
            }
            BACKGROUND_ALPHA | BACKGROUND_ALPHA_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid(
                        "background_alpha option requires one argument",
                    ));
                }
                let alpha = parse_unit_f32(&option.args[0], "background_alpha")?;
                parsed_options.background_alpha = Some(alpha);
                if let Some(ref mut background) = parsed_options.background {
                    background[3] = (alpha * 255.0).round() as u8;
                }
            }
            MAX_RESULT_DIMENSION | MAX_RESULT_DIMENSION_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid(
                        "max_result_dimension option requires one argument",
                    ));
                }
                parsed_options.max_result_dimension =
                    Some(option.args[0].parse::<MaxResultDimension>().map_err(|source| {
                        OptionParseError::SecurityLimit {
                            option: "max_result_dimension".to_string(),
                            source,
                        }
                    })?);
            }
            MAX_SRC_RESOLUTION | MAX_SRC_RESOLUTION_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid(
                        "max_src_resolution option requires one argument",
                    ));
                }
                parsed_options.max_src_resolution =
                    Some(option.args[0].parse::<MaxSourceResolution>().map_err(|source| {
                        OptionParseError::SecurityLimit {
                            option: "max_src_resolution".to_string(),
                            source,
                        }
                    })?);
            }
            MAX_SRC_FILE_SIZE | MAX_SRC_FILE_SIZE_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid(
                        "max_src_file_size option requires one argument",
                    ));
                }
                parsed_options.max_src_file_size =
                    Some(option.args[0].parse::<MaxSourceFileSize>().map_err(|source| {
                        OptionParseError::SecurityLimit {
                            option: "max_src_file_size".to_string(),
                            source,
                        }
                    })?);
            }
            CACHEBUSTER | CACHEBUSTER_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("cachebuster option requires one argument"));
                }
                parsed_options.cache_buster = Some(option.args[0].clone());
            }
            DPR => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("dpr option requires one argument"));
                }
                let dpr = parse_float(&option.args[0], "dpr")?;
                if !(1.0..=5.0).contains(&dpr) {
                    return Err(OptionParseError::invalid("dpr value must be between 1.0 and 5.0"));
                }
                parsed_options.dpr = Some(dpr);
            }
            MIN_WIDTH | MIN_WIDTH_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("min-width option requires one argument"));
                }
                parsed_options.min_width = Some(parse_integer(&option.args[0], "min-width")?);
            }
            MIN_HEIGHT | MIN_HEIGHT_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("min-height option requires one argument"));
                }
                parsed_options.min_height = Some(parse_integer(&option.args[0], "min-height")?);
            }
            ZOOM | ZOOM_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("zoom option requires one argument"));
                }
                parsed_options.zoom = Some(parse_positive_f32(&option.args[0], "zoom")?);
            }
            SHARPEN | SHARPEN_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("sharpen option requires one argument"));
                }
                parsed_options.sharpen = Some(parse_positive_f32(&option.args[0], "sharpen")?);
            }
            PIXELATE | PIXELATE_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("pixelate option requires one argument"));
                }
                parsed_options.pixelate = Some(parse_integer(&option.args[0], "pixelate")?);
            }
            ADJUST | ADJUST_SHORT => {
                let mut adjust = parsed_options.adjust.unwrap_or_default();
                if let Some(arg) = option.args.first().filter(|arg| !arg.is_empty()) {
                    adjust.brightness = parse_brightness(arg)?;
                }
                if let Some(arg) = option.args.get(1).filter(|arg| !arg.is_empty()) {
                    adjust.contrast = parse_positive_f32(arg, "contrast")?;
                }
                if let Some(arg) = option.args.get(2).filter(|arg| !arg.is_empty()) {
                    adjust.saturation = parse_positive_f32(arg, "saturation")?;
                }
                parsed_options.adjust = Some(adjust);
            }
            BRIGHTNESS | BRIGHTNESS_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("brightness option requires one argument"));
                }
                let mut adjust = parsed_options.adjust.unwrap_or_default();
                adjust.brightness = parse_brightness(&option.args[0])?;
                parsed_options.adjust = Some(adjust);
            }
            CONTRAST | CONTRAST_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("contrast option requires one argument"));
                }
                let mut adjust = parsed_options.adjust.unwrap_or_default();
                adjust.contrast = parse_positive_f32(&option.args[0], "contrast")?;
                parsed_options.adjust = Some(adjust);
            }
            SATURATION | SATURATION_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("saturation option requires one argument"));
                }
                let mut adjust = parsed_options.adjust.unwrap_or_default();
                adjust.saturation = parse_positive_f32(&option.args[0], "saturation")?;
                parsed_options.adjust = Some(adjust);
            }
            WATERMARK | WATERMARK_SHORT => {
                if option.args.len() < 2 {
                    return Err(OptionParseError::invalid(
                        "watermark option requires two arguments: opacity, position",
                    ));
                }
                parsed_options.watermark = Some(Watermark {
                    opacity: parse_float(&option.args[0], "watermark opacity")?,
                    position: option.args[1].clone(),
                });
            }
            WATERMARK_URL | WATERMARK_URL_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("watermark_url option requires one argument"));
                }
                let decoded_url = decode_base64(&option.args[0], "watermark_url")?;
                let url = decode_utf8(decoded_url, "watermark_url")?;
                parsed_options.watermark_url = Some(url);
            }
            RESIZING_ALGORITHM | RESIZING_ALGORITHM_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid(
                        "resizing_algorithm option requires one argument",
                    ));
                }
                let algorithm = option.args[0].to_lowercase();
                if !matches!(
                    algorithm.as_str(),
                    "nearest" | "linear" | "cubic" | "lanczos2" | "lanczos3"
                ) {
                    return Err(OptionParseError::invalid(format!(
                        "Invalid resizing algorithm: {}. Must be one of: nearest, linear, cubic, lanczos2, lanczos3",
                        algorithm
                    )));
                }
                parsed_options.resizing_algorithm = Some(algorithm);
            }
            MAX_BYTES | MAX_BYTES_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("max_bytes option requires one argument"));
                }
                parsed_options.save.max_bytes = Some(parse_integer(&option.args[0], "max_bytes")?);
            }
            STRIP_METADATA | STRIP_METADATA_SHORT => {
                parsed_options.save.strip_metadata = Some(
                    option
                        .args
                        .first()
                        .map(|arg| super::utils::parse_boolean(arg))
                        .unwrap_or(true),
                );
            }
            STRIP_COLOR_PROFILE | STRIP_COLOR_PROFILE_SHORT => {
                parsed_options.save.strip_color_profile = Some(
                    option
                        .args
                        .first()
                        .map(|arg| super::utils::parse_boolean(arg))
                        .unwrap_or(true),
                );
            }
            JPEG_OPTIONS | JPEG_OPTIONS_SHORT => {
                parsed_options.save.jpeg.progressive = parse_optional_bool(&option.args, 0);
                parsed_options.save.jpeg.no_subsample = parse_optional_bool(&option.args, 1);
                parsed_options.save.jpeg.trellis_quant = parse_optional_bool(&option.args, 2);
                parsed_options.save.jpeg.overshoot_deringing = parse_optional_bool(&option.args, 3);
                parsed_options.save.jpeg.optimize_scans = parse_optional_bool(&option.args, 4);
                if let Some(arg) = option.args.get(5).filter(|arg| !arg.is_empty()) {
                    parsed_options.save.jpeg.quant_table = Some(parse_integer(arg, "jpeg quant_table")?);
                }
            }
            PNG_OPTIONS | PNG_OPTIONS_SHORT => {
                parsed_options.save.png.interlaced = parse_optional_bool(&option.args, 0);
                parsed_options.save.png.quantize = parse_optional_bool(&option.args, 1);
                if let Some(arg) = option.args.get(2).filter(|arg| !arg.is_empty()) {
                    parsed_options.save.png.quantization_colors = Some(parse_integer(arg, "png quantization_colors")?);
                }
            }
            WEBP_OPTIONS | WEBP_OPTIONS_SHORT => {
                parsed_options.save.webp.lossless = parse_optional_bool(&option.args, 0);
                parsed_options.save.webp.smart_subsample = parse_optional_bool(&option.args, 1);
                if let Some(arg) = option.args.get(2).filter(|arg| !arg.is_empty()) {
                    parsed_options.save.webp.preset = Some(arg.to_lowercase());
                }
            }
            AVIF_OPTIONS | AVIF_OPTIONS_SHORT => {
                parsed_options.save.avif.no_subsample = parse_optional_bool(&option.args, 0);
            }
            PAGE | PAGE_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("page option requires one argument"));
                }
                parsed_options.page = Some(parse_integer(&option.args[0], "page")?);
            }
            PAGES | PAGES_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("pages option requires one argument"));
                }
                parsed_options.pages = Some(parse_integer(&option.args[0], "pages")?);
            }
            DISABLE_ANIMATION | DISABLE_ANIMATION_SHORT => {
                parsed_options.disable_animation = option
                    .args
                    .first()
                    .map(|arg| super::utils::parse_boolean(arg))
                    .unwrap_or(true);
            }
            SKIP_PROCESSING | SKIP_PROCESSING_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid(
                        "skip_processing option requires at least one argument",
                    ));
                }
                parsed_options.skip_processing = option.args.iter().map(|arg| arg.to_lowercase()).collect();
            }
            EXPIRES | EXPIRES_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("expires option requires one argument"));
                }
                parsed_options.expires = Some(parse_integer(&option.args[0], "expires timestamp")?);
            }
            FILENAME | FILENAME_SHORT => {
                if option.args.is_empty() {
                    return Err(OptionParseError::invalid("filename option requires one argument"));
                }
                let encoded = option
                    .args
                    .get(1)
                    .map(|arg| super::utils::parse_boolean(arg))
                    .unwrap_or(false);
                parsed_options.filename = Some(if encoded {
                    let decoded = decode_base64(&option.args[0], "filename")?;
                    decode_utf8(decoded, "filename")?
                } else {
                    option.args[0].clone()
                });
            }
            RETURN_ATTACHMENT | RETURN_ATTACHMENT_SHORT => {
                parsed_options.return_attachment = option
                    .args
                    .first()
                    .map(|arg| super::utils::parse_boolean(arg))
                    .unwrap_or(true);
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

fn parse_brightness(value: &str) -> Result<i16, OptionParseError> {
    let parsed = parse_integer::<i16>(value, "brightness")?;
    if !(-255..=255).contains(&parsed) {
        return Err(OptionParseError::invalid("brightness must be between -255 and 255"));
    }
    Ok(parsed)
}
