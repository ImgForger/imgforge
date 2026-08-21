//! Multi-page and animated sources.
//!
//! libvips represents an animation as a single tall image — every frame stacked
//! vertically — with a `page-height` property saying where one frame ends and
//! the next begins. Operations that change the geometry do not update that
//! property, so scaling the stack as one image silently reinterprets four
//! 80px frames as two 160px ones.
//!
//! The way through is to take the stack apart, run each frame through the
//! ordinary pipeline, put it back together, and tell the encoder the new frame
//! height. That keeps every transformation — including the ones that rotate or
//! pad, which no amount of metadata fixing would survive — working on frames
//! rather than on a strip that happens to contain them.

use crate::processing::options::ParsedOptions;
use crate::processing::transform::{vips, TransformError};
use libvips::{ops, VipsImage};
use tracing::debug;

/// Formats whose loaders accept `page` and `n`.
///
/// Naming a property a loader does not have makes libvips reject the entire
/// call, so this list is what separates a working request from a source that
/// suddenly fails to open at all.
pub fn supports_pages(format: &str) -> bool {
    matches!(format, "gif" | "webp" | "heif" | "avif" | "tiff" | "pdf")
}

/// Formats that can carry more than one frame in the output.
pub fn supports_animation(format: &str) -> bool {
    matches!(format, "gif" | "webp" | "avif" | "heif")
}

/// What to ask the loader for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadPlan {
    /// First page to read.
    pub page: u32,
    /// How many pages to read. `None` means "all of them".
    pub count: Option<u32>,
}

impl LoadPlan {
    /// Works out which pages a request needs from a source of this format.
    ///
    /// Returns `None` when the defaults are wanted, so the common case opens
    /// the source with no loader options at all.
    pub fn resolve(options: &ParsedOptions, source_format: Option<&str>, output_format: &str) -> Option<Self> {
        let source_format = source_format?;
        if !supports_pages(source_format) {
            return None;
        }

        let page = options.page.unwrap_or(0);

        // An explicit page count wins. Otherwise an animation is read whole
        // when the result can hold it, and collapsed to its first frame when it
        // cannot — decoding frames that are about to be discarded is pure cost.
        // `disable_animation` is defined as collapsing the source to a single
        // frame, so it outranks an explicit page count rather than losing to it.
        // Letting `pages` win meant `pages:5/disable_animation:true` loaded five
        // frames and produced an animation from a request that had asked, in as
        // many words, for it not to be one. The starting `page` is still
        // honoured: which frame is a separate question from how many.
        let count = match options.pages {
            _ if options.disable_animation => Some(1),
            Some(pages) => Some(pages),
            None if supports_animation(output_format) => None,
            None => Some(1),
        };

        // A limit only matters once it is below what was going to be read.
        let count = match (count, options.max_animation_frames) {
            (Some(count), Some(limit)) => Some(count.min(limit.get())),
            (None, Some(limit)) => Some(limit.get()),
            (count, None) => count,
        };

        if page == 0 && count == Some(1) && options.max_animation_frames.is_none() {
            // What the loader would have done anyway.
            return None;
        }

        Some(Self { page, count })
    }

    /// Renders the plan as a libvips loader option string.
    pub fn as_load_options(&self) -> String {
        match self.count {
            Some(count) => format!("page={},n={}", self.page, count),
            None => format!("page={},n=-1", self.page),
        }
    }
}

/// An animated image taken apart into its frames.
pub struct Frames {
    pub images: Vec<VipsImage>,
}

/// How many frames an opened image holds, and how tall each one is.
///
/// Returns `None` for a still image, including one whose header claims several
/// pages but whose height does not divide into them — a stack imgforge cannot
/// take apart safely is better treated as the single image it looks like.
pub fn frame_geometry(img: &VipsImage) -> Option<(i32, i32)> {
    let pages = img.get_n_pages();
    let page_height = img.get_page_height();
    let height = img.get_height();

    if pages <= 1 || page_height <= 0 || height <= 0 {
        return None;
    }
    if page_height.checked_mul(pages) != Some(height) {
        debug!(
            "Ignoring animation: {} pages of {}px do not fill {}px",
            pages, page_height, height
        );
        return None;
    }

    Some((pages, page_height))
}

/// Splits an animated image into independent frames.
pub fn split(img: &VipsImage) -> Result<Frames, TransformError> {
    let Some((pages, page_height)) = frame_geometry(img) else {
        return Ok(Frames {
            images: vec![ops::copy(img).map_err(vips("Error copying frame"))?],
        });
    };

    let width = img.get_width();
    let images = (0..pages)
        .map(|page| {
            ops::extract_area(img, 0, page * page_height, width, page_height)
                .map_err(vips("Error extracting animation frame"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    debug!("Split animation into {} frames of {}px", pages, page_height);
    Ok(Frames { images })
}

/// Stacks processed frames back into a single image.
///
/// Returns the joined image and the height of one frame, which the encoder
/// needs in order to cut the stack up again.
pub fn join(mut frames: Vec<VipsImage>) -> Result<(VipsImage, Option<i32>), TransformError> {
    if frames.len() <= 1 {
        let single = frames
            .pop()
            .ok_or_else(|| TransformError::invalid("animation", "processing produced no frames"))?;
        return Ok((single, None));
    }

    let frame_height = frames[0].get_height();
    if frames.iter().any(|frame| frame.get_height() != frame_height) {
        return Err(TransformError::invalid(
            "animation",
            "animation frames came out of processing at different heights",
        ));
    }

    let options = ops::ArrayjoinOptions {
        across: 1,
        ..Default::default()
    };
    let joined = ops::arrayjoin_with_opts(&mut frames, &options).map_err(vips("Error joining animation frames"))?;

    Ok((joined, Some(frame_height)))
}
