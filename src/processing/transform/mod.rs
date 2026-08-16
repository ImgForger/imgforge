//! Pixel transformations.
//!
//! Each stage of the pipeline lives in its own module: [`geometry`] positions
//! and sizes the canvas, [`resize`] scales, [`effects`] changes colour, and
//! [`orientation`] and [`trim`] handle the two operations that depend on what
//! the source itself carries.

pub mod effects;
pub mod geometry;
pub mod orientation;
pub mod resize;
pub mod tone;
pub mod trim;

use libvips::{ops, VipsImage};
use thiserror::Error;

pub use effects::{
    apply_adjust, apply_background_color, apply_blur, apply_pixelate, apply_sharpen, flatten_onto_background,
};
pub use geometry::{apply_padding, calc_position, crop_image, extend_image, extend_to_aspect_ratio, smart_crop};
pub use orientation::{apply_exif_orientation, apply_exif_rotation, apply_flip, apply_rotation};
pub use resize::{apply_min_dimensions, apply_resize, apply_zoom, resolve_resize_dimensions};
pub use tone::{apply_colorize, apply_duotone, apply_monochrome};
pub use trim::apply_trim;

/// Scales below this differ from 1 by less than a pixel on any plausible image,
/// so the resize is skipped rather than run for nothing.
pub(crate) const SCALE_EPSILON: f64 = 1e-6;

/// Largest coordinate libvips accepts for `embed`; anything beyond it is
/// rejected by the operation itself.
pub(crate) const VIPS_MAX_COORD: i64 = 1_000_000_000;

/// Errors produced while transforming an image.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransformError {
    #[error("{operation}: {source}")]
    Vips {
        operation: &'static str,
        #[source]
        source: libvips::error::Error,
    },
    #[error("{message}")]
    InvalidArgument { operation: &'static str, message: String },
}

impl TransformError {
    pub(crate) fn invalid(operation: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidArgument {
            operation,
            message: message.into(),
        }
    }
}

pub(crate) fn vips(operation: &'static str) -> impl FnOnce(libvips::error::Error) -> TransformError {
    move |source| TransformError::Vips { operation, source }
}

/// Converts a resizing algorithm string to a libvips Kernel enum.
pub(crate) fn get_resize_kernel(algorithm: Option<&str>) -> ops::Kernel {
    match algorithm.unwrap_or("lanczos3") {
        "nearest" => ops::Kernel::Nearest,
        "linear" => ops::Kernel::Linear,
        "cubic" => ops::Kernel::Cubic,
        "lanczos2" => ops::Kernel::Lanczos2,
        "lanczos3" => ops::Kernel::Lanczos3,
        _ => ops::Kernel::Lanczos3, // Default to lanczos3
    }
}

/// Expands an RGBA background into the component vector a given band count
/// needs, collapsing to luminance for greyscale.
pub(crate) fn bg_color_for_bands(bg_color: [u8; 4], bands: i32) -> Vec<f64> {
    let luma = (0.299 * bg_color[0] as f64 + 0.587 * bg_color[1] as f64 + 0.114 * bg_color[2] as f64).round();
    match bands {
        4 => vec![
            bg_color[0] as f64,
            bg_color[1] as f64,
            bg_color[2] as f64,
            bg_color[3] as f64,
        ],
        3 => vec![bg_color[0] as f64, bg_color[1] as f64, bg_color[2] as f64],
        2 => vec![luma, bg_color[3] as f64],
        1 => vec![luma],
        _ => vec![bg_color[0] as f64, bg_color[1] as f64, bg_color[2] as f64],
    }
}

/// Helper to resize using the requested algorithm, defaulting to lanczos3.
///
/// Images carrying alpha are premultiplied for the duration of the scale.
/// libvips is explicit that `vips_resize` does not do this itself — "if your
/// image has an alpha channel, you should use vips_premultiply() on it first" —
/// and without it the kernel averages the colour of fully transparent pixels
/// into visible ones. Downscaling white-on-transparent that way drags the edge
/// toward whatever colour happens to sit in the invisible pixels, which shows up
/// as a dark halo around logos and cutouts once the result is composited.
pub fn resize_with_algorithm(
    img: &VipsImage,
    hscale: f64,
    vscale: Option<f64>,
    resizing_algorithm: Option<&str>,
    error_context: &'static str,
) -> Result<VipsImage, TransformError> {
    let options = ops::ResizeOptions {
        kernel: get_resize_kernel(resizing_algorithm),
        vscale: vscale.unwrap_or(hscale),
        ..Default::default()
    };

    if !img.image_hasalpha() {
        return ops::resize_with_opts(img, hscale, &options).map_err(vips(error_context));
    }

    // A source whose format vips cannot report is not one to guess at; fall
    // back to the plain resize rather than casting to something invented.
    let Ok(source_format) = img.get_format() else {
        return ops::resize_with_opts(img, hscale, &options).map_err(vips(error_context));
    };
    let premultiplied = ops::premultiply(img).map_err(vips(error_context))?;
    let resized = ops::resize_with_opts(&premultiplied, hscale, &options).map_err(vips(error_context))?;
    let restored = ops::unpremultiply(&resized).map_err(vips(error_context))?;

    // premultiply/unpremultiply work in float; without casting back, every
    // later step and the encoder would see a float image.
    ops::cast(&restored, source_format).map_err(vips(error_context))
}
