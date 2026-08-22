//! Tone mapping: recolouring an image from its luminance.
//!
//! Monochrome, duotone and colorize are all the same shape — derive a colour
//! per pixel, then blend it over the original by an intensity — so they share
//! the blend and differ only in how the colour is derived. Keeping them
//! together is what makes that obvious; scattered across `effects` they would
//! read as three unrelated filters.

use super::{vips, TransformError};
use crate::processing::options::{Colorize, Duotone, Monochrome};
use libvips::{ops, VipsImage};

/// The value a fully lit channel holds, for the band format in play.
///
/// Every colour in a URL is written in 8-bit terms, but a 16-bit image spans a
/// far wider range. Blending an 8-bit constant into one of those makes
/// `colorize:1:ff0000` come out very nearly black instead of red, so the
/// constants are scaled to the range they are being mixed into.
///
/// Signed formats hold half of what their unsigned counterparts do — `Short`
/// tops out at 32767, not 65535 — and the difference is not cosmetic. Scaling
/// mid-grey against the wrong ceiling put `colorize:1:808080` at roughly 32896,
/// which the cast back clipped to 32767: mid-grey arrived as white, and a
/// duotone's shadow offset clipped the same way.
fn channel_ceiling(img: &VipsImage) -> f64 {
    match img.get_format() {
        Ok(ops::BandFormat::Ushort) => 65535.0,
        Ok(ops::BandFormat::Short) => 32767.0,
        Ok(ops::BandFormat::Uint) => 4294967295.0,
        Ok(ops::BandFormat::Int) => 2147483647.0,
        // Float formats are conventionally 0-255 in libvips' sRGB space, which
        // is also what the 8-bit default covers.
        _ => 255.0,
    }
}

/// Scales an 8-bit colour component into the image's own range.
fn component(value: u8, ceiling: f64) -> f64 {
    f64::from(value) / 255.0 * ceiling
}

/// Splits an image into its colour bands and its alpha, if it has one.
///
/// Every operation here recolours the visible channels and must leave alpha
/// exactly as it found it: folding a tint into opacity would fade the image in
/// or out rather than colour it.
fn split_alpha(img: &VipsImage) -> Result<(VipsImage, Option<VipsImage>), TransformError> {
    if !img.image_hasalpha() {
        return Ok((ops::copy(img).map_err(vips("Error copying image"))?, None));
    }

    let bands = img.get_bands();
    let colour = ops::extract_band_with_opts(img, 0, &ops::ExtractBandOptions { n: bands - 1 })
        .map_err(vips("Error separating colour from alpha"))?;
    let alpha = ops::extract_band(img, bands - 1).map_err(vips("Error extracting alpha"))?;

    Ok((colour, Some(alpha)))
}

fn rejoin_alpha(colour: VipsImage, alpha: Option<VipsImage>) -> Result<VipsImage, TransformError> {
    let Some(alpha) = alpha else {
        return Ok(colour);
    };
    ops::bandjoin(&mut [colour, alpha]).map_err(vips("Error restoring alpha"))
}

/// A single-band luminance image expanded back to three bands.
///
/// `colourspace` to BW does the perceptual weighting properly, which a hand
/// written matrix would only approximate.
fn luminance_bands(colour: &VipsImage) -> Result<VipsImage, TransformError> {
    let grey = ops::colourspace(colour, ops::Interpretation::BW).map_err(vips("Error deriving luminance"))?;
    let grey = ops::extract_band(&grey, 0).map_err(vips("Error deriving luminance"))?;
    ops::bandjoin(&mut [
        ops::copy(&grey).map_err(vips("Error deriving luminance"))?,
        ops::copy(&grey).map_err(vips("Error deriving luminance"))?,
        grey,
    ])
    .map_err(vips("Error deriving luminance"))
}

/// Blends `tinted` over `base` by `intensity`, and casts back to the band
/// format the original arrived in.
fn blend(base: &VipsImage, tinted: &VipsImage, intensity: f64) -> Result<VipsImage, TransformError> {
    let format = base.get_format();

    let kept = ops::linear(base, &mut [1.0 - intensity; 3], &mut [0.0; 3]).map_err(vips("Error blending tone"))?;
    let applied = ops::linear(tinted, &mut [intensity; 3], &mut [0.0; 3]).map_err(vips("Error blending tone"))?;
    let blended = ops::add(&kept, &applied).map_err(vips("Error blending tone"))?;

    // `linear` and `add` both promote to float to hold intermediate values;
    // casting back clips them and keeps the encoder on the source's format.
    match format {
        Ok(format) => ops::cast(&blended, format).map_err(vips("Error blending tone")),
        Err(_) => Ok(blended),
    }
}

