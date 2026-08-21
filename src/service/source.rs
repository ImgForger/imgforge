//! Identifying the source image and deciding how to open it.

use crate::config::DefaultOutputFormat;
use crate::processing::animation::LoadPlan;
use crate::processing::options::{Gravity, GravityType, ParsedOptions};
use crate::processing::{load_scale_factor, load_shrink_factor};
use crate::utils::content_type_to_format;
use bytes::Bytes;
use libvips::VipsImage;
use tracing::debug;

/// Identifies a format from its magic bytes.
///
/// The `Content-Type` a server advertises is frequently wrong or absent, and
/// every decision downstream — which loader options are legal, whether the
/// source can be passed through untouched — depends on knowing what the bytes
/// actually are.
pub fn sniff_image_format(image_bytes: &[u8]) -> Option<&'static str> {
    if image_bytes.len() >= 3 && image_bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpeg");
    }

    if image_bytes.len() >= 8 && image_bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }

    if image_bytes.len() >= 6 && (image_bytes.starts_with(b"GIF87a") || image_bytes.starts_with(b"GIF89a")) {
        return Some("gif");
    }

    if image_bytes.len() >= 12 && image_bytes.starts_with(b"RIFF") && &image_bytes[8..12] == b"WEBP" {
        return Some("webp");
    }

    if image_bytes.len() >= 4 && (image_bytes.starts_with(b"II*\0") || image_bytes.starts_with(b"MM\0*")) {
        return Some("tiff");
    }

    if image_bytes.len() >= 5 && image_bytes.starts_with(b"%PDF-") {
        return Some("pdf");
    }

    if image_bytes.len() >= 12
        && image_bytes[4..8] == *b"ftyp"
        && matches!(
            &image_bytes[8..12],
            b"avif" | b"avis" | b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1"
        )
    {
        let brand = &image_bytes[8..12];
        return Some(if brand == b"avif" || brand == b"avis" {
            "avif"
        } else {
            "heif"
        });
    }

    // SVG has no magic number, only a root element that may sit behind an XML
    // declaration, a doctype, or a byte-order mark.
    if looks_like_svg(image_bytes) {
        return Some("svg");
    }

    None
}

fn looks_like_svg(image_bytes: &[u8]) -> bool {
    let prefix = &image_bytes[..image_bytes.len().min(1024)];
    let text = String::from_utf8_lossy(prefix);
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    (trimmed.starts_with("<?xml") || trimmed.starts_with("<!DOCTYPE svg") || trimmed.starts_with("<svg"))
        && text.contains("<svg")
}

/// Resolves the format to report for a source, preferring the declared type.
pub fn detect_image_format(content_type: Option<&str>, image_bytes: &[u8]) -> String {
    if let Some(format) = content_type.and_then(content_type_to_format) {
        return format.to_string();
    }

    sniff_image_format(image_bytes).unwrap_or("unknown").to_string()
}

/// The loader option string for a source, or an empty string for the defaults.
pub fn loader_options(options: &ParsedOptions, source_format: Option<&str>, output_format: &str) -> String {
    LoadPlan::resolve(options, source_format, output_format)
        .map(|plan| plan.as_load_options())
        .unwrap_or_default()
}

/// Whether the request may be answered with the source bytes as they arrived.
///
/// `skip_processing` names source formats to leave alone. imgproxy applies it
/// only when the output format matches the source, because a request that also
/// asks for a different format is asking for work that cannot be skipped.
pub fn can_skip_processing(
    options: &ParsedOptions,
    source_format: Option<&str>,
    default_format: DefaultOutputFormat,
) -> bool {
    if options.skip_processing.is_empty() {
        return false;
    }
    let Some(source_format) = source_format else {
        return false;
    };

    // Compared through the one alias table rather than a copy of it. This used
    // to spell out jpg/jpeg and heic/heif by hand and simply omit tif/tiff, so
    // `skip_processing:tif` never matched a TIFF source — a second table that
    // had drifted from the real one. Routing through `canonical_format_name`
    // means a format added there is understood here for free.
    let canonical = crate::processing::save::canonical_format_name;
    let matches_source = |candidate: &str| match (canonical(candidate), canonical(source_format)) {
        (Some(candidate), Some(source)) => candidate == source,
        // A name imgforge cannot encode can still be compared to itself, which
        // keeps an unrecognised `skip_processing` entry behaving predictably
        // rather than matching everything or nothing.
        _ => candidate.eq_ignore_ascii_case(source_format),
    };

    if !options.skip_processing.iter().any(|listed| matches_source(listed)) {
        return false;
    }

    // A requested format that differs from the source is a conversion, and a
    // conversion is processing. The URL is not the only thing that can request
    // one: a fixed `IMGFORGE_DEFAULT_FORMAT` says every response is that format,
    // so a URL naming none is still asking for a conversion. Reading an absent
    // URL format as "same as the source" let `skip_processing:jpeg` hand back a
    // JPEG from a deployment configured to serve only WebP.
    match options.format.as_deref().or_else(|| default_format.fixed_format()) {
        Some(requested) => matches_source(requested),
        None => true,
    }
}

/// Whether EXIF orientation will transpose the image during processing.
pub fn swaps_axes(parsed_options: &ParsedOptions, image_bytes: &Bytes) -> bool {
    parsed_options.auto_rotate
        && matches!(
            crate::utils::read_exif_orientation(image_bytes),
            Some(5) | Some(6) | Some(7) | Some(8)
        )
}

