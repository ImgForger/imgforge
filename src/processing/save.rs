use libvips::{ops, VipsImage};

/// Saves an image to bytes in the specified format.
pub fn save_image(img: VipsImage, format: &str, _quality: u8) -> Result<Vec<u8>, String> {
    match format {
        "jpeg" | "jpg" => ops::jpegsave_buffer(&img).map_err(|e| format!("Error encoding JPEG: {}", e)),
        "png" => ops::pngsave_buffer(&img).map_err(|e| format!("Error encoding PNG: {}", e)),
        "webp" => ops::webpsave_buffer(&img).map_err(|e| format!("Error encoding WebP: {}", e)),
        "tiff" => ops::tiffsave_buffer(&img).map_err(|e| format!("Error encoding TIFF: {}", e)),
        "gif" => ops::gifsave_buffer(&img).map_err(|e| format!("Error encoding GIF: {}", e)),
        _ => Err(format!("Unsupported output format: {}", format)),
    }
}
