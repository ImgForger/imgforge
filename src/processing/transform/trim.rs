//! Removing a uniform border.

use super::{vips, TransformError};
use crate::processing::options::Trim;
use libvips::{ops, VipsImage};
use tracing::debug;

/// How many components `find_trim` expects in a background colour: one per
/// band, less the alpha if there is one. libvips accepts a single value or
/// exactly that many, and rejects anything else — three components against a
/// CMYK image fails with "vector must have 1 or 4 elements".
fn background_components(img: &VipsImage) -> usize {
    let bands = usize::try_from(img.get_bands()).unwrap_or(1).max(1);
    if img.image_hasalpha() {
        bands.saturating_sub(1).max(1)
    } else {
        bands
    }
}

/// Reads the top-left pixel, to use as the background when the request does not
/// name one. imgproxy works this out from the image the same way; libvips on its
/// own would assume white, which never trims a dark border.
///
/// Averaging a one-pixel band reads its value whatever the band format, so this
/// works on 16-bit sources as well as 8-bit — interpreting raw memory would
/// have meant knowing the layout of each format.
fn corner_pixel(img: &VipsImage, components: usize) -> Option<Vec<f64>> {
    let corner = ops::extract_area(img, 0, 0, 1, 1).ok()?;
    (0..components)
        .map(|band| {
            let band = ops::extract_band(&corner, i32::try_from(band).ok()?).ok()?;
            ops::avg(&band).ok()
        })
        .collect()
}

/// Trims a uniform border.
///
/// Note for callers: the trimmed size is not knowable in advance, which is why
/// scale-on-load steps aside when this is in play — there is no way to choose a
/// decode scale against an unknown result.
pub fn apply_trim(img: VipsImage, trim: &Trim) -> Result<VipsImage, TransformError> {
    let components = background_components(&img);
    let background = match trim.color {
        // An explicit colour arrives as sRGB, which only lines up with a
        // three-component image. Greyscale takes its luminance; anything else —
        // CMYK, say — has no meaningful conversion, and guessing would trim the
        // wrong thing silently.
        Some(color) if components == 3 => vec![f64::from(color[0]), f64::from(color[1]), f64::from(color[2])],
        Some(color) if components == 1 => {
            vec![0.299 * f64::from(color[0]) + 0.587 * f64::from(color[1]) + 0.114 * f64::from(color[2])]
        }
        Some(_) => {
            return Err(TransformError::invalid(
                "trim",
                format!(
                    "trim colour cannot be applied to a {components}-component image; omit it to detect the background instead"
                ),
            ))
        }
        None => corner_pixel(&img, components).unwrap_or_else(|| vec![255.0; components]),
    };

    let options = ops::FindTrimOptions {
        threshold: trim.threshold,
        background,
        line_art: false,
    };
    let (left, top, width, height) = ops::find_trim_with_opts(&img, &options).map_err(vips("Error finding trim"))?;

    // An image that is entirely background has nothing to keep. Returning it
    // untouched beats handing back an empty or one-pixel image.
    if width <= 0 || height <= 0 {
        debug!("Trim found no content to keep; leaving the image alone");
        return Ok(img);
    }

    let (src_width, src_height) = (img.get_width(), img.get_height());
    let (mut left, mut top, mut width, mut height) = (left, top, width, height);

    // "Equal" means the same amount comes off both sides, so the subject keeps
    // its position rather than shifting toward whichever border was thicker.
    if trim.equal_hor {
        let margin = left.min(src_width - (left + width));
        left = margin;
        width = src_width - 2 * margin;
    }
    if trim.equal_ver {
        let margin = top.min(src_height - (top + height));
        top = margin;
        height = src_height - 2 * margin;
    }

    debug!("Trimming to {}x{} at ({}, {})", width, height, left, top);
    ops::extract_area(&img, left, top, width, height).map_err(vips("Error trimming image"))
}
