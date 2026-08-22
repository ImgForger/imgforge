//! Parsing of the imgproxy-compatible processing directives carried in the URL
//! path.
//!
//! The directive table lives in [`names`]; each option group owns its own types
//! and argument parsing, and [`parse_all_options`] is the dispatch that binds a
//! directive name to the group that understands it.

mod effects;
mod encoder;
mod error;
mod geometry;
mod names;

pub use effects::{Adjust, Watermark, WatermarkPosition, Zoom};
pub use encoder::{AvifOptions, JpegOptions, PngOptions, SaveOptions, WebpOptions};
pub use error::OptionParseError;
pub use geometry::{Crop, Extend, Flip, Gravity, GravityType, Resize, ResizingType, Trim};

use crate::limits::{
    MaxAnimationFrameResolution, MaxAnimationFrames, MaxResultDimension, MaxSourceFileSize, MaxSourceResolution,
};
use crate::processing::utils::parse_boolean;
use error::{arg, decode_base64, decode_utf8, parse_integer, parse_positive_f32, parse_quality, parse_unit_f32};
use names::*;
use std::str::FromStr;
use tracing::debug;

/// Represents a single image processing option from the URL path.
#[derive(Debug, Clone)]
pub struct ProcessingOption {
    /// The name of the processing option (e.g., "resize", "quality").
    pub name: String,
    /// Arguments for the processing option.
    pub args: Vec<String>,
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
    /// Optional gravity for cropping or filling.
    pub gravity: Option<Gravity>,
    /// Whether to allow enlarging the image beyond its original dimensions.
    pub enlarge: bool,
    /// Padding out to the requested dimensions after resizing.
    pub extend: Extend,
    /// Padding out to the requested aspect ratio after resizing.
    pub extend_aspect_ratio: Extend,
    /// Optional padding values (top, right, bottom, left).
    pub padding: Option<(u32, u32, u32, u32)>,
    /// Optional image rotation (rotation angle).
    pub rotation: Option<u16>,
    /// Optional flip operation.
    pub flip: Option<Flip>,
    /// Whether to automatically rotate the image based on EXIF data.
    pub auto_rotate: bool,
    /// Whether to return the source untouched.
    pub raw: bool,
    /// Maximum allowed source image resolution in megapixels.
    pub max_src_resolution: Option<MaxSourceResolution>,
    /// Ceiling for either dimension of the processed image.
    pub max_result_dimension: Option<MaxResultDimension>,
    /// Ceiling on how many frames of an animated source are decoded.
    pub max_animation_frames: Option<MaxAnimationFrames>,
    /// Ceiling on the pixel count of a single animation frame.
    pub max_animation_frame_resolution: Option<MaxAnimationFrameResolution>,
    /// Border trimming, applied before crop and resize.
    pub trim: Option<Trim>,
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
    /// Zoom factors applied after resizing.
    pub zoom: Option<Zoom>,
    /// Sharpen factor for the image.
    pub sharpen: Option<f32>,
    /// Pixelate factor for the image.
    pub pixelate: Option<u32>,
    /// Watermark placement, when a watermark source is configured.
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
    /// Prefer the source's embedded thumbnail when one is large enough.
    pub enforce_thumbnail: bool,
    /// First page of a multi-page source to read.
    pub page: Option<u32>,
    /// How many pages of a multi-page source to read.
    pub pages: Option<u32>,
    /// Whether to collapse an animated source to its first frame.
    pub disable_animation: bool,
    /// Source formats that may bypass processing when output format matches.
    pub skip_processing: Vec<String>,
}

/// Server-configured starting values for the options a URL may override.
///
/// imgproxy lets an operator set the default for `auto_rotate`,
/// `strip_metadata` and friends, with the URL overriding it. Seeding the parse
/// rather than patching the result afterwards keeps that a single rule: the
/// URL always wins because it is applied second, and no option needs a separate
/// "was this set?" flag alongside its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionDefaults {
    pub auto_rotate: bool,
    pub strip_metadata: bool,
    pub keep_copyright: bool,
    pub strip_color_profile: bool,
    pub preserve_hdr: bool,
    pub enforce_thumbnail: bool,
    pub return_attachment: bool,
    pub quality: Option<u8>,
}

