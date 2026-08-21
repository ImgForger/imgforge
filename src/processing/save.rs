//! Encoding a processed image.
//!
//! Every format goes through libvips' save-suffix parser rather than the
//! generated `*save_buffer_with_opts` bindings. Those bindings pass every
//! option as a varargs name/value pair, including properties that only exist in
//! libvips 8.16 and later — `exact` on webpsave, `tune` on heifsave,
//! `keep-duplicate-frames` on gifsave. An older libvips rejects the entire call
//! with "no property named ...", so nothing encodes at all; Ubuntu 24.04 ships
//! 8.15.1, which is exactly that case.
//!
//! The suffix sets only the options named here, so it stays correct across
//! libvips versions, and it is the only form that can express a combination of
//! metadata `keep` flags — `keep=exif|icc` has no counterpart in the generated
//! bindings' single-variant enum.

use crate::processing::options::SaveOptions;
use libvips::{bindings, VipsImage};
use std::collections::HashSet;
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;
use thiserror::Error;
use tracing::debug;

/// Errors produced while encoding an image.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SaveError {
    #[error("output format {format:?} is not supported by this libvips build")]
    UnsupportedFormat { format: String },
    #[error("error encoding {format}: libvips call panicked")]
    EncoderPanicked { format: &'static str },
    #[error("error encoding {format}: {source}")]
    Vips {
        format: &'static str,
        #[source]
        source: libvips::error::Error,
    },
}

/// Every format imgforge knows how to encode, with the capabilities that decide
/// what the pipeline may hand the encoder.
struct FormatSpec {
    /// Canonical name, which is also the file suffix vips is given.
    name: &'static str,
    /// Whether the format can carry an alpha channel.
    alpha: bool,
    /// Whether the format can carry an embedded colour profile.
    color_profile: bool,
    /// Whether the format can carry more than one frame.
    animation: bool,
    /// Whether the format can carry more than 8 bits per channel.
    high_bit_depth: bool,
    /// Largest side the format's container can address, if it is bounded
    /// tightly enough to matter in practice.
    max_dimension: Option<u32>,
}

const FORMATS: &[FormatSpec] = &[
    FormatSpec {
        name: "jpeg",
        alpha: false,
        color_profile: true,
        animation: false,
        high_bit_depth: false,
        // libjpeg's own JPEG_MAX_DIMENSION, which is deliberately below what
        // the 16-bit SOF fields could hold. Taking the container's 65_535 let
        // a result between 65_501 and 65_535 skip the fit and then fail in the
        // encoder — the failure this limit exists to turn into a downscale.
        max_dimension: Some(65_500),
    },
    FormatSpec {
        name: "png",
        alpha: true,
        color_profile: true,
        animation: false,
        high_bit_depth: true,
        max_dimension: None,
    },
    FormatSpec {
        name: "webp",
        alpha: true,
        color_profile: true,
        animation: true,
        high_bit_depth: false,
        // libwebp refuses anything larger; see its encode.h.
        max_dimension: Some(16_383),
    },
    FormatSpec {
        name: "tiff",
        alpha: true,
        color_profile: true,
        animation: false,
        high_bit_depth: true,
        max_dimension: None,
    },
    FormatSpec {
        name: "gif",
        alpha: true,
        color_profile: false,
        animation: true,
        high_bit_depth: false,
        max_dimension: Some(65_535),
    },
    FormatSpec {
        name: "avif",
        alpha: true,
        color_profile: true,
        animation: true,
        high_bit_depth: true,
        max_dimension: Some(16_384),
    },
    FormatSpec {
        name: "heif",
        alpha: true,
        color_profile: true,
        animation: true,
        high_bit_depth: true,
        max_dimension: Some(16_384),
    },
];

/// Resolves a requested format name, including the aliases URLs use.
fn canonical_format(format: &str) -> Option<&'static FormatSpec> {
    let name = match format.to_lowercase().as_str() {
        "jpg" | "jpeg" => "jpeg",
        "heic" | "heif" => "heif",
        "tif" | "tiff" => "tiff",
        other => return FORMATS.iter().find(|spec| spec.name == other),
    };
    FORMATS.iter().find(|spec| spec.name == name)
}

