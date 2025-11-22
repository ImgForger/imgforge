use libvips::{ops, VipsImage};

/// Saves an image to bytes in the specified format.
pub fn save_image(img: VipsImage, format: &str, quality: u8) -> Result<Vec<u8>, String> {
    match format {
        "jpeg" | "jpg" => {
            let opts = ops::JpegsaveBufferOptions {
                q: quality as i32,
                optimize_coding: true,
                keep: ops::ForeignKeep::None,
                ..Default::default()
            };
            ops::jpegsave_buffer_with_opts(&img, &opts).map_err(|e| format!("Error encoding JPEG: {}", e))
        }
        "png" => {
            // PNG: map quality to effort (1-10), higher quality = more effort
            let effort = ((quality as i32).clamp(1, 100) / 10).clamp(1, 10);
            let opts = ops::PngsaveBufferOptions {
                compression: 9,
                effort,
                keep: ops::ForeignKeep::None,
                ..Default::default()
            };
            ops::pngsave_buffer_with_opts(&img, &opts).map_err(|e| format!("Error encoding PNG: {}", e))
        }
        "webp" => {
            let mut opts = ops::WebpsaveBufferOptions::default();

            // Set quality for WebP
            opts.q = quality as i32;
            opts.lossless = false;

            // Note: WebpsaveBufferOptions in libvips 1.7.1 causes crashes when used with _with_opts.
            // Using default save for WebP until the library is updated.
            ops::webpsave_buffer_with_opts(&img, &opts).map_err(|e| format!("Error encoding WebP: {}", e))
        }
        "tiff" => ops::tiffsave_buffer(&img).map_err(|e| format!("Error encoding TIFF: {}", e)),
        "gif" => ops::gifsave_buffer(&img).map_err(|e| format!("Error encoding GIF: {}", e)),
        _ => Err(format!("Unsupported output format: {}", format)),
    }
}
