//! Geometry options: resizing, gravity, cropping, extending, trimming and
//! flipping.

use super::error::{arg, parse_float, parse_integer, OptionParseError};
use crate::processing::utils::parse_boolean;
use std::str::FromStr;

/// How a resize maps the source onto the requested box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ResizingType {
    /// Keep the aspect ratio and fit inside the box.
    #[default]
    Fit,
    /// Keep the aspect ratio, cover the box, and crop what projects out.
    Fill,
    /// `Fill`, except that a result smaller than the box is cropped to the
    /// box's aspect ratio rather than padded out to its size.
    FillDown,
    /// Ignore the aspect ratio and hit the box exactly.
    Force,
    /// `Fill` when the source and the box share an orientation, `Fit` otherwise.
    Auto,
}

/// Rejected value for [`ResizingType`], carrying the list of what is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizingTypeParseError;

impl FromStr for ResizingType {
    type Err = ResizingTypeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fit" => Ok(Self::Fit),
            "fill" => Ok(Self::Fill),
            "fill-down" | "fill_down" => Ok(Self::FillDown),
            "force" => Ok(Self::Force),
            "auto" => Ok(Self::Auto),
            _ => Err(ResizingTypeParseError),
        }
    }
}

impl ResizingType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::Fill => "fill",
            Self::FillDown => "fill-down",
            Self::Force => "force",
            Self::Auto => "auto",
        }
    }

    /// Whether a zero axis is taken from the source rather than derived from
    /// the aspect ratio. Only `force` does that, which is why it is the one
    /// resizing type scale-on-load has to leave a full-size axis for.
    pub const fn fills_zero_axis_from_source(self) -> bool {
        matches!(self, Self::Force)
    }
}

/// Represents the parameters for a resize operation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Resize {
    /// The type of resizing to perform.
    pub resizing_type: ResizingType,
    /// The target width for the resize operation.
    pub width: u32,
    /// The target height for the resize operation.
    pub height: u32,
}

/// Anchor used when an operation has to choose which part of an image to keep.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GravityType {
    #[default]
    Center,
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
    /// The offsets name a point, in 0..1 of each axis, to centre the result on.
    FocusPoint,
    /// libvips picks the window, by looking for the part of the image a viewer
    /// would look at.
    Smart,
}

impl GravityType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ce" => Some(Self::Center),
            "no" => Some(Self::North),
            "so" => Some(Self::South),
            "ea" => Some(Self::East),
            "we" => Some(Self::West),
            "noea" => Some(Self::NorthEast),
            "nowe" => Some(Self::NorthWest),
            "soea" => Some(Self::SouthEast),
            "sowe" => Some(Self::SouthWest),
            "fp" => Some(Self::FocusPoint),
            "sm" => Some(Self::Smart),
            _ => None,
        }
    }

    /// Whether the window position is chosen by looking at the pixels rather
    /// than computed from the geometry.
    pub const fn is_content_aware(self) -> bool {
        matches!(self, Self::Smart)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Center => "ce",
            Self::North => "no",
            Self::South => "so",
            Self::East => "ea",
            Self::West => "we",
            Self::NorthEast => "noea",
            Self::NorthWest => "nowe",
            Self::SouthEast => "soea",
            Self::SouthWest => "sowe",
            Self::FocusPoint => "fp",
            Self::Smart => "sm",
        }
    }
}

/// An anchor plus its offsets.
///
/// [`GravityType::Smart`] ignores the offsets: the window is chosen from the
/// image's content. For every anchor but that and [`GravityType::FocusPoint`]
/// the offsets nudge the window away from the anchor: an absolute pixel count when the magnitude is
/// at least 1, otherwise a fraction of the axis being positioned. Focus point
/// instead reads them as the coordinates, in 0..1, that the result centres on.
/// Both readings come from imgproxy, whose `calcPosition` this mirrors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gravity {
    pub kind: GravityType,
    pub x: f64,
    pub y: f64,
}

impl Default for Gravity {
    fn default() -> Self {
        Self::new(GravityType::Center)
    }
}

impl Gravity {
    pub const fn new(kind: GravityType) -> Self {
        Self { kind, x: 0.0, y: 0.0 }
    }