/// Rewrites an absolute crop region to match a source decoded at a reduced size.
///
/// Rounds the region up: it is clamped to the image by the crop itself anyway,
/// and rounding down could leave it fractionally smaller than the resize target,
/// which `enlarge:false` would then refuse to make up.
///
/// Each axis is decided on its own. A fractional extent is already measured
/// against whatever was decoded and needs no rewriting, but that is a property
/// of the axis rather than of the crop: `crop:0.5:1000` mixes the two, and
/// exempting the whole crop because one axis was fractional left the absolute
/// one addressing full-resolution coordinates.
pub fn rescale_crop(parsed_options: &mut ParsedOptions, original: (i32, i32), shrunk: (i32, i32)) {
    let Some(crop) = parsed_options.crop.as_mut() else {
        return;
    };

    let (ow, oh) = (f64::from(original.0), f64::from(original.1));
    let (sw, sh) = (f64::from(shrunk.0), f64::from(shrunk.1));
    if ow <= 0.0 || oh <= 0.0 || sw <= 0.0 || sh <= 0.0 {
        return;
    }

    // A zero extent already means "all of it" and stays that way.
    if crop.width >= 1.0 {
        crop.width = (crop.width * (sw / ow)).ceil().max(1.0);
    }
    if crop.height >= 1.0 {
        crop.height = (crop.height * (sh / oh)).ceil().max(1.0);
    }

    // The window's *position* is measured in the same pixels as its size, so an
    // absolute gravity offset has to move with it. Rewriting only the extents
    // left a `crop:...:nowe:400:0` against a 4x shrink offsetting by 400px of a
    // quarter-size image — four times too far, selecting the wrong region of the
    // source entirely.
    //
    // The scaled offsets are written into the crop's own gravity rather than the
    // request's. `crop_gravity` falls back to `gravity` when the crop names
    // none, but `fill_gravity` reads that same field to position the *resized*
    // image, which is not measured in source pixels at all — scaling it in place
    // would fix the crop by breaking the fill.
    let fallback = parsed_options.gravity;
    if let Some(gravity) = crop.gravity.or(fallback) {
        crop.gravity = Some(rescale_gravity(gravity, sw / ow, sh / oh));
    }
}

/// Rewrites a gravity's absolute offsets for a source decoded at a reduced size.
///
/// Only offsets of magnitude 1 or more are pixel counts; anything smaller is a
/// fraction of the axis and already scales itself. A focus point reads its two
/// arguments as coordinates in 0..1, so it is a fraction throughout and must be
/// left exactly as it is.
fn rescale_gravity(gravity: Gravity, x_scale: f64, y_scale: f64) -> Gravity {
    if gravity.kind == GravityType::FocusPoint {
        return gravity;
    }

    let scaled = |offset: f64, scale: f64| {
        if offset.abs() >= 1.0 {
            offset * scale
        } else {
            offset
        }
    };

    Gravity {
        kind: gravity.kind,
        x: scaled(gravity.x, x_scale),
        y: scaled(gravity.y, y_scale),
    }
}

/// Reopens the source at a reduced scale when the plan allows it, falling back
/// to the image already opened.
///
/// Only JPEG and WebP: their loaders genuinely skip the work, JPEG through a
/// power-of-two `shrink` and WebP through a continuous `scale`. Other loaders
/// either have no equivalent or spell it differently, and naming a property a
/// loader does not have makes libvips reject the whole call.
pub fn shrink_source_on_load(
    source_image: VipsImage,
    image_bytes: &Bytes,
    metadata_bytes: &Bytes,
    parsed_options: &mut ParsedOptions,
    base_load_options: &str,
) -> VipsImage {
    let format = sniff_image_format(image_bytes);
    if !matches!(format, Some("jpeg") | Some("webp")) {
        return source_image;
    }

    let (width, height) = (source_image.get_width(), source_image.get_height());
    let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
        return source_image;
    };

    // EXIF orientations 5-8 swap the axes, and the rotation happens after the
    // load. The plan is written against what the viewer sees, so the factor has
    // to be chosen against the rotated dimensions, not the stored ones — and
    // read from the bytes the rotation will actually come from, which for a
    // substituted thumbnail are the parent's, not the ones being decoded.
    let (width, height) = if swaps_axes(parsed_options, metadata_bytes) {
        (height, width)
    } else {
        (width, height)
    };

    // JPEG takes a power-of-two shrink; WebP takes a continuous scale and can
    // therefore decode much closer to what is actually needed.
    let load_option = if format == Some("webp") {
        load_scale_factor(parsed_options, width, height).map(|scale| format!("scale={scale}"))
    } else {
        match load_shrink_factor(parsed_options, width, height) {
            factor if factor > 1 => Some(format!("shrink={factor}")),
            _ => None,
        }
    };
    let Some(load_option) = load_option else {
        return source_image;
    };

    let load_options = if base_load_options.is_empty() {
        load_option
    } else {
        format!("{base_load_options},{load_option}")
    };

    match VipsImage::new_from_buffer(image_bytes, &load_options) {
        Ok(shrunk) => {
            debug!(
                "Decoding with {}: {}x{} -> {}x{}",
                load_options,
                width,
                height,
                shrunk.get_width(),
                shrunk.get_height()
            );
            // A crop names a region of the source in pixels, and the source
            // just got smaller. Rewrite it against what was actually decoded
            // rather than the requested factor, so rounding in the loader
            // cannot leave the region pointing at the wrong pixels.
            rescale_crop(
                parsed_options,
                (source_image.get_width(), source_image.get_height()),
                (shrunk.get_width(), shrunk.get_height()),
            );
            shrunk
        }
        // Nothing is lost by carrying on with the image already opened.
        Err(err) => {
            debug!("Shrink-on-load unavailable, using the full-size source: {}", err);
            source_image
        }
    }
}

/// Whether a channel count implies an alpha channel.
pub fn image_has_alpha(channels: u32) -> bool {
    matches!(channels, 2 | 4)
}
