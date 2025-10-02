use crate::ProcessingOption;
use image::{
    codecs::jpeg::JpegEncoder, imageops, load_from_memory, DynamicImage, ImageBuffer, ImageFormat,
    Rgba,
};
use std::io::Cursor;

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

pub async fn process_image(
    image_bytes: Vec<u8>,
    options: Vec<ProcessingOption>,
) -> Result<Vec<u8>, String> {
    let mut img = load_from_memory(&image_bytes).map_err(|e| e.to_string())?;
    let mut output_format = image::guess_format(&image_bytes).map_err(|e| e.to_string())?;
    let mut quality = 85u8;
    let mut background_color: Option<Rgba<u8>> = None;

    for option in options {
        if option.name == "resize" {
            if option.args.len() < 3 {
                return Err(
                    "resize option requires at least 3 arguments: type, width, height".to_string(),
                );
            }
            let resize_type = &option.args[0];
            let width: u32 = option.args[1].parse().map_err(|_| "Invalid width".to_string())?;
            let height: u32 = option.args[2].parse().map_err(|_| "Invalid height".to_string())?;

            img = match resize_type.as_str() {
                "fill" => img.resize_to_fill(width, height, imageops::FilterType::Lanczos3),
                "fit" => img.resize(width, height, imageops::FilterType::Lanczos3),
                "force" => img.resize_exact(width, height, imageops::FilterType::Lanczos3),
                _ => return Err(format!("Unknown resize type: {}", resize_type)),
            };
        } else if option.name == "blur" {
            if option.args.len() < 1 {
                return Err("blur option requires one argument: sigma".to_string());
            }
            let sigma: f32 =
                option.args[0].parse().map_err(|_| "Invalid sigma for blur".to_string())?;
            img = img.blur(sigma);
        } else if option.name == "crop" {
            if option.args.len() < 4 {
                return Err(
                    "crop option requires four arguments: x, y, width, height".to_string(),
                );
            }
            let x: u32 = option.args[0].parse().map_err(|_| "Invalid x for crop".to_string())?;
            let y: u32 = option.args[1].parse().map_err(|_| "Invalid y for crop".to_string())?;
            let width: u32 =
                option.args[2].parse().map_err(|_| "Invalid width for crop".to_string())?;
            let height: u32 =
                option.args[3].parse().map_err(|_| "Invalid height for crop".to_string())?;
            img = img.crop_imm(x, y, width, height);
        } else if option.name == "format" {
            if option.args.len() < 1 {
                return Err("format option requires one argument".to_string());
            }
            output_format = match option.args[0].as_str() {
                "jpg" | "jpeg" => ImageFormat::Jpeg,
                "png" => ImageFormat::Png,
                "gif" => ImageFormat::Gif,
                "webp" => ImageFormat::WebP,
                "avif" => ImageFormat::Avif,
                "tiff" => ImageFormat::Tiff,
                "bmp" => ImageFormat::Bmp,
                _ => return Err(format!("Unsupported format: {}", option.args[0])),
            };
        } else if option.name == "quality" {
            if option.args.len() < 1 {
                return Err("quality option requires one argument".to_string());
            }
            quality = option.args[0]
                .parse::<u8>()
                .map_err(|_| "Invalid quality".to_string())?
                .clamp(1, 100);
        } else if option.name == "background" {
            if option.args.len() < 1 {
                return Err("background option requires one argument".to_string());
            }
            background_color = Some(parse_hex_color(&option.args[0])?);
        }
    }

    if let Some(bg_color) = background_color {
        if output_format == ImageFormat::Jpeg {
            let mut background = ImageBuffer::from_pixel(img.width(), img.height(), bg_color);
            imageops::overlay(&mut background, &img, 0, 0);
            img = DynamicImage::ImageRgba8(background);
        }
    }

    let mut buf = Cursor::new(Vec::new());
    match output_format {
        ImageFormat::Jpeg => {
            let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
            img.write_with_encoder(encoder).map_err(|e| e.to_string())?;
        }
        _ => {
            img.write_to(&mut buf, output_format).map_err(|e| e.to_string())?;
        }
    }

    Ok(buf.into_inner())
}