/// The canonical spelling of a requested format, or `None` if unrecognised.
///
/// URLs may name a format by any of its aliases, and everything downstream keys
/// off the string that arrives: the response `Content-Type`, the
/// `format_quality` lookup, and the metrics label. Left un-normalised,
/// `format:tif` selected the TIFF encoder and then fell through
/// `format_to_content_type`'s catch-all, so the client received TIFF bytes
/// labelled `image/jpeg` — and its metrics landed under a second, separate
/// format name.
pub fn canonical_format_name(format: &str) -> Option<&'static str> {
    canonical_format(format).map(|spec| spec.name)
}

/// Whether the format can carry an alpha channel.
pub fn format_supports_alpha(format: &str) -> bool {
    canonical_format(format).is_some_and(|spec| spec.alpha)
}

/// Whether the format can carry an embedded colour profile.
pub fn format_supports_color_profile(format: &str) -> bool {
    canonical_format(format).is_some_and(|spec| spec.color_profile)
}

/// Whether the format can carry more than one frame.
pub fn format_supports_animation(format: &str) -> bool {
    canonical_format(format).is_some_and(|spec| spec.animation)
}

/// Whether the format can carry more than 8 bits per channel.
pub fn format_supports_high_bit_depth(format: &str) -> bool {
    canonical_format(format).is_some_and(|spec| spec.high_bit_depth)
}

/// Largest side the format's container can address.
pub fn format_max_dimension(format: &str) -> Option<u32> {
    canonical_format(format).and_then(|spec| spec.max_dimension)
}

/// Saves an image to bytes in the specified format.
pub fn save_image(img: VipsImage, format: &str, quality: u8) -> Result<Vec<u8>, SaveError> {
    save_image_with_options(img, format, quality, &SaveOptions::default(), None)
}

/// Saves an image to bytes using imgproxy-compatible encoder controls.
///
/// `page_height` carries the frame height of an animation. libvips stores an
/// animation as one tall image and needs telling where the frames divide;
/// without it the encoder writes a single very tall still.
pub fn save_image_with_options(
    img: VipsImage,
    format: &str,
    quality: u8,
    options: &SaveOptions,
    page_height: Option<i32>,
) -> Result<Vec<u8>, SaveError> {
    let Some(spec) = canonical_format(format) else {
        return Err(SaveError::UnsupportedFormat {
            format: format.to_string(),
        });
    };

    if !is_format_supported(spec.name) {
        return Err(SaveError::UnsupportedFormat {
            format: format.to_string(),
        });
    }

    encode_with_max_bytes(&img, spec, quality, options, page_height)
}

fn encode_with_max_bytes(
    img: &VipsImage,
    spec: &FormatSpec,
    quality: u8,
    options: &SaveOptions,
    page_height: Option<i32>,
) -> Result<Vec<u8>, SaveError> {
    let Some(max_bytes) = options.max_bytes else {
        return encode_once(img, spec, quality, options, page_height);
    };

    let mut quality = quality.clamp(1, 100);
    loop {
        let bytes = encode_once(img, spec, quality, options, page_height)?;
        if bytes.len() <= max_bytes || quality <= 1 {
            return Ok(bytes);
        }
        quality = quality.saturating_sub(5).max(1);
    }
}

/// The `keep` flag combination for a request.
///
/// libvips' flags are `none|exif|xmp|iptc|icc|other|gainmap|all`, and the two
/// strip options address different subsets of them: `strip_metadata` drops the
/// descriptive tags, `strip_color_profile` drops the ICC profile. Treating
/// either as "drop everything" — which is what a single-variant enum forces —
/// meant asking to drop the colour profile also silently discarded the EXIF.
fn metadata_keep(options: &SaveOptions) -> String {
    let strip_metadata = options.strip_metadata.unwrap_or(false);
    let strip_profile = options.strip_color_profile.unwrap_or(false);

    if !strip_metadata && !strip_profile {
        return "all".to_string();
    }

    let mut flags: Vec<&str> = Vec::new();
    if !strip_metadata {
        flags.extend_from_slice(&["exif", "xmp", "iptc", "other"]);
    }
    if !strip_profile {
        flags.push("icc");
    }
    // A gain map is what makes an HDR image high dynamic range; it is neither
    // descriptive metadata nor a colour profile, so neither strip option should
    // take it away when the request explicitly asked to preserve it.
    if options.preserve_hdr.unwrap_or(false) {
        flags.push("gainmap");
    }

    if flags.is_empty() {
        "none".to_string()
    } else {
        flags.join("|")
    }
}

