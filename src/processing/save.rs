use libvips::{bindings, ops, VipsImage};
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Saves an image to bytes in the specified format.
pub fn save_image(img: VipsImage, format: &str, quality: u8) -> Result<Vec<u8>, String> {
    let format = format.to_lowercase();

    if !is_format_supported(&format) {
        return Err(format!(
            "Output format '{}' is not supported by this libvips build",
            format
        ));
    }

    // map quality to effort (1-10), higher quality = more effort
    let effort = ((quality as i32).clamp(1, 100) / 10).clamp(1, 10);
    match format.as_str() {
        "jpeg" | "jpg" => encode_image("JPEG", || {
            let opts = ops::JpegsaveBufferOptions {
                q: quality as i32,
                optimize_coding: true,
                keep: ops::ForeignKeep::None,
                ..Default::default()
            };
            ops::jpegsave_buffer_with_opts(&img, &opts)
        }),
        "png" => encode_image("PNG", || {
            let opts = ops::PngsaveBufferOptions {
                compression: 9,
                effort,
                keep: ops::ForeignKeep::None,
                ..Default::default()
            };
            ops::pngsave_buffer_with_opts(&img, &opts)
        }),
        "webp" => encode_image("WebP", || {
            let mut opts = ops::WebpsaveBufferOptions::default();

            // Set quality for WebP
            opts.q = quality as i32;
            opts.lossless = false;
            // libvips caps WebP effort at 6 (0-6 range)
            opts.effort = effort.min(6);

            ops::webpsave_buffer_with_opts(&img, &opts)
        }),
        "tiff" => encode_image("TIFF", || {
            let mut opts = ops::TiffsaveBufferOptions::default();
            opts.q = quality as i32;
            opts.compression = ops::ForeignTiffCompression::Lzw;

            ops::tiffsave_buffer_with_opts(&img, &opts)
        }),
        "gif" => encode_image("GIF", || {
            let mut opts = ops::GifsaveBufferOptions::default();
            opts.effort = effort;

            ops::gifsave_buffer_with_opts(&img, &opts)
        }),
        _ => Err(format!("Unsupported output format: {}", format)),
    }
}

fn encode_image<F>(label: &str, op: F) -> Result<Vec<u8>, String>
where
    F: FnOnce() -> libvips::Result<Vec<u8>>,
{
    catch_unwind(AssertUnwindSafe(op))
        .map_err(|_| format!("Error encoding {}: libvips call panicked", label))?
        .map_err(|e| format!("Error encoding {}: {}", label, e))
}

fn is_format_supported(format: &str) -> bool {
    let lower = format.to_lowercase();
    let candidates = [lower.clone(), format!(".{}", lower), format!("output.{}", lower)];

    for candidate in candidates {
        if let Ok(c_str) = CString::new(candidate) {
            unsafe {
                if !bindings::vips_foreign_find_save_buffer(c_str.as_ptr()).is_null()
                    || !bindings::vips_foreign_find_save(c_str.as_ptr()).is_null()
                {
                    return true;
                }
            }
        }
    }

    false
}
