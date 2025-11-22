use libvips::{ops, VipsImage};

/// Saves an image to bytes in the specified format.
pub fn save_image(img: VipsImage, format: &str, quality: u8) -> Result<Vec<u8>, String> {
    // map quality to effort (1-10), higher quality = more effort
    let effort = ((quality as i32).clamp(1, 100) / 10).clamp(1, 10);
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
            // libvips caps WebP effort at 6 (0-6 range)
            opts.effort = effort.min(6);

            ops::webpsave_buffer_with_opts(&img, &opts).map_err(|e| format!("Error encoding WebP: {}", e))
        }
        "tiff" => {
            let mut opts = ops::TiffsaveBufferOptions::default();
            opts.q = quality as i32;
            opts.compression = ops::ForeignTiffCompression::Lzw;

            ops::tiffsave_buffer_with_opts(&img, &opts).map_err(|e| format!("Error encoding TIFF: {}", e))
        }
        "gif" => {
            let mut opts = ops::GifsaveBufferOptions::default();
            opts.effort = effort;

            ops::gifsave_buffer_with_opts(&img, &opts).map_err(|e| format!("Error encoding GIF: {}", e))
        }
        _ => Err(format!("Unsupported output format: {}", format)),
    }
}
