//! Parsing failures and the primitive argument parsers shared by every option
//! group.

use base64::engine::general_purpose;
use base64::Engine as _;
use std::str::FromStr;
use thiserror::Error;

/// Errors produced while parsing image processing options.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OptionParseError {
    #[error("invalid {option} value {value:?}")]
    Integer {
        option: String,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("invalid {option} value {value:?}")]
    Float {
        option: String,
        value: String,
        #[source]
        source: std::num::ParseFloatError,
    },
    #[error("invalid Base64 for {option}")]
    Base64 {
        option: String,
        #[source]
        source: base64::DecodeError,
    },
    #[error("invalid UTF-8 for {option}")]
    Utf8 {
        option: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("invalid {option}: {source}")]
    SecurityLimit {
        option: String,
        #[source]
        source: crate::limits::SecurityLimitError,
    },
    #[error("invalid background color")]
    Color(#[source] crate::processing::utils::ColorParseError),
    #[error("{0}")]
    InvalidValue(String),
}

impl OptionParseError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidValue(message.into())
    }
}

pub(crate) fn parse_integer<T>(value: &str, option: &str) -> Result<T, OptionParseError>
where
    T: FromStr<Err = std::num::ParseIntError>,
{
    value.parse().map_err(|source| OptionParseError::Integer {
        option: option.to_string(),
        value: value.to_string(),
        source,
    })
}

pub(crate) fn parse_float(value: &str, option: &str) -> Result<f32, OptionParseError> {
    value.parse().map_err(|source| OptionParseError::Float {
        option: option.to_string(),
        value: value.to_string(),
        source,
    })
}

pub(crate) fn decode_base64(value: &str, option: &str) -> Result<Vec<u8>, OptionParseError> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|source| OptionParseError::Base64 {
            option: option.to_string(),
            source,
        })
}

pub(crate) fn decode_utf8(value: Vec<u8>, option: &str) -> Result<String, OptionParseError> {
    String::from_utf8(value).map_err(|source| OptionParseError::Utf8 {
        option: option.to_string(),
        source,
    })
}

pub(crate) fn parse_positive_f32(value: &str, option_name: &str) -> Result<f32, OptionParseError> {
    let parsed = parse_float(value, option_name)?;

    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(OptionParseError::invalid(format!(
            "{} must be a finite positive number",
            option_name
        )));
    }

    Ok(parsed)
}

pub(crate) fn parse_unit_f32(value: &str, option_name: &str) -> Result<f32, OptionParseError> {
    let parsed = parse_float(value, option_name)?;

    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        return Err(OptionParseError::invalid(format!(
            "{} must be a finite number between 0 and 1",
            option_name
        )));
    }

    Ok(parsed)
}

pub(crate) fn parse_quality(value: &str, option_name: &str) -> Result<u8, OptionParseError> {
    Ok(parse_integer::<u8>(value, option_name)?.clamp(1, 100))
}

/// Reads argument `index` as a boolean, treating an absent or empty argument as
/// "not specified" rather than `false`.
pub(crate) fn parse_optional_bool(args: &[String], index: usize) -> Option<bool> {
    args.get(index)
        .filter(|arg| !arg.is_empty())
        .map(|arg| crate::processing::utils::parse_boolean(arg))
}

/// Reads argument `index`, skipping empty placeholders left by callers who only
/// wanted to set a later positional argument.
pub(crate) fn arg(args: &[String], index: usize) -> Option<&str> {
    args.get(index).map(String::as_str).filter(|value| !value.is_empty())
}
