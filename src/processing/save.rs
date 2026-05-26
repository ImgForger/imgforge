use crate::processing::options::SaveOptions;
use libvips::{bindings, ops, VipsImage};
use std::collections::HashSet;
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

/// Saves an image to bytes in the specified format.
pub fn save_image(img: VipsImage, format: &str, quality: u8) -> Result<Vec<u8>, String> {
    save_image_with_options(img, format, quality, &SaveOptions::default())
}

/// Saves an image to bytes using imgproxy-compatible encoder controls.
pub fn save_image_with_options(
    img: VipsImage,
    format: &str,
    quality: u8,
    options: &SaveOptions,
) -> Result<Vec<u8>, String> {
    let format = format.to_lowercase();

    if !is_format_supported(&format) {
        return Err(format!(
            "Output format '{}' is not supported by this libvips build",
            format
        ));
    }

    encode_with_max_bytes(&img, &format, quality, options)
}

fn encode_with_max_bytes(img: &VipsImage, format: &str, quality: u8, options: &SaveOptions) -> Result<Vec<u8>, String> {
    let Some(max_bytes) = options.max_bytes else {
        return encode_once(img, format, quality, options);
    };

    let mut quality = quality.clamp(1, 100);
    loop {
        let bytes = encode_once(img, format, quality, options)?;
        if bytes.len() <= max_bytes || quality <= 1 {
            return Ok(bytes);
        }
        quality = quality.saturating_sub(5).max(1);
    }
}

fn metadata_keep(options: &SaveOptions) -> ops::ForeignKeep {
    if options.strip_metadata.unwrap_or(false) || options.strip_color_profile.unwrap_or(false) {
        ops::ForeignKeep::None
    } else {
        ops::ForeignKeep::All
    }
}

fn encode_once(img: &VipsImage, format: &str, quality: u8, options: &SaveOptions) -> Result<Vec<u8>, String> {
    // map quality to effort (1-10), higher quality = more effort
    let effort = ((quality as i32).clamp(1, 100) / 10).clamp(1, 10);
    let keep = metadata_keep(options);

    match format {
        "jpeg" | "jpg" => encode_image("JPEG", || {
            let opts = ops::JpegsaveBufferOptions {
                q: quality as i32,
                optimize_coding: true,
                interlace: options.save_jpeg_progressive(),
                trellis_quant: options.jpeg.trellis_quant.unwrap_or(false),
                overshoot_deringing: options.jpeg.overshoot_deringing.unwrap_or(false),
                optimize_scans: options.jpeg.optimize_scans.unwrap_or(false),
                quant_table: options.jpeg.quant_table.unwrap_or(0).clamp(0, 8),
                subsample_mode: if options.jpeg.no_subsample.unwrap_or(false) {
                    ops::ForeignSubsample::Off
                } else {
                    ops::ForeignSubsample::Auto
                },
                keep,
                ..Default::default()
            };
            ops::jpegsave_buffer_with_opts(img, &opts)
        }),
        "png" => encode_image("PNG", || {
            let opts = ops::PngsaveBufferOptions {
                interlace: options.png.interlaced.unwrap_or(false),
                palette: options.png.quantize.unwrap_or(false),
                q: options
                    .png
                    .quantization_colors
                    .map(|colors| colors.min(256) as i32)
                    .unwrap_or(100),
                effort,
                keep,
                ..Default::default()
            };
            ops::pngsave_buffer_with_opts(img, &opts)
        }),
        "webp" => encode_image("WebP", || {
            // libvips 1.7.x's generated WebP save bindings can abort the process, so
            // WebP encoder options are parsed for imgproxy compatibility but not applied.
            img.image_write_to_buffer(".webp")
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
                keep,
                ..Default::default()
            };

            ops::tiffsave_buffer_with_opts(img, &opts)
        }),
        "gif" => encode_image("GIF", || {
            let opts = ops::GifsaveBufferOptions {
                effort,
                ..Default::default()
            };

            ops::gifsave_buffer_with_opts(img, &opts)
        }),
        "avif" => encode_image("AVIF", || {
            let opts = ops::HeifsaveBufferOptions {
                q: quality as i32,
                compression: ops::ForeignHeifCompression::Av1,
                effort: (effort - 1).clamp(0, 9),
                subsample_mode: if options.avif.no_subsample.unwrap_or(false) {
                    ops::ForeignSubsample::Off
                } else {
                    ops::ForeignSubsample::Auto
                },
                keep,
                ..Default::default()
            };
            ops::heifsave_buffer_with_opts(img, &opts)
        }),
        "heif" | "heic" => encode_image("HEIF", || {
            let opts = ops::HeifsaveBufferOptions {
                q: quality as i32,
                effort: (effort - 1).clamp(0, 9),
                subsample_mode: if options.avif.no_subsample.unwrap_or(false) {
                    ops::ForeignSubsample::Off
                } else {
                    ops::ForeignSubsample::Auto
                },
                keep,
                ..Default::default()
            };
            ops::heifsave_buffer_with_opts(img, &opts)
        }),
        _ => Err(format!("Unsupported output format: {}", format)),
    }
}

trait SaveOptionExt {
    fn save_jpeg_progressive(&self) -> bool;
}

impl SaveOptionExt for SaveOptions {
    fn save_jpeg_progressive(&self) -> bool {
        self.jpeg.progressive.unwrap_or(false)
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
