//! Choosing a reduced decode scale.
//!
//! A loader that can decode at a fraction of full size skips the work rather
//! than doing it and throwing the result away, which is the difference between
//! unpacking a 9000x7000 source and unpacking what a 450px result needs.

use crate::processing::options::ParsedOptions;

/// The JPEG loader can decode at 1/2, 1/4 or 1/8 scale, skipping the work
/// rather than doing it and throwing the result away.
const MAX_LOAD_SHRINK: u32 = 8;

/// Below this, re-decoding at a reduced scale is not worth the divergence: at
/// 1.5 the pixel count already drops to 44%, and under it the saving thins out
/// fast.
const MIN_LOAD_SHRINK: f64 = 1.5;

/// How much larger the source is than what the request needs, as a ratio.
///
/// `None` means decode it whole: `raw` returns the source untouched, and a crop
/// addresses source pixels by coordinate, so shrinking underneath it would move
/// the region being cut.
fn load_shrink_ratio(parsed_options: &ParsedOptions, src_width: u32, src_height: u32) -> Option<f64> {
    if parsed_options.raw {
        return None;
    }
    // Trim removes an unknown number of pixels, so there is no way to tell how
    // many will be left for the resize. Choosing a decode scale against that is
    // guesswork, and guessing low leaves the resize short. imgproxy stands
    // aside here too.
    if parsed_options.trim.is_some() {
        return None;
    }
    let resize = parsed_options.resize.as_ref()?;
    if src_width == 0 || src_height == 0 {
        return None;
    }

    // A crop runs before the resize, so the pixels that have to survive are the
    // crop region, not the whole source. Measuring against the source would
    // shrink past what the crop still needs: an 8000x6000 source cropped to
    // 2000x1500 and resized to 500 wide can only lose a factor of 4, not 16.
    let (available_width, available_height) = match parsed_options.crop.as_ref() {
        Some(crop) => {
            let (crop_width, crop_height) = crop.resolve(src_width, src_height);
            (
                if crop_width == 0 {
                    src_width
                } else {
                    crop_width.min(src_width)
                },
                if crop_height == 0 {
                    src_height
                } else {
                    crop_height.min(src_height)
                },
            )
        }
        None => (src_width, src_height),
    };

    // Anything that can grow the target after this point has to be folded in,
    // or the shrink could drop the source below what the pipeline still needs.
    let grow = f64::from(parsed_options.dpr_factor()) * f64::from(parsed_options.zoom_factors().max_factor());

    // `force` fills a zero axis from the *source* dimension, so that axis needs
    // the source at full size. Every other type derives a zero axis from the
    // aspect ratio, which survives a shrink unchanged.
    let forced = resize.resizing_type.fills_zero_axis_from_source();
    let target_width = if forced && resize.width == 0 {
        f64::from(available_width)
    } else {
        (f64::from(resize.width) * grow).max(f64::from(parsed_options.min_width.unwrap_or(0)))
    };
    let target_height = if forced && resize.height == 0 {
        f64::from(available_height)
    } else {
        (f64::from(resize.height) * grow).max(f64::from(parsed_options.min_height.unwrap_or(0)))
    };

    // The *least* shrink any axis needs, so the decoded image is still at least
    // as large as the target on both. Overshooting would hand the pipeline a
    // source smaller than the request, which `enlarge:false` then refuses to
    // scale back up.
    let mut ratio = f64::INFINITY;
    if target_width >= 1.0 {
        ratio = ratio.min(f64::from(available_width) / target_width);
    }
    if target_height >= 1.0 {
        ratio = ratio.min(f64::from(available_height) / target_height);
    }
    (ratio.is_finite() && ratio >= MIN_LOAD_SHRINK).then_some(ratio)
}

/// Power-of-two shrink for the JPEG loader, or 1 to decode at full size.
pub fn load_shrink_factor(parsed_options: &ParsedOptions, src_width: u32, src_height: u32) -> u32 {
    let Some(ratio) = load_shrink_ratio(parsed_options, src_width, src_height) else {
        return 1;
    };

    let mut factor = 1;
    while factor * 2 <= MAX_LOAD_SHRINK && f64::from(factor * 2) <= ratio {
        factor *= 2;
    }
    factor
}

/// Continuous scale for the WebP loader, or `None` to decode at full size.
///
/// WebP takes a scale rather than JPEG's power-of-two shrink, so it can decode
/// much closer to what is needed — a request needing a 3x reduction gets one,
/// where the JPEG path has to settle for 2x.
///
/// The loader rounds decoded dimensions to nearest and can round down — 4000 x
/// 0.3333 is 1333.2 and decodes to 1333 — so an undershoot would be possible
/// with a scale that had been truncated on its way in. Deriving it exactly from
/// the target avoids that: the multiplication lands back on the target and the
/// rounding has nothing to shave. Checked over several million source/target
/// pairs, and guarded by a test that decodes real WebP data rather than
/// modelling the rounding.
pub fn load_scale_factor(parsed_options: &ParsedOptions, src_width: u32, src_height: u32) -> Option<f64> {
    let scale = 1.0 / load_shrink_ratio(parsed_options, src_width, src_height)?;
    (scale > 0.0 && scale < 1.0).then_some(scale)
}