impl Default for OptionDefaults {
    fn default() -> Self {
        Self {
            auto_rotate: true,
            strip_metadata: false,
            keep_copyright: false,
            strip_color_profile: false,
            preserve_hdr: false,
            enforce_thumbnail: false,
            return_attachment: false,
            quality: None,
        }
    }
}

impl Default for ParsedOptions {
    fn default() -> Self {
        Self::with_defaults(OptionDefaults::default())
    }
}

impl ParsedOptions {
    /// Builds the starting point for a parse, seeded from the server's
    /// configured defaults.
    pub fn with_defaults(defaults: OptionDefaults) -> Self {
        Self {
            resize: None,
            blur: None,
            crop: None,
            format: None,
            quality: defaults.quality,
            background: None,
            width: None,
            height: None,
            gravity: None,
            enlarge: false,
            extend: Extend::default(),
            extend_aspect_ratio: Extend::default(),
            padding: None,
            rotation: None,
            flip: None,
            auto_rotate: defaults.auto_rotate,
            raw: false,
            max_src_resolution: None,
            max_result_dimension: None,
            max_animation_frames: None,
            max_animation_frame_resolution: None,
            trim: None,
            max_src_file_size: None,
            cache_buster: None,
            expires: None,
            filename: None,
            return_attachment: defaults.return_attachment,
            // `None` until the URL names one — presence is how "the URL said
            // `dpr:1`" stays distinguishable from "the URL said nothing", which
            // is what lets an explicit `dpr:1` refuse a larger client hint.
            // `dpr_factor()` reads the absence as 1.0.
            dpr: None,
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
            save: SaveOptions {
                strip_metadata: Some(defaults.strip_metadata),
                strip_color_profile: Some(defaults.strip_color_profile),
                keep_copyright: Some(defaults.keep_copyright),
                preserve_hdr: Some(defaults.preserve_hdr),
                ..SaveOptions::default()
            },
            enforce_thumbnail: defaults.enforce_thumbnail,
            page: None,
            pages: None,
            disable_animation: false,
            skip_processing: Vec::new(),
        }
    }

    /// The zoom factors in effect, defaulting to no zoom.
    pub fn zoom_factors(&self) -> Zoom {
        self.zoom.unwrap_or_default()
    }

    /// The device pixel ratio in effect, never below 1.
    pub fn dpr_factor(&self) -> f32 {
        self.dpr.unwrap_or(1.0).max(1.0)
    }

    /// Gravity for a crop: the crop's own if it names one, otherwise the
    /// request's `gravity`, otherwise centre. imgproxy resolves it the same way.
    pub fn crop_gravity(&self) -> Gravity {
        self.crop
            .and_then(|crop| crop.gravity)
            .or(self.gravity)
            .unwrap_or_default()
    }