    /// Parses `type[:x_offset[:y_offset]]` starting at `start` in `args`.
    pub fn parse(args: &[String], start: usize, option: &'static str) -> Result<Self, OptionParseError> {
        let Some(kind) = arg(args, start) else {
            return Err(OptionParseError::invalid(format!(
                "{option} gravity requires a gravity type"
            )));
        };
        let kind = GravityType::parse(kind).ok_or_else(|| {
            OptionParseError::invalid(format!(
                "{option} gravity must be one of: ce, no, so, ea, we, noea, nowe, soea, sowe, fp, sm"
            ))
        })?;

        let x = match arg(args, start + 1) {
            Some(value) => f64::from(parse_float(value, "gravity x offset")?),
            None => 0.0,
        };
        let y = match arg(args, start + 2) {
            Some(value) => f64::from(parse_float(value, "gravity y offset")?),
            None => 0.0,
        };

        if !x.is_finite() || !y.is_finite() {
            return Err(OptionParseError::invalid("gravity offsets must be finite numbers"));
        }

        if kind == GravityType::FocusPoint && (!(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y)) {
            return Err(OptionParseError::invalid(
                "focus point gravity coordinates must be between 0 and 1",
            ));
        }

        Ok(Self { kind, x, y })
    }
}

/// Represents the parameters for a crop operation.
///
/// Extents follow imgproxy: a value of at least 1 is a pixel count, a value
/// below 1 is a fraction of the source axis, and 0 means "the whole axis".
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Crop {
    pub width: f64,
    pub height: f64,
    /// Optional crop gravity, falling back to the request's own `gravity`.
    pub gravity: Option<Gravity>,
}

/// Correcting the crop area's shape, independently of its size.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CropAspectRatio {
    /// The ratio the crop area should match. Zero means "leave it alone".
    pub ratio: f64,
    /// Grow the crop area to reach the ratio instead of shrinking it.
    pub enlarge: bool,
}

impl CropAspectRatio {
    /// Parses `aspect_ratio[:enlarge]`.
    pub fn parse(args: &[String]) -> Result<Self, OptionParseError> {
        let Some(ratio) = arg(args, 0) else {
            return Err(OptionParseError::invalid(
                "crop_aspect_ratio option requires an aspect ratio",
            ));
        };
        let ratio = f64::from(parse_float(ratio, "crop aspect ratio")?);
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(OptionParseError::invalid(
                "crop aspect ratio must be a finite non-negative number",
            ));
        }

        Ok(Self {
            ratio,
            enlarge: arg(args, 1).map(parse_boolean).unwrap_or(false),
        })
    }

    /// Reshapes a crop window to the requested ratio.
    ///
    /// Shrinking the long axis is the default because it cannot ask for pixels
    /// the source does not have; `enlarge` grows the short axis instead, and
    /// the caller still clamps the result to the image.
    pub fn correct(&self, width: u32, height: u32) -> (u32, u32) {
        if self.ratio <= 0.0 || width == 0 || height == 0 {
            return (width, height);
        }

        let current = f64::from(width) / f64::from(height);
        if (current - self.ratio).abs() < f64::EPSILON {
            return (width, height);
        }

        let too_wide = current > self.ratio;
        if too_wide == self.enlarge {
            // Adjust the height: shrink it when the window is too tall, or grow
            // it when the window is too wide and we were told to enlarge.
            let corrected = (f64::from(width) / self.ratio).round() as u32;
            (width, corrected.max(1))
        } else {
            let corrected = (f64::from(height) * self.ratio).round() as u32;
            (corrected.max(1), height)
        }
    }
}

impl Crop {
    /// Resolves one extent against the source axis it is measured on.
    pub fn resolve_extent(extent: f64, source: u32) -> u32 {
        if extent <= 0.0 || !extent.is_finite() {
            return 0;
        }
        if extent >= 1.0 {
            return extent as u32;
        }
        ((f64::from(source) * extent).round() as u32).max(1)
    }

    /// The pixel extents this crop resolves to against a given source.
    pub fn resolve(&self, src_width: u32, src_height: u32) -> (u32, u32) {
        (
            Self::resolve_extent(self.width, src_width),
            Self::resolve_extent(self.height, src_height),
        )
    }
}

