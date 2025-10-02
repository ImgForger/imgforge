use crate::ProcessingOption;
use image::{
    codecs::jpeg::JpegEncoder, imageops, load_from_memory, DynamicImage, GenericImageView,
    ImageBuffer, ImageFormat, Rgba,
};
use std::io::Cursor;
use exif::{In, Tag};

const RESIZE: &str = "resize";
const WIDTH: &str = "width";
const HEIGHT: &str = "height";
const GRAVITY: &str = "gravity";
const ENLARGE: &str = "enlarge";
const EXTEND: &str = "extend";
const PADDING: &str = "padding";
const ORIENTATION: &str = "orientation";
const AUTO_ROTATE: &str = "auto_rotate";
const BLUR: &str = "blur";
const CROP: &str = "crop";
const FORMAT: &str = "format";
const QUALITY: &str = "quality";
const BACKGROUND: &str = "background";

#[derive(Debug, Default)]
struct Resize {
    resizing_type: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Default)]
struct Crop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct ParsedOptions {
    resize: Option<Resize>,
    blur: Option<f32>,
    crop: Option<Crop>,
    format: Option<ImageFormat>,
    quality: Option<u8>,
    background: Option<Rgba<u8>>,
    width: Option<u32>,
    height: Option<u32>,
    gravity: Option<String>,
    enlarge: bool,
    extend: bool,
    padding: Option<(u32, u32, u32, u32)>,
    orientation: Option<u16>,
    auto_rotate: bool,
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

fn parse_all_options(options: Vec<ProcessingOption>) -> Result<ParsedOptions, String> {
    let mut parsed_options = ParsedOptions::default();

    for option in options {
        match option.name.as_str() {
            RESIZE => {
                if option.args.len() < 3 {
                    return Err(
                        "resize option requires at least 3 arguments: type, width, height"
                            .to_string(),
                    );
                }
                parsed_options.resize = Some(Resize {
                    resizing_type: option.args[0].clone(),
                    width: option.args[1]
                        .parse()
                        .map_err(|_| "Invalid width".to_string())?,
                    height: option.args[2]
                        .parse()
                        .map_err(|_| "Invalid height".to_string())?,
                });
            }
            WIDTH => {
                if option.args.len() < 1 {
                    return Err("width option requires one argument".to_string());
                }
                parsed_options.width = Some(
                    option.args[0]
                        .parse()
                        .map_err(|_| "Invalid width".to_string())?,
                );
            }
            HEIGHT => {
                if option.args.len() < 1 {
                    return Err("height option requires one argument".to_string());
                }
                parsed_options.height = Some(
                    option.args[0]
                        .parse()
                        .map_err(|_| "Invalid height".to_string())?,
                );
            }
            GRAVITY => {
                if option.args.len() < 1 {
                    return Err("gravity option requires one argument".to_string());
                }
                parsed_options.gravity = Some(option.args[0].clone());
            }
            ENLARGE => {
                if option.args.len() < 1 {
                    return Err("enlarge option requires one argument".to_string());
                }
                parsed_options.enlarge = parse_boolean(&option.args[0]);
            }
            EXTEND => {
                if option.args.len() < 1 {
                    return Err("extend option requires one argument".to_string());
                }
                parsed_options.extend = parse_boolean(&option.args[0]);
            }
            PADDING => {
                if option.args.is_empty() {
                    return Err("padding option requires at least one argument".to_string());
                }
                let values: Vec<u32> = option
                    .args
                    .iter()
                    .map(|s| s.parse().map_err(|_| "Invalid padding value".to_string()))
                    .collect::<Result<Vec<u32>, String>>()?;
                parsed_options.padding = Some(match values.len() {
                    1 => (values[0], values[0], values[0], values[0]),
                    2 => (values[0], values[1], values[0], values[1]),
                    4 => (values[0], values[1], values[2], values[3]),
                    _ => return Err("padding must have 1, 2, or 4 arguments".to_string()),
                });
            }
            ORIENTATION => {
                if option.args.len() < 1 {
                    return Err("orientation option requires one argument".to_string());
                }
                parsed_options.orientation = Some(
                    option.args[0]
                        .parse()
                        .map_err(|_| "Invalid orientation".to_string())?,
                );
            }
            AUTO_ROTATE => {
                if option.args.len() < 1 {
                    return Err("auto_rotate option requires one argument".to_string());
                }
                parsed_options.auto_rotate = parse_boolean(&option.args[0]);
            }
            BLUR => {
                if option.args.len() < 1 {
                    return Err("blur option requires one argument: sigma".to_string());
                }
                parsed_options.blur = Some(
                    option.args[0]
                        .parse()
                        .map_err(|_| "Invalid sigma for blur".to_string())?,
                );
            }
            CROP => {
                if option.args.len() < 4 {
                    return Err(
                        "crop option requires four arguments: x, y, width, height".to_string()
                    );
                }
                parsed_options.crop = Some(Crop {
                    x: option.args[0]
                        .parse()
                        .map_err(|_| "Invalid x for crop".to_string())?,
                    y: option.args[1]
                        .parse()
                        .map_err(|_| "Invalid y for crop".to_string())?,
                    width: option.args[2]
                        .parse()
                        .map_err(|_| "Invalid width for crop".to_string())?,
                    height: option.args[3]
                        .parse()
                        .map_err(|_| "Invalid height for crop".to_string())?,
                });
            }
            FORMAT => {
                if option.args.len() < 1 {
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
                    _ => return Err(format!("Unsupported format: {}", option.args[0])),
                });
            }
            QUALITY => {
                if option.args.len() < 1 {
                    return Err("quality option requires one argument".to_string());
                }
                parsed_options.quality = Some(
                    option.args[0]
                        .parse::<u8>()
                        .map_err(|_| "Invalid quality".to_string())?
                        .clamp(1, 100),
                );
            }
            BACKGROUND => {
                if option.args.len() < 1 {
                    return Err("background option requires one argument".to_string());
                }
                parsed_options.background = Some(parse_hex_color(&option.args[0])?);
            }
            _ => {}
        }
    }

