//! Output options: what the encoder is told, and what metadata survives.

use super::error::{arg, parse_integer, parse_optional_bool, parse_quality, OptionParseError};
use std::collections::HashMap;

/// Encoder-specific output controls.
#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    pub format_quality: HashMap<String, u8>,
    pub max_bytes: Option<usize>,
    pub strip_metadata: Option<bool>,
    pub strip_color_profile: Option<bool>,
    /// Retain the copyright tags when metadata is stripped.
    pub keep_copyright: Option<bool>,
    /// Keep a high bit-depth image high bit-depth, and carry any gain map
    /// through to the result.
    pub preserve_hdr: Option<bool>,
    pub jpeg: JpegOptions,
    pub png: PngOptions,
    pub webp: WebpOptions,
    pub avif: AvifOptions,
}

impl SaveOptions {
    /// Whether the request asked for any metadata to be dropped.
    pub fn strips_anything(&self) -> bool {
        self.strip_metadata.unwrap_or(false) || self.strip_color_profile.unwrap_or(false)
    }

    /// Whether the copyright tags must be carried across a metadata strip.
    pub fn retains_copyright(&self) -> bool {
        self.keep_copyright.unwrap_or(false) && self.strip_metadata.unwrap_or(false)
    }
}

/// JPEG encoder controls.
#[derive(Debug, Clone, Default)]
pub struct JpegOptions {
    pub progressive: Option<bool>,
    pub no_subsample: Option<bool>,
    pub trellis_quant: Option<bool>,
    pub overshoot_deringing: Option<bool>,
    pub optimize_scans: Option<bool>,
    pub quant_table: Option<i32>,
}

/// PNG encoder controls.
#[derive(Debug, Clone, Default)]
pub struct PngOptions {
    pub interlaced: Option<bool>,
    pub quantize: Option<bool>,
    pub quantization_colors: Option<u16>,
}

/// WebP encoder controls.
#[derive(Debug, Clone, Default)]
pub struct WebpOptions {
    pub lossless: Option<bool>,
    pub smart_subsample: Option<bool>,
    pub preset: Option<String>,
}

/// AVIF/HEIF encoder controls.
#[derive(Debug, Clone, Default)]
pub struct AvifOptions {
    pub no_subsample: Option<bool>,
}

pub(super) fn parse_format_quality(args: &[String], save: &mut SaveOptions) -> Result<(), OptionParseError> {
    if args.len() < 2 || !args.len().is_multiple_of(2) {
        return Err(OptionParseError::invalid(
            "format_quality option requires format/quality pairs",
        ));
    }

    for pair in args.chunks_exact(2) {
        // Stored under the canonical name, because that is what the lookup uses.
        // The output format is canonicalised before it reaches the encoder, so a
        // key left as the URL spelled it — `format:tif/format_quality:tif:20` —
        // was looked up as `tiff`, missed, and silently fell back to the default
        // quality. An unrecognised name is kept as written: it names no format
        // imgforge can encode, so it can only ever miss, and rewriting it would
        // hide the typo rather than leave it visible in a debug log.
        let name = pair[0].to_lowercase();
        let key = crate::processing::save::canonical_format_name(&name)
            .map(str::to_owned)
            .unwrap_or(name);
        save.format_quality
            .insert(key, parse_quality(&pair[1], "format_quality")?);
    }

    Ok(())
}

pub(super) fn parse_jpeg_options(args: &[String], jpeg: &mut JpegOptions) -> Result<(), OptionParseError> {
    jpeg.progressive = parse_optional_bool(args, 0);
    jpeg.no_subsample = parse_optional_bool(args, 1);
    jpeg.trellis_quant = parse_optional_bool(args, 2);
    jpeg.overshoot_deringing = parse_optional_bool(args, 3);
    jpeg.optimize_scans = parse_optional_bool(args, 4);
    if let Some(value) = arg(args, 5) {
        jpeg.quant_table = Some(parse_integer(value, "jpeg quant_table")?);
    }
    Ok(())
}

pub(super) fn parse_png_options(args: &[String], png: &mut PngOptions) -> Result<(), OptionParseError> {
    png.interlaced = parse_optional_bool(args, 0);
    png.quantize = parse_optional_bool(args, 1);
    if let Some(value) = arg(args, 2) {
        png.quantization_colors = Some(parse_integer(value, "png quantization_colors")?);
    }
    Ok(())
}

pub(super) fn parse_webp_options(args: &[String], webp: &mut WebpOptions) {
    webp.lossless = parse_optional_bool(args, 0);
    webp.smart_subsample = parse_optional_bool(args, 1);
    if let Some(value) = arg(args, 2) {
        webp.preset = Some(value.to_lowercase());
    }
}
