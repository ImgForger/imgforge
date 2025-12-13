use crate::processing::options::Watermark;
use crate::processing::transform::resize_with_algorithm;
use bytes::Bytes;
use rs_vips::{
    ops,
    voption::{Setter, VOption},
    VipsImage,
};

#[derive(Clone)]
pub struct PreparedWatermark {
    bytes: Bytes,
    width: i32,
    height: i32,
    bands: i32,
    format: ops::BandFormat,
}

impl PreparedWatermark {
    fn to_image(&self) -> Result<VipsImage, String> {
        VipsImage::new_from_memory(&self.bytes, self.width, self.height, self.bands, self.format)
            .map_err(|e| format!("Failed to load watermark from prepared bytes: {}", e))
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

pub fn load_watermark_image(watermark_bytes: &[u8]) -> Result<VipsImage, String> {
    let watermark_img = VipsImage::new_from_buffer(watermark_bytes, "")
        .map_err(|e| format!("Failed to load watermark image from buffer: {}", e))?;
    ensure_alpha_channel(watermark_img)
}

pub fn prepare_cached_watermark(bytes: Bytes) -> Result<CachedWatermark, String> {
    let watermark_img = load_watermark_image(bytes.as_ref())?;
    let prepared_rgba = build_prepared_watermark_image(watermark_img)?;
    Ok(CachedWatermark::from_prepared(bytes, prepared_rgba))
}

/// Applies a watermark to an image.
pub fn apply_watermark(
    img: VipsImage,
    watermark: &CachedWatermark,
    watermark_opts: &Watermark,
    resizing_algorithm: &Option<String>,
) -> Result<VipsImage, String> {
    let watermark_img = resolve_watermark_image(watermark)?;

    // Resize watermark to be 1/4 of the main image's width, maintaining aspect ratio
    let factor = (img.get_width() as f64 / 4.0) / watermark_img.get_width() as f64;
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
    let multipliers = [1.0, 1.0, 1.0, watermark_opts.opacity as f64];
    let adders = [0.0, 0.0, 0.0, 0.0];
    let watermark_with_opacity = watermark_with_alpha
        .linear(&multipliers, &adders)
        .map_err(|e| format!("Failed to apply opacity to watermark: {}", e))?;

    // Calculate position
    let (x, y) = calculate_watermark_position(&img, &watermark_with_opacity, &watermark_opts.position);

    // Composite watermark  
    let watermark_on_canvas = watermark_with_opacity
        .embed(x as i32, y as i32, img.get_width(), img.get_height())
        .map_err(|e| format!("Failed to embed watermark on canvas: {}", e))?;

    img.composite2(&watermark_on_canvas, ops::BlendMode::Over)
        .map_err(|e| format!("Failed to composite watermark: {}", e))
}

fn resolve_watermark_image(watermark: &CachedWatermark) -> Result<VipsImage, String> {
    if let Some(prepared_rgba) = &watermark.prepared_rgba {
        return prepared_rgba.to_image();
    }

    load_watermark_image(watermark.bytes.as_ref())
}

fn ensure_alpha_channel(watermark_img: VipsImage) -> Result<VipsImage, String> {
    if watermark_img.get_bands() == 4 || watermark_img.get_bands() == 2 {
        return Ok(watermark_img);
    }

    watermark_img
        .bandjoin_const(&[255.0])
        .map_err(|e| format!("Failed to add alpha to watermark: {}", e))
}

fn build_prepared_watermark_image(watermark_img: VipsImage) -> Result<PreparedWatermark, String> {
    let format = watermark_img
        .get_format()
        .map_err(|e| format!("Failed to determine watermark format: {}", e))?;
    let prepared = PreparedWatermark {
        bytes: Bytes::from(watermark_img.write_to_memory()),
        width: watermark_img.get_width(),
        height: watermark_img.get_height(),
        bands: watermark_img.get_bands(),
        format,
    };

    Ok(prepared)
}

fn calculate_watermark_position(main_img: &VipsImage, watermark_img: &VipsImage, position: &str) -> (u32, u32) {
    let main_w = main_img.get_width() as u32;
    let main_h = main_img.get_height() as u32;
    let wm_w = watermark_img.get_width() as u32;
    let wm_h = watermark_img.get_height() as u32;
    let margin = (main_w.min(main_h) as f32 * 0.05).round() as u32; // 5% margin

    match position {
        "north" => ((main_w - wm_w) / 2, margin),
        "south" => ((main_w - wm_w) / 2, main_h - wm_h - margin),
        "east" => (main_w - wm_w - margin, (main_h - wm_h) / 2),
        "west" => (margin, (main_h - wm_h) / 2),
        "north_west" => (margin, margin),
        "north_east" => (main_w - wm_w - margin, margin),
        "south_west" => (margin, main_h - wm_h - margin),
        "south_east" => (main_w - wm_w - margin, main_h - wm_h - margin),
        "center" => ((main_w - wm_w) / 2, (main_h - wm_h) / 2),
        _ => ((main_w - wm_w) / 2, (main_h - wm_h) / 2),
    }
}
