use crate::ProcessingOption;
use image::{codecs::jpeg::JpegEncoder, imageops, load_from_memory, DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;
use exif::{In, Tag};
use tracing::{debug, error, info};

const RESIZE: &str = "resize";
const WIDTH: &str = "width";
const HEIGHT: &str = "height";
const GRAVITY: &str = "gravity";
const ENLARGE: &str = "enlarge";
const EXTEND: &str = "extend";
const PADDING: &str = "padding";
const ORIENTATION: &str = "orientation";
const AUTO_ROTATE: &str = "auto_rotate";
const RAW: &str = "raw";
const BLUR: &str = "blur";
const CROP: &str = "crop";
const FORMAT: &str = "format";
const QUALITY: &str = "quality";
const BACKGROUND: &str = "background";
const MAX_SRC_RESOLUTION: &str = "max_src_resolution";
const MAX_SRC_FILE_SIZE: &str = "max_src_file_size";
const CACHE_BUSTER: &str = "cache_buster";
const DPR: &str = "dpr";

#[derive(Debug, Default)]
pub struct Resize {
    pub resizing_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Default)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct ParsedOptions {
    pub resize: Option<Resize>,
    pub blur: Option<f32>,
    pub crop: Option<Crop>,
    pub format: Option<ImageFormat>,
    pub quality: Option<u8>,
    pub background: Option<Rgba<u8>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub gravity: Option<String>,
    pub enlarge: bool,
    pub extend: bool,
    pub padding: Option<(u32, u32, u32, u32)>,
    pub orientation: Option<u16>,
    pub auto_rotate: bool,
    pub raw: bool,
    pub max_src_resolution: Option<f32>,
    pub max_src_file_size: Option<usize>,
    pub cache_buster: Option<String>,
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
            orientation: None,
            auto_rotate: true,
            raw: false,
            max_src_resolution: None,
            max_src_file_size: None,
            cache_buster: None,
            dpr: Some(1.0),
        }
    }
}

fn parse_hex_color(hex: &str) -> Result<Rgba<u8>, String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Invalid hex color format".to_string());
    }
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex color".to_string())?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex color".to_string())?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex color".to_string())?;
    Ok(Rgba([r, g, b, 255]))
}

fn resize_to_fill_with_gravity(
    img: &DynamicImage,
    width: u32,
    height: u32,
    gravity: &str,
) -> DynamicImage {
    let (img_w, img_h) = img.dimensions();
    let aspect_ratio = img_w as f32 / img_h as f32;
    let target_aspect_ratio = width as f32 / height as f32;

    let (resize_w, resize_h) = if aspect_ratio > target_aspect_ratio {
        ((height as f32 * aspect_ratio).round() as u32, height)
    } else {
        (width, (width as f32 / aspect_ratio).round() as u32)
    };

    let resized_img = img.resize_exact(resize_w, resize_h, imageops::FilterType::Lanczos3);

    let (crop_x, crop_y) = match gravity {
        "no" | "center" => ((resize_w - width) / 2, (resize_h - height) / 2),
        "north" => ((resize_w - width) / 2, 0),
        "south" => ((resize_w - width) / 2, resize_h - height),
        "west" => (0, (resize_h - height) / 2),
        "east" => (resize_w - width, (resize_h - height) / 2),
        _ => ((resize_w - width) / 2, (resize_h - height) / 2), // Default to center
    };

    resized_img.crop_imm(crop_x, crop_y, width, height)
}

fn parse_boolean(s: &str) -> bool {
    matches!(s, "1" | "true")
}

