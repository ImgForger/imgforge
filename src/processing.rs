
use crate::ProcessingOption;
use image::{load_from_memory, DynamicImage, ImageFormat};
use std::io::Cursor;

pub async fn process_image(
    image_bytes: Vec<u8>,
    options: Vec<ProcessingOption>,
) -> Result<Vec<u8>, String> {
    let mut img = load_from_memory(&image_bytes).map_err(|e| e.to_string())?;
    let format = image::guess_format(&image_bytes).map_err(|e| e.to_string())?;

    for option in options {
        if option.name == "resize" {
            if option.args.len() < 3 {
                return Err("resize option requires at least 3 arguments: type, width, height".to_string());
            }
            let resize_type = &option.args[0];
            let width: u32 = option.args[1].parse().map_err(|_| "Invalid width".to_string())?;
            let height: u32 = option.args[2].parse().map_err(|_| "Invalid height".to_string())?;

            img = match resize_type.as_str() {
                "fill" => img.resize_to_fill(width, height, image::imageops::FilterType::Lanczos3),
                "fit" => img.resize(width, height, image::imageops::FilterType::Lanczos3),
                "force" => img.resize_exact(width, height, image::imageops::FilterType::Lanczos3),
                _ => return Err(format!("Unknown resize type: {}", resize_type)),
            };
        }
    }

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, format).map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}