/// Converts an image to a single-hue palette built from `color`.
///
/// The luminance decides how much of the colour each pixel gets, so the result
/// keeps the original's tonal structure and loses only its hue.
pub fn apply_monochrome(img: VipsImage, monochrome: Monochrome) -> Result<VipsImage, TransformError> {
    if monochrome.intensity <= 0.0 {
        return Ok(img);
    }

    let (colour, alpha) = split_alpha(&img)?;
    let grey = luminance_bands(&colour)?;

    // Scale each channel by its share of the base colour: a pixel at full
    // luminance becomes the colour itself, and black stays black. This one is
    // a ratio rather than a constant, so it needs no range scaling.
    let mut scale = channel_scale(monochrome.color);
    let tinted = ops::linear(&grey, &mut scale, &mut [0.0; 3]).map_err(vips("Error applying monochrome"))?;

    rejoin_alpha(blend(&colour, &tinted, monochrome.intensity)?, alpha)
}

/// Maps the tonal range between two colours: shadows toward the first,
/// highlights toward the second.
pub fn apply_duotone(img: VipsImage, duotone: Duotone) -> Result<VipsImage, TransformError> {
    if duotone.intensity <= 0.0 {
        return Ok(img);
    }

    let (colour, alpha) = split_alpha(&img)?;
    let grey = luminance_bands(&colour)?;

    // A linear interpolation from shadow colour to highlight colour, expressed
    // as the gradient and the offset `vips_linear` already computes.
    let ceiling = channel_ceiling(&colour);
    let shadow = duotone.shadow;
    let highlight = duotone.highlight;
    // The gradient is a ratio of the image's own range, so it stays in 0..1;
    // the offset is an absolute value and has to be scaled into that range.
    let mut gradient = [
        (f64::from(highlight[0]) - f64::from(shadow[0])) / 255.0,
        (f64::from(highlight[1]) - f64::from(shadow[1])) / 255.0,
        (f64::from(highlight[2]) - f64::from(shadow[2])) / 255.0,
    ];
    let mut offset = [
        component(shadow[0], ceiling),
        component(shadow[1], ceiling),
        component(shadow[2], ceiling),
    ];
    let tinted = ops::linear(&grey, &mut gradient, &mut offset).map_err(vips("Error applying duotone"))?;

    rejoin_alpha(blend(&colour, &tinted, duotone.intensity)?, alpha)
}

/// Washes a flat colour over the image.
///
/// Unlike the two above this ignores luminance entirely, which is the whole
/// point: it is a wash, not a remapping, and reduces to one pass over the
/// pixels.
pub fn apply_colorize(img: VipsImage, colorize: Colorize) -> Result<VipsImage, TransformError> {
    if colorize.opacity <= 0.0 {
        return Ok(img);
    }

    let (colour, alpha) = split_alpha(&img)?;
    let format = colour.get_format();

    let ceiling = channel_ceiling(&colour);
    let opacity = colorize.opacity;
    let mut multipliers = [1.0 - opacity; 3];
    let mut adders = [
        component(colorize.color[0], ceiling) * opacity,
        component(colorize.color[1], ceiling) * opacity,
        component(colorize.color[2], ceiling) * opacity,
    ];
    let washed = ops::linear(&colour, &mut multipliers, &mut adders).map_err(vips("Error applying colorize"))?;
    let washed = match format {
        Ok(format) => ops::cast(&washed, format).map_err(vips("Error applying colorize"))?,
        Err(_) => washed,
    };

    // `keep_alpha:true` leaves transparency exactly as it was. `false` asks for
    // the wash to reach it as well — which means blending the alpha toward the
    // colour's own by the same opacity, not discarding the band. Dropping it
    // made every pixel fully opaque no matter how faint the wash, so
    // `colorize:0.01:ff0000` turned an invisible pixel solid instead of nudging
    // it one percent of the way.
    if colorize.keep_alpha {
        return rejoin_alpha(washed, alpha);
    }
    let Some(alpha) = alpha else {
        return Ok(washed);
    };

    let mut alpha_multiplier = [1.0 - opacity];
    let mut alpha_adder = [component(colorize.color[3], ceiling) * opacity];
    let blended_alpha =
        ops::linear(&alpha, &mut alpha_multiplier, &mut alpha_adder).map_err(vips("Error applying colorize"))?;
    let blended_alpha = match format {
        Ok(format) => ops::cast(&blended_alpha, format).map_err(vips("Error applying colorize"))?,
        Err(_) => blended_alpha,
    };

    rejoin_alpha(washed, Some(blended_alpha))
}

fn channel_scale(color: [u8; 4]) -> [f64; 3] {
    [
        f64::from(color[0]) / 255.0,
        f64::from(color[1]) / 255.0,
        f64::from(color[2]) / 255.0,
    ]
}
