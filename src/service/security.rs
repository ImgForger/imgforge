//! Request-level limits.
//!
//! Every limit has two possible sources: the server's configuration and the URL
//! itself. The URL only wins when the server opted in with
//! `IMGFORGE_ALLOW_SECURITY_OPTIONS`, so a public deployment cannot have its
//! ceilings raised by whoever is composing the URLs.

use super::error::ServiceError;
use crate::app::AppState;
use crate::config::Config;
use crate::limits::{
    MaxAnimationFrameResolution, MaxAnimationFrames, MaxResultDimension, MaxSourceFileSize, MaxSourceResolution,
};
use crate::processing::options::ParsedOptions;
use axum::http::StatusCode;
use bytes::Bytes;
use libvips::VipsImage;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error};

/// Builds the accessor pair for one limit: request override when allowed,
/// configuration otherwise.
macro_rules! resolve_limit {
    ($name:ident, $ty:ty, $field:ident) => {
        pub fn $name(config: &Config, parsed_options: &ParsedOptions) -> Option<$ty> {
            if config.allow_security_options {
                parsed_options.$field.or(config.$field)
            } else {
                config.$field
            }
        }
    };
}

resolve_limit!(resolve_max_src_file_size, MaxSourceFileSize, max_src_file_size);
resolve_limit!(resolve_max_src_resolution, MaxSourceResolution, max_src_resolution);
resolve_limit!(resolve_max_result_dimension, MaxResultDimension, max_result_dimension);
resolve_limit!(resolve_max_animation_frames, MaxAnimationFrames, max_animation_frames);
resolve_limit!(
    resolve_max_animation_frame_resolution,
    MaxAnimationFrameResolution,
    max_animation_frame_resolution
);

/// Copies the effective limits back onto the parsed options.
///
/// The pipeline enforces the result and per-frame ceilings itself, deep inside
/// the blocking task, and it only ever sees `ParsedOptions`. Folding the
/// configured values in here means there is one resolution rule rather than a
/// second copy of it further down.
pub fn apply_effective_limits(config: &Config, parsed_options: &mut ParsedOptions) {
    parsed_options.max_result_dimension = resolve_max_result_dimension(config, parsed_options);
    parsed_options.max_animation_frames = resolve_max_animation_frames(config, parsed_options);
    parsed_options.max_animation_frame_resolution = resolve_max_animation_frame_resolution(config, parsed_options);
}

/// The same checks, for a response that hands back the source untouched.
///
/// `raw` and `skip_processing` never build a pipeline, so the decoded image the
/// resolution check wants does not exist yet. Only that one check needs it, and
/// only when a limit is configured, so the header is read on demand rather than
/// for every passthrough — opening a buffer parses the header and decodes
/// nothing, which is what makes paying for it here cheap enough to be
/// unconditional.
pub fn enforce_source_constraints(
    state: &AppState,
    parsed_options: &ParsedOptions,
    image_bytes: &Bytes,
    source_content_type: Option<&str>,
) -> Result<(), ServiceError> {
    let decoded = resolve_max_src_resolution(&state.config, parsed_options)
        .map(|_| VipsImage::new_from_buffer(image_bytes, ""))
        .transpose()
        .map_err(|source| ServiceError::SourceImageDecode { source })?;

    enforce_security_constraints(
        state,
        parsed_options,
        image_bytes,
        source_content_type,
        decoded.as_ref(),
    )
}

pub fn enforce_security_constraints(
    state: &AppState,
    parsed_options: &ParsedOptions,
    image_bytes: &Bytes,
    source_content_type: Option<&str>,
    decoded_image: Option<&VipsImage>,
) -> Result<(), ServiceError> {
    let config = &state.config;

    if let Some(max_size) = resolve_max_src_file_size(config, parsed_options) {
        if image_bytes.len() > max_size.get() {
            error!("Source image file size is too large");
            return Err(ServiceError::new(
                StatusCode::BAD_REQUEST,
                "Source image file size is too large",
            ));
        }
    }

    if let Some(allowed_types) = &config.allowed_mime_types {
        if let Some(content_type) = source_content_type {
            if !allowed_types.iter().any(|allowed| allowed == content_type) {
                error!("Source image MIME type is not allowed: {}", content_type);
                return Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "Source image MIME type is not allowed",
                ));
            }
        }
    }

    if let Some(max_res) = resolve_max_src_resolution(config, parsed_options) {
        let (w, h) = match decoded_image {
            Some(img) => (img.get_width(), img.get_height()),
            None => {
                error!("Failed to load image for resolution check");
                return Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "Failed to load image for resolution check",
                ));
            }
        };
        debug!("Image resolution: {}x{}", w, h);
        let source_pixels = checked_source_pixel_count(w, h)?;
        if source_pixels > max_res.pixels() {
            error!("Source image resolution is too large");
            return Err(ServiceError::new(
                StatusCode::BAD_REQUEST,
                "Source image resolution is too large",
            ));
        }
    }

    Ok(())
}

pub fn checked_source_pixel_count(width: i32, height: i32) -> Result<u64, ServiceError> {
    let width = u64::try_from(width)
        .map_err(|_| ServiceError::new(StatusCode::BAD_REQUEST, "Invalid source image dimensions"))?;
    let height = u64::try_from(height)
        .map_err(|_| ServiceError::new(StatusCode::BAD_REQUEST, "Invalid source image dimensions"))?;

    width
        .checked_mul(height)
        .ok_or_else(|| ServiceError::new(StatusCode::BAD_REQUEST, "Source image resolution is too large"))
}

pub fn enforce_expiration(parsed_options: &ParsedOptions) -> Result<(), ServiceError> {
    let Some(expires) = parsed_options.expires else {
        return Ok(());
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "system clock is before unix epoch"))?
        .as_secs();

    if now > expires {
        return Err(ServiceError::new(StatusCode::NOT_FOUND, "URL has expired"));
    }

    Ok(())
}
