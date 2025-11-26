use libvips::{bindings, ops, VipsImage};
use std::collections::HashSet;
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

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
                ..Default::default()
            };
            ops::jpegsave_buffer_with_opts(&img, &opts)
        }),
        "png" => encode_image("PNG", || {
            let opts = ops::PngsaveBufferOptions {
                effort,
                ..Default::default()
            };
            ops::pngsave_buffer_with_opts(&img, &opts)
        }),
        "webp" => encode_image("WebP", || {
            // Note: WebpsaveBufferOptions in libvips 1.7.1 causes crashes when used with _with_opts.
            // Using default save for WebP until the library is updated.
            ops::webpsave_buffer(&img)
        }),
        "tiff" => encode_image("TIFF", || {
            let clamped_quality = (quality as i32).clamp(1, 100);
            let compression = if clamped_quality == 100 {
                // Preserve lossless output when callers request max quality.
                ops::ForeignTiffCompression::Lzw
            } else {
                ops::ForeignTiffCompression::Jpeg
            };

            let opts = ops::TiffsaveBufferOptions {
                q: clamped_quality,
                compression,
                ..Default::default()
            };

            ops::tiffsave_buffer_with_opts(&img, &opts)
        }),
        "gif" => encode_image("GIF", || {
            let opts = ops::GifsaveBufferOptions {
                effort,
                ..Default::default()
            };

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
    let supported = supported_formats();
    if supported.contains(&lower) {
        return true;
    }

    probe_format(&lower)
}

fn supported_formats() -> &'static HashSet<String> {
    static SUPPORTED: OnceLock<HashSet<String>> = OnceLock::new();
    SUPPORTED.get_or_init(|| {
        // Probe the formats we know how to encode; this happens once at startup.
        ["jpeg", "jpg", "png", "webp", "tiff", "gif", "avif", "heif"]
            .iter()
            .filter(|fmt| probe_format(fmt))
            .map(|fmt| fmt.to_string())
            .collect()
    })
}

fn probe_format(format: &str) -> bool {
    let candidates = [format.to_string(), format!(".{}", format), format!("output.{}", format)];

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