/// Builds a libvips save suffix: `.png[option,option=value]`.
struct Suffix {
    parts: Vec<String>,
    extension: &'static str,
}

impl Suffix {
    fn new(extension: &'static str) -> Self {
        Self {
            parts: Vec::new(),
            extension,
        }
    }

    fn flag(mut self, name: &str, enabled: bool) -> Self {
        if enabled {
            self.parts.push(name.to_string());
        }
        self
    }

    fn value(mut self, name: &str, value: impl std::fmt::Display) -> Self {
        self.parts.push(format!("{name}={value}"));
        self
    }

    fn maybe(self, name: &str, value: Option<impl std::fmt::Display>) -> Self {
        match value {
            Some(value) => self.value(name, value),
            None => self,
        }
    }

    fn build(self) -> String {
        if self.parts.is_empty() {
            return format!(".{}", self.extension);
        }
        format!(".{}[{}]", self.extension, self.parts.join(","))
    }
}

/// Maps a requested WebP preset to the matching vips nickname.
///
/// `preset` reaches us as free text from the URL, so only names vips actually
/// defines may be interpolated into the option string; anything else is
/// dropped and the encoder default applies.
fn webp_preset_nickname(preset: &str) -> Option<&'static str> {
    match preset {
        "default" => Some("default"),
        "picture" => Some("picture"),
        "photo" => Some("photo"),
        "drawing" => Some("drawing"),
        "icon" => Some("icon"),
        "text" => Some("text"),
        _ => None,
    }
}

/// Builds the encoder suffix for one format.
pub(crate) fn save_suffix(
    format: &str,
    quality: u8,
    options: &SaveOptions,
    page_height: Option<i32>,
) -> Result<String, SaveError> {
    let Some(spec) = canonical_format(format) else {
        return Err(SaveError::UnsupportedFormat {
            format: format.to_string(),
        });
    };

    let quality = i32::from(quality).clamp(1, 100);
    // Map quality onto the effort scales the slower encoders take, so a request
    // for high quality also buys the extra search those formats offer.
    let effort = (quality / 10).clamp(1, 10);
    let keep = metadata_keep(options);
    let page_height = page_height.filter(|height| *height > 0 && spec.animation);

    let suffix = match spec.name {
        "jpeg" => Suffix::new("jpg")
            .value("Q", quality)
            .flag("optimize-coding", true)
            .flag("interlace", options.jpeg.progressive.unwrap_or(false))
            .flag("trellis-quant", options.jpeg.trellis_quant.unwrap_or(false))
            .flag("overshoot-deringing", options.jpeg.overshoot_deringing.unwrap_or(false))
            .flag("optimize-scans", options.jpeg.optimize_scans.unwrap_or(false))
            .value("quant-table", options.jpeg.quant_table.unwrap_or(0).clamp(0, 8))
            .value(
                "subsample-mode",
                if options.jpeg.no_subsample.unwrap_or(false) {
                    "off"
                } else {
                    "auto"
                },
            )
            .value("keep", keep),
        "png" => {
            let palette = options.png.quantize.unwrap_or(false);
            Suffix::new("png")
                .flag("interlace", options.png.interlaced.unwrap_or(false))
                .flag("palette", palette)
                .value(
                    "Q",
                    options
                        .png
                        .quantization_colors
                        .map(|colors| i32::from(colors.min(256)))
                        .unwrap_or(100),
                )
                .value("effort", effort)
                .value("keep", keep)
        }
        "webp" => Suffix::new("webp")
            .value("Q", quality)
            .flag("lossless", options.webp.lossless.unwrap_or(false))
            .flag("smart-subsample", options.webp.smart_subsample.unwrap_or(false))
            .maybe("preset", options.webp.preset.as_deref().and_then(webp_preset_nickname))
            .maybe("page-height", page_height)
            .value("keep", keep),
        "tiff" => Suffix::new("tif")
            .value("Q", quality)
            .value(
                "compression",
                // Preserve lossless output when callers request max quality.
                if quality == 100 { "lzw" } else { "jpeg" },
            )
            .value("keep", keep),
        "gif" => Suffix::new("gif")
            .value("effort", effort.clamp(1, 10))
            .maybe("page-height", page_height)
            .value("keep", keep),
        "avif" | "heif" => {
            let (extension, compression) = if spec.name == "avif" {
                ("avif", "av1")
            } else {
                ("heif", "hevc")
            };
            Suffix::new(extension)
                .value("Q", quality)
                .value("compression", compression)
                .value("effort", (effort - 1).clamp(0, 9))
                .value(
                    "subsample-mode",
                    if options.avif.no_subsample.unwrap_or(false) {
                        "off"
                    } else {
                        "auto"
                    },
                )
                .maybe("page-height", page_height)
                .value("keep", keep)
        }
        other => {
            return Err(SaveError::UnsupportedFormat {
                format: other.to_string(),
            })
        }
    };

    Ok(suffix.build())
}