    if parsed_options.resize.is_none()
        && (parsed_options.width.is_some() || parsed_options.height.is_some())
    {
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
    options: Vec<ProcessingOption>,
) -> Result<Vec<u8>, String> {
    let parsed_options = parse_all_options(options)?;

    let mut img = if parsed_options.auto_rotate {
        load_from_memory(&image_bytes).map_err(|e| e.to_string())?
    } else {
        let exif_reader = exif::Reader::new();
        let exif = exif_reader.read_raw(image_bytes.clone()).ok();
        let mut img = load_from_memory(&image_bytes).map_err(|e| e.to_string())?;
        if let Some(exif) = exif {
            if let Some(orientation) = exif.get_field(Tag::Orientation, In::PRIMARY) {
                match orientation.value.get_uint(0) {
                    Some(2) => img = img.fliph(),
                    Some(3) => img = img.rotate180(),
                    Some(4) => img = img.flipv(),
                    Some(5) => img = img.rotate90().fliph(),
                    Some(6) => img = img.rotate90(),
                    Some(7) => img = img.rotate270().fliph(),
                    Some(8) => img = img.rotate270(),
                    _ => {}
                }
            }
        }
        img
    };

    let original_format = image::guess_format(&image_bytes).map_err(|e| e.to_string())?;
    let mut background_applied = false;

    if let Some(crop) = parsed_options.crop {
        img = img.crop_imm(crop.x, crop.y, crop.width, crop.height);
    }

    if let Some(ref resize) = parsed_options.resize {
        let (w, h) = (resize.width, resize.height);

        if !parsed_options.enlarge && (w > img.width() || h > img.height()) {
            // Do not enlarge
        } else {
            img = match resize.resizing_type.as_str() {
                "fill" => {
                    if w == 0 || h == 0 {
                        return Err("resize:fill requires non-zero width and height".to_string());
                    }
                    let gravity = parsed_options.gravity.as_deref().unwrap_or("center");
                    resize_to_fill_with_gravity(&img, w, h, gravity)
                }
                "fit" => {
                    if w == 0 && h == 0 {
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

                        img.resize(target_w, target_h, imageops::FilterType::Lanczos3)
                    }
                }
                "force" => {
                    if w == 0 || h == 0 {
                        return Err("resize:force requires non-zero width and height".to_string());
                    }
                    img.resize_exact(w, h, imageops::FilterType::Lanczos3)
                }
                _ => return Err(format!("Unknown resize type: {}", resize.resizing_type)),
            };
        }
    }

    if parsed_options.extend {
        if let Some(resize) = &parsed_options.resize {
            let (w, h) = (resize.width, resize.height);
            if img.width() < w || img.height() < h {
                let mut background = ImageBuffer::from_pixel(
                    w,
                    h,
                    parsed_options
                        .background
                        .unwrap_or_else(|| Rgba([0, 0, 0, 0])),
                );
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
        let mut background = ImageBuffer::from_pixel(
            img.width() + left + right,
            img.height() + top + bottom,
            parsed_options
                .background
                .unwrap_or_else(|| Rgba([0, 0, 0, 0])),
        );
        imageops::overlay(&mut background, &img, left as i64, top as i64);
        img = DynamicImage::ImageRgba8(background);
        background_applied = true;
    }

    if let Some(orientation) = parsed_options.orientation {
        img = match orientation {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => img,
        };
    }

    if let Some(sigma) = parsed_options.blur {
        img = img.blur(sigma);
    }

    let output_format = parsed_options.format.unwrap_or(original_format);

    if let Some(bg_color) = parsed_options.background {
        if !background_applied && output_format == ImageFormat::Jpeg {
            let mut background = ImageBuffer::from_pixel(img.width(), img.height(), bg_color);
            imageops::overlay(&mut background, &img, 0i64, 0i64);
            img = DynamicImage::ImageRgba8(background);
        }
    }

    let mut buf = Cursor::new(Vec::new());
    match output_format {
        ImageFormat::Jpeg => {
            let quality = parsed_options.quality.unwrap_or(85);
            let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
            img.write_with_encoder(encoder).map_err(|e| e.to_string())?;
        }
        _ => {
            img.write_to(&mut buf, output_format)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(buf.into_inner())
}