    /// Gravity for the fill window, which never consults the crop's.
    pub fn fill_gravity(&self) -> Gravity {
        self.gravity.unwrap_or_default()
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
    parse_all_options_with_defaults(options, OptionDefaults::default())
}

/// Parses processing options on top of the server's configured defaults.
pub fn parse_all_options_with_defaults(
    options: Vec<ProcessingOption>,
    defaults: OptionDefaults,
) -> Result<ParsedOptions, OptionParseError> {
    let mut parsed = ParsedOptions::with_defaults(defaults);

    for option in options {
        debug!("Parsing option: {} with args: {:?}", option.name, option.args);
        apply_option(&option, &mut parsed)?;
    }

    // Default resize type is `fit`
    if parsed.resize.is_none() && (parsed.width.is_some() || parsed.height.is_some()) {
        debug!("Applying default 'fit' resize due to width/height options");
        parsed.resize = Some(Resize {
            resizing_type: ResizingType::Fit,
            width: parsed.width.unwrap_or(0),
            height: parsed.height.unwrap_or(0),
        });
    }

    Ok(parsed)
}

fn apply_option(option: &ProcessingOption, parsed: &mut ParsedOptions) -> Result<(), OptionParseError> {
    let args = option.args.as_slice();

    match option.name.as_str() {
        RESIZE | RESIZE_SHORT => apply_resize(args, parsed)?,
        RESIZING_TYPE | RESIZING_TYPE_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("resizing_type option requires one argument"))?;
            parsed.resize.get_or_insert_with(Resize::default).resizing_type = parse_resizing_type(value)?;
        }
        SIZE | SIZE_SHORT => apply_size(args, parsed)?,
        WIDTH | WIDTH_SHORT => {
            parsed.width = Some(match arg(args, 0) {
                Some(value) => parse_integer(value, "width")?,
                None => 0,
            });
        }
        HEIGHT | HEIGHT_SHORT => {
            parsed.height = Some(match arg(args, 0) {
                Some(value) => parse_integer(value, "height")?,
                None => 0,
            });
        }
        GRAVITY | GRAVITY_SHORT => parsed.gravity = Some(Gravity::parse(args, 0, "gravity")?),
        ENLARGE | ENLARGE_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("enlarge option requires one argument"))?;
            parsed.enlarge = parse_boolean(value);
        }
        EXTEND | EXTEND_SHORT => {
            if args.is_empty() {
                return Err(OptionParseError::invalid("extend option requires one argument"));
            }
            parsed.extend = Extend::parse(args, "extend")?;
        }
        EXTEND_ASPECT_RATIO | EXTEND_ASPECT_RATIO_ALT | EXTEND_ASPECT_RATIO_SHORT => {
            if args.is_empty() {
                return Err(OptionParseError::invalid(
                    "extend_aspect_ratio option requires one argument",
                ));
            }
            parsed.extend_aspect_ratio = Extend::parse(args, "extend_aspect_ratio")?;
        }
        PADDING | PADDING_SHORT => parsed.padding = Some(geometry::parse_padding(args)?),
        ROTATE | ROTATE_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("rotation option requires one argument"))?;
            parsed.rotation = Some(geometry::parse_rotation(value)?);
        }
        FLIP | FLIP_SHORT => {
            parsed.flip = Some(Flip {
                horizontal: error::parse_optional_bool(args, 0).unwrap_or(false),
                vertical: error::parse_optional_bool(args, 1).unwrap_or(false),
            });
        }
        AUTO_ROTATE | AUTO_ROTATE_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("auto_rotate option requires one argument"))?;
            parsed.auto_rotate = parse_boolean(value);
        }
        RAW => parsed.raw = arg(args, 0).map(parse_boolean).unwrap_or(true),
        BLUR | BLUR_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("blur option requires one argument: sigma"))?;
            parsed.blur = Some(parse_positive_f32(value, "blur")?);
        }
        CROP | CROP_SHORT => parsed.crop = Some(parse_crop(args)?),
        FORMAT | FORMAT_SHORT | FORMAT_EXT => {
            let value = arg(args, 0).ok_or_else(|| OptionParseError::invalid("format option requires one argument"))?;
            parsed.format = Some(value.to_lowercase());
        }
        QUALITY | QUALITY_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("quality option requires one argument"))?;
            parsed.quality = Some(parse_quality(value, "quality")?);
        }
        FORMAT_QUALITY | FORMAT_QUALITY_SHORT => encoder::parse_format_quality(args, &mut parsed.save)?,
        BACKGROUND | BACKGROUND_SHORT => apply_background(args, parsed)?,
        BACKGROUND_ALPHA | BACKGROUND_ALPHA_SHORT => {
            let value = arg(args, 0)
                .ok_or_else(|| OptionParseError::invalid("background_alpha option requires one argument"))?;
            let alpha = parse_unit_f32(value, "background_alpha")?;
            parsed.background_alpha = Some(alpha);
            if let Some(background) = parsed.background.as_mut() {
                background[3] = (alpha * 255.0).round() as u8;
            }
        }
        TRIM | TRIM_SHORT => parsed.trim = Some(geometry::parse_trim(args)?),
        MAX_RESULT_DIMENSION | MAX_RESULT_DIMENSION_SHORT => {
            parsed.max_result_dimension = Some(parse_limit(args, "max_result_dimension")?);
        }
        MAX_SRC_RESOLUTION | MAX_SRC_RESOLUTION_SHORT => {
            parsed.max_src_resolution = Some(parse_limit(args, "max_src_resolution")?);
        }
        MAX_SRC_FILE_SIZE | MAX_SRC_FILE_SIZE_SHORT => {
            parsed.max_src_file_size = Some(parse_limit(args, "max_src_file_size")?);
        }
        MAX_ANIMATION_FRAMES | MAX_ANIMATION_FRAMES_SHORT => {
            parsed.max_animation_frames = Some(parse_limit(args, "max_animation_frames")?);
        }
        MAX_ANIMATION_FRAME_RESOLUTION | MAX_ANIMATION_FRAME_RESOLUTION_SHORT => {
            parsed.max_animation_frame_resolution = Some(parse_limit(args, "max_animation_frame_resolution")?);
        }
        CACHEBUSTER | CACHEBUSTER_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("cachebuster option requires one argument"))?;
            parsed.cache_buster = Some(value.to_string());
        }
        DPR => {
            let value = arg(args, 0).ok_or_else(|| OptionParseError::invalid("dpr option requires one argument"))?;
            let dpr = error::parse_float(value, "dpr")?;
            if !(1.0..=5.0).contains(&dpr) {
                return Err(OptionParseError::invalid("dpr value must be between 1.0 and 5.0"));
            }
            parsed.dpr = Some(dpr);
        }
        MIN_WIDTH | MIN_WIDTH_ALT | MIN_WIDTH_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("min-width option requires one argument"))?;
            parsed.min_width = Some(parse_integer(value, "min-width")?);
        }
        MIN_HEIGHT | MIN_HEIGHT_ALT | MIN_HEIGHT_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("min-height option requires one argument"))?;
            parsed.min_height = Some(parse_integer(value, "min-height")?);
        }
        ZOOM | ZOOM_SHORT => parsed.zoom = Some(Zoom::parse(args)?),
        SHARPEN | SHARPEN_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("sharpen option requires one argument"))?;
            parsed.sharpen = Some(parse_positive_f32(value, "sharpen")?);
        }
        PIXELATE | PIXELATE_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("pixelate option requires one argument"))?;
            parsed.pixelate = Some(parse_integer(value, "pixelate")?);
        }
        ADJUST | ADJUST_SHORT => {
            let mut adjust = parsed.adjust.unwrap_or_default();
            if let Some(value) = arg(args, 0) {
                adjust.brightness = effects::parse_brightness(value)?;
            }
            if let Some(value) = arg(args, 1) {
                adjust.contrast = parse_positive_f32(value, "contrast")?;
            }
            if let Some(value) = arg(args, 2) {
                adjust.saturation = parse_positive_f32(value, "saturation")?;
            }
            parsed.adjust = Some(adjust);
        }
        BRIGHTNESS | BRIGHTNESS_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("brightness option requires one argument"))?;
            let mut adjust = parsed.adjust.unwrap_or_default();
            adjust.brightness = effects::parse_brightness(value)?;
            parsed.adjust = Some(adjust);
        }
        CONTRAST | CONTRAST_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("contrast option requires one argument"))?;
            let mut adjust = parsed.adjust.unwrap_or_default();
            adjust.contrast = parse_positive_f32(value, "contrast")?;
            parsed.adjust = Some(adjust);
        }
        SATURATION | SATURATION_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("saturation option requires one argument"))?;
            let mut adjust = parsed.adjust.unwrap_or_default();
            adjust.saturation = parse_positive_f32(value, "saturation")?;
            parsed.adjust = Some(adjust);
        }
        WATERMARK | WATERMARK_SHORT => parsed.watermark = Some(Watermark::parse(args)?),
        WATERMARK_URL | WATERMARK_URL_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("watermark_url option requires one argument"))?;
            let decoded = decode_base64(value, "watermark_url")?;
            parsed.watermark_url = Some(decode_utf8(decoded, "watermark_url")?);
        }
        RESIZING_ALGORITHM | RESIZING_ALGORITHM_SHORT => {
            let value = arg(args, 0)
                .ok_or_else(|| OptionParseError::invalid("resizing_algorithm option requires one argument"))?;
            let algorithm = value.to_lowercase();
            if !matches!(
                algorithm.as_str(),
                "nearest" | "linear" | "cubic" | "lanczos2" | "lanczos3"
            ) {
                return Err(OptionParseError::invalid(format!(
                    "Invalid resizing algorithm: {}. Must be one of: nearest, linear, cubic, lanczos2, lanczos3",
                    algorithm
                )));
            }
            parsed.resizing_algorithm = Some(algorithm);
        }
        MAX_BYTES | MAX_BYTES_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("max_bytes option requires one argument"))?;
            parsed.save.max_bytes = Some(parse_integer(value, "max_bytes")?);
        }
        STRIP_METADATA | STRIP_METADATA_SHORT => {
            parsed.save.strip_metadata = Some(arg(args, 0).map(parse_boolean).unwrap_or(true));
        }
        KEEP_COPYRIGHT | KEEP_COPYRIGHT_SHORT => {
            parsed.save.keep_copyright = Some(arg(args, 0).map(parse_boolean).unwrap_or(true));
        }
        STRIP_COLOR_PROFILE | STRIP_COLOR_PROFILE_SHORT => {
            parsed.save.strip_color_profile = Some(arg(args, 0).map(parse_boolean).unwrap_or(true));
        }
        PRESERVE_HDR | PRESERVE_HDR_SHORT => {
            parsed.save.preserve_hdr = Some(arg(args, 0).map(parse_boolean).unwrap_or(true));
        }
        ENFORCE_THUMBNAIL | ENFORCE_THUMBNAIL_SHORT => {
            parsed.enforce_thumbnail = arg(args, 0).map(parse_boolean).unwrap_or(true);
        }
        JPEG_OPTIONS | JPEG_OPTIONS_SHORT => encoder::parse_jpeg_options(args, &mut parsed.save.jpeg)?,
        PNG_OPTIONS | PNG_OPTIONS_SHORT => encoder::parse_png_options(args, &mut parsed.save.png)?,
        WEBP_OPTIONS | WEBP_OPTIONS_SHORT => encoder::parse_webp_options(args, &mut parsed.save.webp),
        AVIF_OPTIONS | AVIF_OPTIONS_SHORT => {
            parsed.save.avif.no_subsample = error::parse_optional_bool(args, 0);
        }
        PAGE | PAGE_SHORT => {
            let value = arg(args, 0).ok_or_else(|| OptionParseError::invalid("page option requires one argument"))?;
            parsed.page = Some(parse_integer(value, "page")?);
        }
        PAGES | PAGES_SHORT => {
            let value = arg(args, 0).ok_or_else(|| OptionParseError::invalid("pages option requires one argument"))?;
            let pages: u32 = parse_integer(value, "pages")?;
            if pages == 0 {
                return Err(OptionParseError::invalid("pages must be greater than zero"));
            }
            parsed.pages = Some(pages);
        }
        DISABLE_ANIMATION | DISABLE_ANIMATION_SHORT => {
            parsed.disable_animation = arg(args, 0).map(parse_boolean).unwrap_or(true);
        }
        SKIP_PROCESSING | SKIP_PROCESSING_SHORT => {
            if args.is_empty() {
                return Err(OptionParseError::invalid(
                    "skip_processing option requires at least one argument",
                ));
            }
            parsed.skip_processing = args.iter().map(|value| value.to_lowercase()).collect();
        }
        EXPIRES | EXPIRES_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("expires option requires one argument"))?;
            parsed.expires = Some(parse_integer(value, "expires timestamp")?);
        }
        FILENAME | FILENAME_SHORT => {
            let value =
                arg(args, 0).ok_or_else(|| OptionParseError::invalid("filename option requires one argument"))?;
            let encoded = arg(args, 1).map(parse_boolean).unwrap_or(false);
            parsed.filename = Some(if encoded {
                decode_utf8(decode_base64(value, "filename")?, "filename")?
            } else {
                value.to_string()
            });
        }
        RETURN_ATTACHMENT | RETURN_ATTACHMENT_SHORT => {
            parsed.return_attachment = arg(args, 0).map(parse_boolean).unwrap_or(true);
        }
        unknown => debug!("Unknown option: {}", unknown),
    }

    Ok(())
}

