use std::num::{NonZeroU64, NonZeroUsize, ParseFloatError, ParseIntError};
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn file_size_requires_a_positive_integer() {
        for value in ["invalid", "0", "-1"] {
            assert!(value.parse::<MaxSourceFileSize>().is_err(), "accepted {value}");
        }
    }
}
