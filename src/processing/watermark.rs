use crate::processing::options::{Gravity, Watermark, WatermarkPosition};
use crate::processing::transform::{calc_position, resize_with_algorithm, TransformError};
use bytes::Bytes;
use libvips::{ops, VipsImage};
use thiserror::Error;

/// Errors produced while loading, preparing, or applying a watermark.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WatermarkError {
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error("{operation}: {source}")]
    Vips {
        operation: &'static str,
        #[source]
        source: libvips::error::Error,
    },
}

fn vips(operation: &'static str) -> impl FnOnce(libvips::error::Error) -> WatermarkError {
    move |source| WatermarkError::Vips { operation, source }
}

#[derive(Clone)]
pub struct PreparedWatermark {
    bytes: Bytes,
    width: i32,
    height: i32,
    bands: i32,
    format: ops::BandFormat,
    interpretation: ops::Interpretation,
    xres: f64,
    yres: f64,
}

impl PreparedWatermark {
    fn to_image(&self) -> Result<VipsImage, WatermarkError> {
        let raw = VipsImage::new_from_memory(&self.bytes, self.width, self.height, self.bands, self.format)
            .map_err(vips("Failed to load watermark from prepared bytes"))?;

        // new_from_memory yields a header-less raw image (interpretation
        // MULTIBAND), which vips_composite2 refuses to blend with an sRGB
        // base. Restore the full header captured at decode time; every
        // CopyOptions field must be set because copy applies them all.
        ops::copy_with_opts(
            &raw,
            &ops::CopyOptions {
                width: self.width,
                height: self.height,
                bands: self.bands,
                format: self.format,
                coding: ops::Coding::None,
                interpretation: self.interpretation,
                xres: self.xres,
                yres: self.yres,
                xoffset: 0,
                yoffset: 0,
            },
        )
        .map_err(vips("Failed to restore watermark image header"))
    }
}

#[derive(Clone)]
pub struct CachedWatermark {
    pub bytes: Bytes,
    pub prepared_rgba: Option<PreparedWatermark>,
}

impl CachedWatermark {
    pub fn from_bytes(bytes: Bytes) -> Self {
        Self {
            bytes,
            prepared_rgba: None,
        }
    }

    pub fn from_prepared(bytes: Bytes, prepared_rgba: PreparedWatermark) -> Self {
        Self {
            bytes,
            prepared_rgba: Some(prepared_rgba),
        }
    }
}

pub fn load_watermark_image(watermark_bytes: &[u8]) -> Result<VipsImage, WatermarkError> {
    let watermark_img =
        VipsImage::new_from_buffer(watermark_bytes, "").map_err(vips("Failed to load watermark image from buffer"))?;
    ensure_alpha_channel(watermark_img)
}

pub fn prepare_cached_watermark(bytes: Bytes) -> Result<CachedWatermark, WatermarkError> {
    let watermark_img = load_watermark_image(bytes.as_ref())?;
    let prepared_rgba = build_prepared_watermark_image(watermark_img)?;
    Ok(CachedWatermark::from_prepared(bytes, prepared_rgba))
}

/// The fraction of the image width a watermark covers when the request does not
/// ask for a size.
///
/// imgproxy leaves an unscaled watermark at its natural pixel size, which makes
/// the same watermark dominate a thumbnail and vanish on a large render.
/// imgforge has always sized it relative to the result instead, and keeps doing
/// so; `scale` overrides it with imgproxy's own meaning.
const DEFAULT_WATERMARK_WIDTH_FRACTION: f64 = 0.25;

/// Applies a watermark to an image.
pub fn apply_watermark(
    img: VipsImage,
    watermark: &CachedWatermark,
    watermark_opts: &Watermark,
    resizing_algorithm: Option<&str>,
) -> Result<VipsImage, WatermarkError> {
    let watermark_img = resolve_watermark_image(watermark)?;
    if watermark_img.get_width() <= 0 || watermark_img.get_height() <= 0 {
        return Ok(img);
    }

    let fraction = if watermark_opts.scale > 0.0 {
        watermark_opts.scale
    } else {
        DEFAULT_WATERMARK_WIDTH_FRACTION
    };
    let factor = (f64::from(img.get_width()) * fraction) / f64::from(watermark_img.get_width());
    let watermark_resized = resize_with_algorithm(
        &watermark_img,
        factor,
        None,
        resizing_algorithm,
        "Failed to resize watermark",
    )?;

    // Add alpha channel to watermark if it doesn't have one
    let watermark_with_alpha = ensure_alpha_channel(watermark_resized)?;

    // Apply opacity
    let multipliers = &mut [1.0, 1.0, 1.0, f64::from(watermark_opts.opacity)];
    let adders = &mut [0.0, 0.0, 0.0, 0.0];
    let watermark_with_opacity = ops::linear(&watermark_with_alpha, multipliers, adders)
        .map_err(vips("Failed to apply opacity to watermark"))?;

    let watermark_on_canvas = place_watermark(&img, &watermark_with_opacity, watermark_opts)?;

    ops::composite_2(&img, &watermark_on_canvas, ops::BlendMode::Over).map_err(vips("Failed to composite watermark"))
}

