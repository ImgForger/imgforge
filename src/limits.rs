use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize, ParseFloatError, ParseIntError};
use std::str::FromStr;
use thiserror::Error;

const PIXELS_PER_MEGAPIXEL: f64 = 1_000_000.0;

/// Error returned when a source-image security limit is invalid.
#[derive(Debug, Error)]
pub enum SecurityLimitError {
    #[error("must be a positive integer number of bytes")]
    InvalidFileSize(#[source] ParseIntError),
    #[error("must be greater than zero")]
    ZeroFileSize,
    #[error("must be a finite positive number of megapixels")]
    InvalidResolution(#[source] ParseFloatError),
    #[error("must be a finite positive number of megapixels")]
    NonPositiveResolution,
    #[error("must allow at least one pixel")]
    ResolutionBelowOnePixel,
    #[error("is too large to represent as a pixel count")]
    ResolutionTooLarge,
    #[error("must be a positive whole number of pixels")]
    InvalidDimension(#[source] ParseIntError),
    #[error("must be greater than zero")]
    ZeroDimension,
}

/// Validated maximum source-image size in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxSourceFileSize(NonZeroUsize);

impl MaxSourceFileSize {
    /// Construct a limit from a positive byte count.
    pub fn new(bytes: usize) -> Result<Self, SecurityLimitError> {
        NonZeroUsize::new(bytes)
            .map(Self)
            .ok_or(SecurityLimitError::ZeroFileSize)
    }

    /// Return the configured byte count.
    pub fn get(self) -> usize {
        self.0.get()
    }
}

impl FromStr for MaxSourceFileSize {
    type Err = SecurityLimitError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = value.parse::<usize>().map_err(SecurityLimitError::InvalidFileSize)?;
        Self::new(bytes)
    }
}

/// Validated maximum source-image resolution stored as a pixel count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxSourceResolution(NonZeroU64);

impl MaxSourceResolution {
    /// Construct a limit from a finite, positive megapixel value.
    pub fn from_megapixels(megapixels: f64) -> Result<Self, SecurityLimitError> {
        if !megapixels.is_finite() || megapixels <= 0.0 {
            return Err(SecurityLimitError::NonPositiveResolution);
        }

        let pixels = megapixels * PIXELS_PER_MEGAPIXEL;
        if !pixels.is_finite() || pixels >= u64::MAX as f64 {
            return Err(SecurityLimitError::ResolutionTooLarge);
        }

        let pixels = pixels.floor() as u64;
        NonZeroU64::new(pixels)
            .map(Self)
            .ok_or(SecurityLimitError::ResolutionBelowOnePixel)
    }

    /// Return the maximum allowed pixel count.
    pub fn pixels(self) -> u64 {
        self.0.get()
    }
}

impl FromStr for MaxSourceResolution {
    type Err = SecurityLimitError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let megapixels = value.parse::<f64>().map_err(SecurityLimitError::InvalidResolution)?;
        Self::from_megapixels(megapixels)
    }
}

/// Validated ceiling for either dimension of the processed image.
///
/// The source limits bound what imgforge will read; this bounds what it will
/// produce. Nothing else does: a request may ask for any width and height it
/// likes, and with `enlarge:true` a small source can be told to become
/// arbitrarily large.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxResultDimension(NonZeroU32);

impl MaxResultDimension {
    /// Construct a limit from a positive pixel count.
    pub fn new(pixels: u32) -> Result<Self, SecurityLimitError> {
        NonZeroU32::new(pixels)
            .map(Self)
            .ok_or(SecurityLimitError::ZeroDimension)
    }

    /// Return the maximum allowed width or height, in pixels.
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl FromStr for MaxResultDimension {
    type Err = SecurityLimitError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let pixels = value.parse::<u32>().map_err(SecurityLimitError::InvalidDimension)?;
        Self::new(pixels)
    }
}

/// Validated ceiling on how many frames of an animated source are decoded.
///
/// An animation multiplies every cost by its frame count, so the source limits
/// that bound a still image bound almost nothing here: a 1000-frame GIF well
/// under the resolution ceiling still asks for a thousand times the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxAnimationFrames(NonZeroU32);

impl MaxAnimationFrames {
    /// Construct a limit from a positive frame count.
    pub fn new(frames: u32) -> Result<Self, SecurityLimitError> {
        NonZeroU32::new(frames)
            .map(Self)
            .ok_or(SecurityLimitError::ZeroDimension)
    }

    /// Return the maximum allowed frame count.
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl FromStr for MaxAnimationFrames {
    type Err = SecurityLimitError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let frames = value.parse::<u32>().map_err(SecurityLimitError::InvalidDimension)?;
        Self::new(frames)
    }
}

/// Validated ceiling on the pixel count of a single animation frame.
///
/// Expressed in megapixels, like [`MaxSourceResolution`], because that is what
/// imgproxy's `max_animation_frame_resolution` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxAnimationFrameResolution(NonZeroU64);

impl MaxAnimationFrameResolution {
    /// Construct a limit from a finite, positive megapixel value.
    pub fn from_megapixels(megapixels: f64) -> Result<Self, SecurityLimitError> {
        MaxSourceResolution::from_megapixels(megapixels).map(|limit| Self(limit.0))
    }

    /// Return the maximum allowed pixel count per frame.
    pub fn pixels(self) -> u64 {
        self.0.get()
    }
}

impl FromStr for MaxAnimationFrameResolution {
    type Err = SecurityLimitError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let megapixels = value.parse::<f64>().map_err(SecurityLimitError::InvalidResolution)?;
        Self::from_megapixels(megapixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_limits_reject_zero_and_nonsense() {
        for value in ["0", "-1", "abc", ""] {
            assert!(value.parse::<MaxAnimationFrames>().is_err(), "accepted {value}");
            assert!(
                value.parse::<MaxAnimationFrameResolution>().is_err(),
                "accepted {value}"
            );
        }

        assert_eq!("64".parse::<MaxAnimationFrames>().unwrap().get(), 64);
        assert_eq!(
            "1.5".parse::<MaxAnimationFrameResolution>().unwrap().pixels(),
            1_500_000
        );
    }

    #[test]
    fn resolution_rejects_non_finite_and_non_positive_values() {
        for value in ["NaN", "inf", "-inf", "0", "-1"] {
            assert!(value.parse::<MaxSourceResolution>().is_err(), "accepted {value}");
        }
    }

    #[test]
    fn resolution_converts_megapixels_to_pixels() {
        let limit = "10.5".parse::<MaxSourceResolution>().expect("valid resolution");
        assert_eq!(limit.pixels(), 10_500_000);
    }

    #[test]
    fn result_dimension_requires_a_positive_whole_number() {
        for value in ["0", "-1", "1.5", "abc", ""] {
            assert!(value.parse::<MaxResultDimension>().is_err(), "accepted {value}");
        }
        assert_eq!("4096".parse::<MaxResultDimension>().unwrap().get(), 4096);
    }

    #[test]
    fn file_size_requires_a_positive_integer() {
        for value in ["invalid", "0", "-1"] {
            assert!(value.parse::<MaxSourceFileSize>().is_err(), "accepted {value}");
        }
    }
}
