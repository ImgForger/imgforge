//! Pixel-effect options: colour adjustment, zoom and watermarking.

use super::error::{arg, parse_float, parse_integer, parse_positive_f32, OptionParseError};
use super::geometry::GravityType;

/// Represents the parameters for colour adjustment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Adjust {
    /// Added to every colour channel, in 8-bit units.
    pub brightness: i16,
    /// Multiplier applied around mid-grey.
    pub contrast: f32,
    /// Multiplier applied to chroma.
    pub saturation: f32,
}

impl Default for Adjust {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 1.0,
            saturation: 1.0,
        }
    }
}

impl Adjust {
    /// Whether this adjustment would change any pixel.
    pub fn is_identity(&self) -> bool {
        self.brightness == 0
            && (self.contrast - 1.0).abs() <= f32::EPSILON
            && (self.saturation - 1.0).abs() <= f32::EPSILON
    }
}

pub(super) fn parse_brightness(value: &str) -> Result<i16, OptionParseError> {
    let parsed = parse_integer::<i16>(value, "brightness")?;
    if !(-255..=255).contains(&parsed) {
        return Err(OptionParseError::invalid("brightness must be between -255 and 255"));
    }
    Ok(parsed)
}

/// Independent horizontal and vertical zoom factors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom {
    pub x: f32,
    pub y: f32,
}

impl Default for Zoom {
    fn default() -> Self {
        Self { x: 1.0, y: 1.0 }
    }
}

impl Zoom {
    /// Parses `zoom_x_y` or `zoom_x:zoom_y`.
    pub fn parse(args: &[String]) -> Result<Self, OptionParseError> {
        let Some(x) = arg(args, 0) else {
            return Err(OptionParseError::invalid("zoom option requires one argument"));
        };
        let x = parse_positive_f32(x, "zoom")?;
        let y = match arg(args, 1) {
            Some(value) => parse_positive_f32(value, "zoom")?,
            None => x,
        };
        Ok(Self { x, y })
    }

    pub fn is_identity(&self) -> bool {
        (self.x - 1.0).abs() <= f32::EPSILON && (self.y - 1.0).abs() <= f32::EPSILON
    }

    /// The largest of the two factors, which is what a target size has to be
    /// grown by before deciding how far the source may be shrunk on load.
    pub fn max_factor(&self) -> f32 {
        self.x.max(self.y).max(1.0)
    }
}

/// Where a watermark sits on the image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WatermarkPosition {
    /// Anchored to one of the nine standard positions.
    Anchor(GravityType),
    /// Tiled across the whole image.
    #[default]
    Replicate,
}

impl WatermarkPosition {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "re" => Some(Self::Replicate),
            other => GravityType::parse(other)
                .filter(|kind| *kind != GravityType::FocusPoint)
                .map(Self::Anchor),
        }
    }
}

/// Represents the parameters for a watermark operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Watermark {
    /// Opacity multiplier applied to the watermark's own alpha.
    pub opacity: f32,
    /// Anchor, or `re` to tile.
    pub position: WatermarkPosition,
    /// Horizontal nudge: pixels at magnitude 1 or more, else a fraction.
    pub x_offset: f64,
    /// Vertical nudge, read the same way.
    pub y_offset: f64,
    /// Scale relative to the image's width. Zero keeps the default sizing.
    pub scale: f64,
}

impl Default for Watermark {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            position: WatermarkPosition::Anchor(GravityType::Center),
            x_offset: 0.0,
            y_offset: 0.0,
            scale: 0.0,
        }
    }
}

impl Watermark {
    /// Parses `opacity:position[:x_offset[:y_offset[:scale]]]`.
    pub fn parse(args: &[String]) -> Result<Self, OptionParseError> {
        let Some(opacity) = arg(args, 0) else {
            return Err(OptionParseError::invalid(
                "watermark option requires at least one argument: opacity",
            ));
        };
        let opacity = parse_float(opacity, "watermark opacity")?;
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(OptionParseError::invalid("watermark opacity must be between 0 and 1"));
        }

        let position = match arg(args, 1) {
            Some(value) => WatermarkPosition::parse(value).ok_or_else(|| {
                OptionParseError::invalid(
                    "watermark position must be one of: ce, no, so, ea, we, noea, nowe, soea, sowe, re",
                )
            })?,
            None => WatermarkPosition::Anchor(GravityType::Center),
        };

        let x_offset = match arg(args, 2) {
            Some(value) => f64::from(parse_float(value, "watermark x offset")?),
            None => 0.0,
        };
        let y_offset = match arg(args, 3) {
            Some(value) => f64::from(parse_float(value, "watermark y offset")?),
            None => 0.0,
        };
        let scale = match arg(args, 4) {
            Some(value) => {
                let scale = parse_float(value, "watermark scale")?;
                if !scale.is_finite() || scale < 0.0 {
                    return Err(OptionParseError::invalid(
                        "watermark scale must be a finite non-negative number",
                    ));
                }
                f64::from(scale)
            }
            None => 0.0,
        };

        if !x_offset.is_finite() || !y_offset.is_finite() {
            return Err(OptionParseError::invalid("watermark offsets must be finite numbers"));
        }

        Ok(Self {
            opacity,
            position,
            x_offset,
            y_offset,
            scale,
        })
    }
}