/// Builds a full-size canvas holding the watermark where the request wants it.
fn place_watermark(img: &VipsImage, watermark: &VipsImage, options: &Watermark) -> Result<VipsImage, WatermarkError> {
    let (canvas_w, canvas_h) = (img.get_width(), img.get_height());

    match options.position {
        WatermarkPosition::Replicate => tile_watermark(watermark, canvas_w, canvas_h),
        WatermarkPosition::Anchor(kind) => {
            let gravity = Gravity {
                kind,
                x: options.x_offset,
                y: options.y_offset,
            };
            // Overflow is allowed so an offset can push part of the watermark
            // off the edge, which is what a caller asking for a bleed wants.
            let (x, y) = calc_position(
                i64::from(canvas_w),
                i64::from(canvas_h),
                i64::from(watermark.get_width()),
                i64::from(watermark.get_height()),
                &gravity,
                1.0,
                true,
            );

            let embed_options = ops::EmbedOptions {
                extend: ops::Extend::Background,
                background: vec![0.0, 0.0, 0.0, 0.0],
            };
            ops::embed_with_opts(watermark, x as i32, y as i32, canvas_w, canvas_h, &embed_options)
                .map_err(vips("Failed to embed watermark on canvas"))
        }
    }
}

/// Tiles the watermark across the whole image, for `re` positioning.
fn tile_watermark(watermark: &VipsImage, canvas_w: i32, canvas_h: i32) -> Result<VipsImage, WatermarkError> {
    let (wm_w, wm_h) = (watermark.get_width().max(1), watermark.get_height().max(1));
    let across = tiles_needed(canvas_w, wm_w);
    let down = tiles_needed(canvas_h, wm_h);

    let tiled = ops::replicate(watermark, across, down).map_err(vips("Failed to tile watermark"))?;
    ops::extract_area(&tiled, 0, 0, canvas_w, canvas_h).map_err(vips("Failed to trim tiled watermark"))
}

/// How many tiles of `tile` it takes to cover `extent`, rounding up.
fn tiles_needed(extent: i32, tile: i32) -> i32 {
    ((extent + tile - 1) / tile).max(1)
}

fn resolve_watermark_image(watermark: &CachedWatermark) -> Result<VipsImage, WatermarkError> {
    if let Some(prepared_rgba) = &watermark.prepared_rgba {
        return prepared_rgba.to_image();
    }

    load_watermark_image(watermark.bytes.as_ref())
}

fn ensure_alpha_channel(watermark_img: VipsImage) -> Result<VipsImage, WatermarkError> {
    if watermark_img.get_bands() == 4 || watermark_img.get_bands() == 2 {
        return Ok(watermark_img);
    }

    ops::bandjoin_const(&watermark_img, &mut [255.0]).map_err(vips("Failed to add alpha to watermark"))
}

fn build_prepared_watermark_image(watermark_img: VipsImage) -> Result<PreparedWatermark, WatermarkError> {
    let format = watermark_img
        .get_format()
        .map_err(vips("Failed to determine watermark format"))?;
    let interpretation = watermark_img
        .get_interpretation()
        .map_err(vips("Failed to determine watermark interpretation"))?;
    let prepared = PreparedWatermark {
        bytes: Bytes::from(watermark_img.image_write_to_memory()),
        width: watermark_img.get_width(),
        height: watermark_img.get_height(),
        bands: watermark_img.get_bands(),
        format,
        interpretation,
        xres: watermark_img.get_xres(),
        yres: watermark_img.get_yres(),
    };

    Ok(prepared)
}