fn parse_resizing_type(value: &str) -> Result<ResizingType, OptionParseError> {
    value
        .parse()
        .map_err(|_| OptionParseError::invalid("resizing_type must be one of: fit, fill, fill-down, force, auto"))
}

/// A limit option always takes exactly one argument and validates it through
/// the same type the configuration uses, so a URL override cannot be looser
/// than what the server would have accepted.
fn parse_limit<T>(args: &[String], option: &'static str) -> Result<T, OptionParseError>
where
    T: FromStr<Err = crate::limits::SecurityLimitError>,
{
    let value =
        arg(args, 0).ok_or_else(|| OptionParseError::invalid(format!("{option} option requires one argument")))?;
    value.parse().map_err(|source| OptionParseError::SecurityLimit {
        option: option.to_string(),
        source,
    })
}

fn apply_resize(args: &[String], parsed: &mut ParsedOptions) -> Result<(), OptionParseError> {
    let mut store_resize = parsed.resize.is_some();
    let mut resize = parsed.resize.take().unwrap_or_default();

    if let Some(value) = arg(args, 0) {
        resize.resizing_type = parse_resizing_type(value)?;
        store_resize = true;
    }
    if let Some(value) = arg(args, 1) {
        resize.width = parse_integer(value, "resize width")?;
        store_resize = true;
    }
    if let Some(value) = arg(args, 2) {
        resize.height = parse_integer(value, "resize height")?;
        store_resize = true;
    }
    if let Some(value) = arg(args, 3) {
        parsed.enlarge = parse_boolean(value);
    }
    if arg(args, 4).is_some() {
        parsed.extend = Extend::parse(&args[4..], "extend")?;
    }

    if store_resize {
        parsed.resize = Some(resize);
    }

    Ok(())
}

fn apply_size(args: &[String], parsed: &mut ParsedOptions) -> Result<(), OptionParseError> {
    let mut store_resize = parsed.resize.is_some();
    let mut resize = parsed.resize.take().unwrap_or_default();

    if let Some(value) = arg(args, 0) {
        resize.width = parse_integer(value, "size width")?;
        store_resize = true;
    }
    if let Some(value) = arg(args, 1) {
        resize.height = parse_integer(value, "size height")?;
        store_resize = true;
    }
    if let Some(value) = arg(args, 2) {
        parsed.enlarge = parse_boolean(value);
    }
    if arg(args, 3).is_some() {
        parsed.extend = Extend::parse(&args[3..], "extend")?;
    }

    if store_resize {
        parsed.resize = Some(resize);
    }

    Ok(())
}

fn apply_background(args: &[String], parsed: &mut ParsedOptions) -> Result<(), OptionParseError> {
    if args.is_empty() || args.iter().all(|value| value.is_empty()) {
        parsed.background = None;
        return Ok(());
    }

    let mut background = if args.len() >= 3 {
        [
            parse_integer(&args[0], "background red channel")?,
            parse_integer(&args[1], "background green channel")?,
            parse_integer(&args[2], "background blue channel")?,
            255,
        ]
    } else {
        crate::processing::utils::parse_hex_color(&args[0]).map_err(OptionParseError::Color)?
    };

    if let Some(alpha) = parsed.background_alpha {
        background[3] = (alpha * 255.0).round() as u8;
    }
    parsed.background = Some(background);

    Ok(())
}

fn parse_crop(args: &[String]) -> Result<Crop, OptionParseError> {
    if args.len() < 2 {
        return Err(OptionParseError::invalid(
            "crop option requires at least two arguments: width, height",
        ));
    }

    let width = parse_crop_extent(&args[0], "crop width")?;
    let height = parse_crop_extent(&args[1], "crop height")?;
    let gravity = match arg(args, 2) {
        Some(_) => Some(Gravity::parse(args, 2, "crop")?),
        None => None,
    };

    Ok(Crop { width, height, gravity })
}

fn parse_crop_extent(value: &str, option: &str) -> Result<f64, OptionParseError> {
    let extent = f64::from(error::parse_float(value, option)?);
    if !extent.is_finite() || extent < 0.0 {
        return Err(OptionParseError::invalid(format!(
            "{option} must be a finite non-negative number"
        )));
    }
    Ok(extent)
}