pub fn parse_all_options(options: Vec<ProcessingOption>) -> Result<ParsedOptions, String> {
    let mut parsed_options = ParsedOptions::default();

    for option in options {
        debug!("Parsing option: {} with args: {:?}", option.name, option.args);
        match option.name.as_str() {
            RESIZE => {
                if option.args.len() < 3 {
                    error!("Resize option requires at least 3 arguments, received: {}", option.args.len());
                    return Err(
                        "resize option requires at least 3 arguments: type, width, height".to_string(),
                    );
                }
                parsed_options.resize = Some(Resize {
                    resizing_type: option.args[0].clone(),
                    width: option.args[1].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid width for resize: {}", e); e.to_string() })?,
                    height: option.args[2].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid height for resize: {}", e); e.to_string() })?,
                });
            }
            WIDTH => {
                if option.args.len() < 1 {
                    error!("Width option requires one argument");
                    return Err("width option requires one argument".to_string());
                }
                parsed_options.width = Some(option.args[0].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid width: {}", e); e.to_string() })?);
            }
            HEIGHT => {
                if option.args.len() < 1 {
                    error!("Height option requires one argument");
                    return Err("height option requires one argument".to_string());
                }
                parsed_options.height = Some(option.args[0].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid height: {}", e); e.to_string() })?);
            }
            GRAVITY => {
                if option.args.len() < 1 {
                    error!("Gravity option requires one argument");
                    return Err("gravity option requires one argument".to_string());
                }
                parsed_options.gravity = Some(option.args[0].clone());
            }
            ENLARGE => {
                if option.args.len() < 1 {
                    error!("Enlarge option requires one argument");
                    return Err("enlarge option requires one argument".to_string());
                }
                parsed_options.enlarge = parse_boolean(&option.args[0]);
            }
            EXTEND => {
                if option.args.len() < 1 {
                    error!("Extend option requires one argument");
                    return Err("extend option requires one argument".to_string());
                }
                parsed_options.extend = parse_boolean(&option.args[0]);
            }
            PADDING => {
                if option.args.is_empty() {
                    error!("Padding option requires at least one argument");
                    return Err("padding option requires at least one argument".to_string());
                }
                let values: Vec<u32> = option.args.iter().map(|s| s.parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid padding value: {}", e); e.to_string() })).collect::<Result<Vec<u32>, String>>()?;
                parsed_options.padding = Some(match values.len() {
                    1 => (values[0], values[0], values[0], values[0]),
                    2 => (values[0], values[1], values[0], values[1]),
                    4 => (values[0], values[1], values[2], values[3]),
                    _ => { error!("Padding must have 1, 2, or 4 arguments, received: {}", values.len()); return Err("padding must have 1, 2, or 4 arguments".to_string()); },
                });
            }
            ORIENTATION => {
                if option.args.len() < 1 {
                    error!("Orientation option requires one argument");
                    return Err("orientation option requires one argument".to_string());
                }
                parsed_options.orientation = Some(option.args[0].parse::<u16>().map_err(|e: std::num::ParseIntError| { error!("Invalid orientation: {}", e); e.to_string() })?);
            }
            AUTO_ROTATE => {
                if option.args.len() < 1 {
                    error!("Auto_rotate option requires one argument");
                    return Err("auto_rotate option requires one argument".to_string());
                }
                parsed_options.auto_rotate = parse_boolean(&option.args[0]);
            }
            RAW => {
                parsed_options.raw = true;
            }
            BLUR => {
                if option.args.len() < 1 {
                    error!("Blur option requires one argument: sigma");
                    return Err("blur option requires one argument: sigma".to_string());
                }
                parsed_options.blur = Some(
                    option.args[0]
                        .parse::<f32>()
                        .map_err(|e: std::num::ParseFloatError| { error!("Invalid sigma for blur: {}", e); e.to_string() })?,
                );
            }
            CROP => {
                if option.args.len() < 4 {
                    error!("Crop option requires four arguments");
                    return Err(
                        "crop option requires four arguments: x, y, width, height".to_string(),
                    );
                }
                parsed_options.crop = Some(Crop {
                    x: option.args[0].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid x for crop: {}", e); e.to_string() })?,
                    y: option.args[1].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid y for crop: {}", e); e.to_string() })?,
                    width: option.args[2].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid width for crop: {}", e); e.to_string() })?,
                    height: option.args[3].parse::<u32>().map_err(|e: std::num::ParseIntError| { error!("Invalid height for crop: {}", e); e.to_string() })?,
                });
            }
            FORMAT => {
                if option.args.len() < 1 {
                    error!("Format option requires one argument");
                    return Err("format option requires one argument".to_string());
                }
                parsed_options.format = Some(match option.args[0].as_str() {
                    "jpg" | "jpeg" => ImageFormat::Jpeg,
                    "png" => ImageFormat::Png,
                    "gif" => ImageFormat::Gif,
                    "webp" => ImageFormat::WebP,
                    "avif" => ImageFormat::Avif,
                    "tiff" => ImageFormat::Tiff,
                    "bmp" => ImageFormat::Bmp,
                    _ => { error!("Unsupported format: {}", option.args[0]); return Err(format!("Unsupported format: {}", option.args[0])); },
                });
            }
            QUALITY => {
                if option.args.len() < 1 {
                    error!("Quality option requires one argument");
                    return Err("quality option requires one argument".to_string());
                }
                parsed_options.quality = Some(
                    option.args[0]
                        .parse::<u8>()
                        .map_err(|e| { error!("Invalid quality: {}", e); e.to_string() })?
                        .clamp(1, 100),
                );
            }
            BACKGROUND => {
                if option.args.len() < 1 {
                    error!("Background option requires one argument");
                    return Err("background option requires one argument".to_string());
                }
                parsed_options.background = Some(parse_hex_color(&option.args[0]).map_err(|e| { error!("Invalid hex color for background: {}", e); e.to_string() })?);
            }
            MAX_SRC_RESOLUTION => {
                if option.args.len() < 1 {
                    error!("Max_src_resolution option requires one argument");
                    return Err("max_src_resolution option requires one argument".to_string());
                }
                parsed_options.max_src_resolution = Some(option.args[0].parse::<f32>().map_err(|e: std::num::ParseFloatError| { error!("Invalid max_src_resolution: {}", e); e.to_string() })?);
            }
            MAX_SRC_FILE_SIZE => {
                if option.args.len() < 1 {
                    error!("Max_src_file_size option requires one argument");
                    return Err("max_src_file_size option requires one argument".to_string());
                }
                parsed_options.max_src_file_size = Some(option.args[0].parse::<usize>().map_err(|e: std::num::ParseIntError| { error!("Invalid max_src_file_size: {}", e); e.to_string() })?);
            }
            CACHE_BUSTER => {
                if option.args.len() < 1 {
                    error!("Cache_buster option requires one argument");
                    return Err("cache_buster option requires one argument".to_string());
                }
                parsed_options.cache_buster = Some(option.args[0].clone());
            }
            DPR => {
                if option.args.len() < 1 {
                    error!("DPR option requires one argument");
                    return Err("dpr option requires one argument".to_string());
                }
                let dpr = option.args[0].parse::<f32>().map_err(|e| { error!("Invalid dpr value: {}", e); e.to_string() })?;
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

pub async fn process_image(
    image_bytes: Vec<u8>,
    mut parsed_options: ParsedOptions,
) -> Result<Vec<u8>, String> {
    debug!("Starting image processing with options: {:?}", parsed_options);

    if let Some(dpr) = parsed_options.dpr {
        if dpr > 1.0 {
            debug!("Applying DPR scaling: {}", dpr);
            if let Some(ref mut resize) = parsed_options.resize {
                debug!("Scaling resize dimensions from {}x{} to {}x{}", resize.width, resize.height, (resize.width as f32 * dpr).round() as u32, (resize.height as f32 * dpr).round() as u32);
                resize.width = (resize.width as f32 * dpr).round() as u32;
                resize.height = (resize.height as f32 * dpr).round() as u32;
            }
            if let Some(ref mut padding) = parsed_options.padding {
                debug!("Scaling padding from {:?} to {:?}", padding, ((padding.0 as f32 * dpr).round() as u32, (padding.1 as f32 * dpr).round() as u32, (padding.2 as f32 * dpr).round() as u32, (padding.3 as f32 * dpr).round() as u32));
                padding.0 = (padding.0 as f32 * dpr).round() as u32;
                padding.1 = (padding.1 as f32 * dpr).round() as u32;
                padding.2 = (padding.2 as f32 * dpr).round() as u32;
                padding.3 = (padding.3 as f32 * dpr).round() as u32;
            }
        }
    }

    let mut img = if parsed_options.auto_rotate {
        debug!("Auto-rotating image based on EXIF data");
        load_from_memory(&image_bytes).map_err(|e| { error!("Error loading image from memory for auto-rotate: {}", e); e.to_string() })?
    } else {
        debug!("Not auto-rotating image");
        let exif_reader = exif::Reader::new();
        let exif = exif_reader.read_raw(image_bytes.clone()).ok();
        let mut img = load_from_memory(&image_bytes).map_err(|e| { error!("Error loading image from memory: {}", e); e.to_string() })?;
        if let Some(exif) = exif {
            if let Some(orientation) = exif.get_field(Tag::Orientation, In::PRIMARY) {
                debug!("Found EXIF orientation: {:?}", orientation.value.get_uint(0));
                match orientation.value.get_uint(0) {
                    Some(2) => img = img.fliph(),
                    Some(3) => img = img.rotate180(),
                    Some(4) => img = img.flipv(),
                    Some(5) => img = img.rotate90().fliph(),
                    Some(6) => img = img.rotate90(),
                    Some(7) => img = img.rotate270().fliph(),
                    Some(8) => img = img.rotate270(),
                    _ => {},
                }
            }
        }
        img
    };

    let original_format = image::guess_format(&image_bytes).map_err(|e| { error!("Error guessing image format: {}", e); e.to_string() })?;
    debug!("Original image format: {:?}", original_format);
    let mut background_applied = false;

    if let Some(crop) = parsed_options.crop {
        debug!("Applying crop: {:?}", crop);
        img = img.crop_imm(crop.x, crop.y, crop.width, crop.height);
    }

    if let Some(ref resize) = parsed_options.resize {
        debug!("Applying resize: {:?}", resize);
        let (w, h) = (resize.width, resize.height);

        if !parsed_options.enlarge && (w > img.width() || h > img.height()) {
            debug!("Not enlarging image as enlarge is false and target dimensions are larger than source");
            // Do not enlarge
        } else {
            img = match resize.resizing_type.as_str() {
                "fill" => {
                    if w == 0 || h == 0 {
                        error!("Resize:fill requires non-zero width and height");
                        return Err("resize:fill requires non-zero width and height".to_string());
                    }
                    let gravity = parsed_options.gravity.as_deref().unwrap_or("center");
                    debug!("Resizing to fill with gravity: {}", gravity);
                    resize_to_fill_with_gravity(&img, w, h, gravity)
                }
                "fit" => {
                    if w == 0 && h == 0 {
                        debug!("Resizing to fit, no dimensions specified, returning original image");
                        img
                    } else {
                        let (img_w, img_h) = img.dimensions();
                        let aspect_ratio = img_w as f32 / img_h as f32;

                        let (target_w, target_h) = if h == 0 {
                            (w, (w as f32 / aspect_ratio).round() as u32)
                        } else if w == 0 {
                            ((h as f32 * aspect_ratio).round() as u32, h)
                        } else {
                            (w, h)
                        };
                        debug!("Resizing to fit from {}x{} to {}x{}", img_w, img_h, target_w, target_h);
                        img.resize(target_w, target_h, imageops::FilterType::Lanczos3)
                    }
                }
                "force" => {
                    if w == 0 || h == 0 {
                        error!("Resize:force requires non-zero width and height");
                        return Err("resize:force requires non-zero width and height".to_string());
                    }
                    debug!("Resizing to force {}x{}", w, h);
                    img.resize_exact(w, h, imageops::FilterType::Lanczos3)
                }
                _ => { error!("Unknown resize type: {}", resize.resizing_type); return Err(format!("Unknown resize type: {}", resize.resizing_type)); },
            };
        }
    }

    if parsed_options.extend {
        debug!("Applying extend option");
        if let Some(resize) = &parsed_options.resize {
            let (w, h) = (resize.width, resize.height);
            if img.width() < w || img.height() < h {
                debug!("Extending image to {}x{}", w, h);
                let mut background = ImageBuffer::from_pixel(w, h, parsed_options.background.unwrap_or_else(|| Rgba([0, 0, 0, 0])));
                let gravity = parsed_options.gravity.as_deref().unwrap_or("center");
                let (x, y) = match gravity {
                    "center" => ((w - img.width()) / 2, (h - img.height()) / 2),
                    "north" => ((w - img.width()) / 2, 0),
                    "south" => ((w - img.width()) / 2, h - img.height()),
                    "west" => (0, (h - img.height()) / 2),
                    "east" => (w - img.width(), (h - img.height()) / 2),
                    _ => ((w - img.width()) / 2, (h - img.height()) / 2),
                };
                imageops::overlay(&mut background, &img, x as i64, y as i64);
                img = DynamicImage::ImageRgba8(background);
                background_applied = true;
            }
        }
    }

    if let Some((top, right, bottom, left)) = parsed_options.padding {
        debug!("Applying padding: {:?}", (top, right, bottom, left));
        let mut background = ImageBuffer::from_pixel(img.width() + left + right, img.height() + top + bottom, parsed_options.background.unwrap_or_else(|| Rgba([0, 0, 0, 0])));
        imageops::overlay(&mut background, &img, left as i64, top as i64);
        img = DynamicImage::ImageRgba8(background);
        background_applied = true;
    }

    if let Some(orientation) = parsed_options.orientation {
        debug!("Applying orientation: {}", orientation);
        img = match orientation {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => img,
        };
    }

    if let Some(sigma) = parsed_options.blur {
        debug!("Applying blur with sigma: {}", sigma);
        img = img.blur(sigma);
    }

    let output_format = parsed_options.format.unwrap_or(original_format);
    debug!("Output format: {:?}", output_format);

    if let Some(bg_color) = parsed_options.background {
        if !background_applied && output_format == ImageFormat::Jpeg {
            debug!("Applying background color for JPEG output: {:?}", bg_color);
            let mut background = ImageBuffer::from_pixel(img.width(), img.height(), bg_color);
            imageops::overlay(&mut background, &img, 0i64, 0i64);
            img = DynamicImage::ImageRgba8(background);
        }
    }

    let mut buf = Cursor::new(Vec::new());
    match output_format {
        ImageFormat::Jpeg => {
            let quality = parsed_options.quality.unwrap_or(85);
            debug!("Encoding to JPEG with quality: {}", quality);
            let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
            img.write_with_encoder(encoder).map_err(|e| { error!("Error encoding JPEG: {}", e); e.to_string() })?;
        }
        _ => {
            debug!("Encoding to {:?} format", output_format);
            img.write_to(&mut buf, output_format).map_err(|e| { error!("Error encoding image to {:?}: {}", output_format, e); e.to_string() })?;
        }
    }

    debug!("Image processing complete");
    Ok(buf.into_inner())
}