fn encode_once(
    img: &VipsImage,
    spec: &FormatSpec,
    quality: u8,
    options: &SaveOptions,
    page_height: Option<i32>,
) -> Result<Vec<u8>, SaveError> {
    let suffix = save_suffix(spec.name, quality, options, page_height)?;
    let label = spec.name;

    catch_unwind(AssertUnwindSafe(|| img.image_write_to_buffer(&suffix)))
        .map_err(|_| SaveError::EncoderPanicked {
            format: static_label(label),
        })?
        .map_err(|source| SaveError::Vips {
            format: static_label(label),
            source,
        })
}

/// Format names are compile-time constants; this hands the error type the
/// `'static` one matching a runtime name.
fn static_label(name: &str) -> &'static str {
    FORMATS
        .iter()
        .find(|spec| spec.name == name)
        .map(|spec| spec.name)
        .unwrap_or("image")
}

pub(crate) fn is_format_supported(format: &str) -> bool {
    let Some(spec) = canonical_format(format) else {
        return false;
    };
    supported_formats().contains(spec.name)
}

fn supported_formats() -> &'static HashSet<&'static str> {
    static SUPPORTED: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SUPPORTED.get_or_init(|| {
        let supported: HashSet<&'static str> = FORMATS
            .iter()
            .filter(|spec| encoder_available(spec.name))
            .map(|spec| spec.name)
            .collect();
        debug!("libvips can encode: {:?}", supported);
        supported
    })
}

/// Whether this libvips build can actually produce the format.
///
/// `vips_foreign_find_save` only answers whether a *saver* is registered, which
/// for HEIF is true on a build with no HEVC encoder behind it — the request then
/// failed at encode time with "Unsupported compression" and a 500, long after
/// the point where "this format is unavailable" could have been reported. The
/// codec-backed formats are therefore probed by encoding a real pixel once, at
/// startup, and the answer is cached for the process's lifetime.
fn encoder_available(format: &str) -> bool {
    if !saver_registered(format) {
        return false;
    }

    if !matches!(format, "avif" | "heif") {
        return true;
    }

    let Ok(probe) = libvips::ops::black(2, 2) else {
        return false;
    };
    let Ok(suffix) = save_suffix(format, 50, &SaveOptions::default(), None) else {
        return false;
    };

    match catch_unwind(AssertUnwindSafe(|| probe.image_write_to_buffer(&suffix))) {
        Ok(Ok(bytes)) => !bytes.is_empty(),
        Ok(Err(err)) => {
            debug!("{} saver is registered but cannot encode: {}", format, err);
            false
        }
        Err(_) => false,
    }
}

fn saver_registered(format: &str) -> bool {
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