/// Padding out to a target size (`extend`) or to a target aspect ratio
/// (`extend_aspect_ratio`).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Extend {
    pub enabled: bool,
    /// Where the source sits on the extended canvas, falling back to centre.
    pub gravity: Option<Gravity>,
}

impl Extend {
    /// Parses `enabled[:gravity_type[:x[:y]]]`.
    ///
    /// Smart gravity is refused. Extending *adds* canvas around the image
    /// rather than choosing a window inside it, so there is nothing for
    /// `smartcrop` to look at; the value would reach `calc_position`, fall
    /// through to the centre branch, and quietly behave as `ce`. A URL that
    /// appears to work and does something else is worse than one that is
    /// rejected. imgproxy draws the line in the same place — its
    /// `ExtendGravityTypes` omits `sm` while keeping `fp`, which does mean
    /// something here: it positions the image against a point on the canvas.
    pub fn parse(args: &[String], option: &'static str) -> Result<Self, OptionParseError> {
        let enabled = arg(args, 0).map(parse_boolean).unwrap_or(false);
        let gravity = match arg(args, 1) {
            Some(_) => {
                let gravity = Gravity::parse(args, 1, option)?;
                if gravity.kind.is_content_aware() {
                    return Err(OptionParseError::invalid(format!(
                        "{option} gravity cannot be {}: it positions the image on a larger canvas, \
                         which has no content to choose from",
                        gravity.kind.as_str()
                    )));
                }
                Some(gravity)
            }
            None => None,
        };
        Ok(Self { enabled, gravity })
    }
}

/// Represents the parameters for a flip operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flip {
    pub horizontal: bool,
    pub vertical: bool,
}

/// Border trimming controls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trim {
    /// How far a pixel may differ from the background and still be trimmed.
    pub threshold: f64,
    /// Colour to treat as background. Detected from the top-left pixel when absent.
    pub color: Option<[u8; 4]>,
    /// Cut equal amounts from the left and right.
    pub equal_hor: bool,
    /// Cut equal amounts from the top and bottom.
    pub equal_ver: bool,
}

/// Parses `padding:top[:right[:bottom[:left]]]` into (top, right, bottom, left).
pub(super) fn parse_padding(args: &[String]) -> Result<(u32, u32, u32, u32), OptionParseError> {
    if args.is_empty() {
        return Err(OptionParseError::invalid(
            "padding option requires at least one argument",
        ));
    }
    let values: Vec<u32> = args
        .iter()
        .map(|value| parse_integer(value, "padding"))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(match values.len() {
        1 => (values[0], values[0], values[0], values[0]),
        2 => (values[0], values[1], values[0], values[1]),
        3 => (values[0], values[1], values[2], values[1]),
        4 => (values[0], values[1], values[2], values[3]),
        _ => return Err(OptionParseError::invalid("padding must have 1 to 4 arguments")),
    })
}

/// Parses the `trim` arguments.
pub(super) fn parse_trim(args: &[String]) -> Result<Trim, OptionParseError> {
    let Some(threshold) = arg(args, 0) else {
        return Err(OptionParseError::invalid(
            "trim option requires at least one argument: threshold",
        ));
    };
    let threshold = parse_float(threshold, "trim threshold")?;
    if !threshold.is_finite() || threshold < 0.0 {
        return Err(OptionParseError::invalid(
            "trim threshold must be a finite, non-negative number",
        ));
    }

    // An empty colour means "work it out from the image", which is how imgproxy
    // behaves when the argument is omitted.
    let color = match arg(args, 1) {
        Some(value) => Some(crate::processing::utils::parse_hex_color(value).map_err(OptionParseError::Color)?),
        None => None,
    };

    Ok(Trim {
        threshold: f64::from(threshold),
        color,
        equal_hor: arg(args, 2).map(parse_boolean).unwrap_or(false),
        equal_ver: arg(args, 3).map(parse_boolean).unwrap_or(false),
    })
}

const VALID_ROTATIONS: [u16; 4] = [0, 90, 180, 270];

pub(super) fn parse_rotation(value: &str) -> Result<u16, OptionParseError> {
    let rotation = parse_integer(value, "rotation")?;
    if !VALID_ROTATIONS.contains(&rotation) {
        return Err(OptionParseError::invalid("rotation must be one of: 0, 90, 180, 270"));
    }
    Ok(rotation)
}
