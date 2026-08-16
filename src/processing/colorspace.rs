//! Colour management around the pipeline.
//!
//! imgforge used to process in whatever colourspace the source arrived in,
//! which is wrong in two ways. A CMYK source has no meaningful red, green and
//! blue channels, so every operation that weights them — saturation, the
//! background flatten, the trim's luminance fallback — produced nonsense. And
//! a source tagged with a wide-gamut ICC profile had its numbers treated as if
//! they were sRGB, so the result came out over-saturated once a viewer applied
//! the profile that was no longer there.
//!
//! Both are fixed by converting into a known space before processing and back
//! out at the end, which is what imgproxy's `colorspaceToProcessing` and
//! `colorspaceToResult` do.

use crate::processing::transform::{vips, TransformError};
use libvips::{ops, VipsImage};
use tracing::debug;

/// The colourspace a frame is processed in.
///
/// 16-bit sources keep their depth when the output format can carry it and the
/// request asked to preserve it; everything else lands in 8-bit sRGB, which any
/// encoder can represent.
fn processing_interpretation(img: &VipsImage, keep_high_bit_depth: bool) -> ops::Interpretation {
    let Ok(interpretation) = img.guess_interpretation() else {
        return ops::Interpretation::Srgb;
    };

    match interpretation {
        // Already somewhere the pipeline understands.
        ops::Interpretation::Srgb | ops::Interpretation::Rgb | ops::Interpretation::BW => interpretation,
        ops::Interpretation::Rgb16 if keep_high_bit_depth => interpretation,
        ops::Interpretation::Grey16 if keep_high_bit_depth => interpretation,
        ops::Interpretation::Rgb16 => ops::Interpretation::Srgb,
        ops::Interpretation::Grey16 => ops::Interpretation::BW,
        // CMYK, Lab, HSV, and the rest: sRGB can be produced from any of them.
        _ if keep_high_bit_depth => ops::Interpretation::Rgb16,
        _ => ops::Interpretation::Srgb,
    }
}

/// Converts a frame into the colourspace the pipeline works in.
///
/// When the source carries an embedded ICC profile and is not already in a
/// standard space, the conversion goes through that profile so the numbers mean
/// what the profile says they mean. A source without a usable profile falls
/// back to a plain colourspace conversion — `icc_transform` fails rather than
/// guessing, and a failure there is not a reason to fail the request.
pub fn to_processing(img: VipsImage, keep_high_bit_depth: bool) -> Result<VipsImage, TransformError> {
    let target = processing_interpretation(&img, keep_high_bit_depth);
    let current = img.get_interpretation().unwrap_or(ops::Interpretation::Error);

    // The interpretation enum is not evidence about colour. libvips derives it
    // from the band count and format, so every 8-bit three-band image reports as
    // `Srgb` — a Display-P3 or Adobe RGB JPEG included, with its real space
    // recorded only in the embedded profile. Deciding on the enum alone let
    // wide-gamut sources through untransformed and their numbers were then read
    // as sRGB: visibly oversaturated output from a file that says exactly what
    // it is.
    //
    // The profile cannot be detected through this crate — `VipsImage::ctx` is
    // private, so `vips_image_get_typeof` is out of reach — so the transform is
    // attempted and a failure is taken as "no usable profile". That is also what
    // imgproxy does: it imports whenever a profile is present rather than
    // consulting the interpretation, and pays the same conversion on an image
    // whose profile is already sRGB.
    let options = ops::IccTransformOptions {
        embedded: true,
        intent: ops::Intent::Relative,
        ..Default::default()
    };
    match ops::icc_transform_with_opts(&img, "srgb", &options) {
        Ok(transformed) => {
            debug!("Converted {:?} to sRGB through its embedded ICC profile", current);
            // The transform lands in sRGB; a 16-bit target still needs the final
            // hop, and for an 8-bit target this is a no-op.
            return convert_colourspace(transformed, target);
        }
        Err(err) => {
            debug!("No usable ICC profile ({}); converting {:?} directly", err, current);
        }
    }

    // Without a profile the enum is all there is, and an image already in the
    // target space needs nothing done to it.
    if same_space(current, target) {
        return Ok(img);
    }

    convert_colourspace(img, target)
}

/// Prepares a frame for the encoder.
///
/// Everything reaching this point is already in a standard space, so there is
/// nothing left to convert — the remaining question is only whether the profile
/// that came in should still be attached, and that is settled by the `keep`
/// flags handed to the encoder. This exists so the decision has one home, and
/// so a caller reading the pipeline sees where the round trip closes.
pub fn to_result(img: VipsImage, target_supports_profile: bool) -> Result<VipsImage, TransformError> {
    if target_supports_profile {
        return Ok(img);
    }

    let current = img.get_interpretation().unwrap_or(ops::Interpretation::Error);
    if matches!(current, ops::Interpretation::Srgb | ops::Interpretation::BW) {
        return Ok(img);
    }

    convert_colourspace(img, ops::Interpretation::Srgb)
}

/// `Interpretation` carries no `PartialEq`, so comparisons go through the
/// discriminant the enum is generated from.
fn same_space(left: ops::Interpretation, right: ops::Interpretation) -> bool {
    left as i32 == right as i32
}

fn convert_colourspace(img: VipsImage, target: ops::Interpretation) -> Result<VipsImage, TransformError> {
    if img
        .get_interpretation()
        .is_ok_and(|current| same_space(current, target))
    {
        return Ok(img);
    }
    ops::colourspace(&img, target).map_err(vips("Error converting colourspace"))
}
