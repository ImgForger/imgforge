use thiserror::Error;

/// Errors produced while parsing an RGB hexadecimal color.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ColorParseError {
    #[error("hex color must contain exactly six digits")]
    InvalidLength,
    #[error("invalid {channel} channel in hex color")]
    InvalidChannel {
        channel: &'static str,
        #[source]
        source: std::num::ParseIntError,
    },
}

/// Parses a hexadecimal color string into an RGBA array.
///
/// # Arguments
///
/// * `hex` - The hexadecimal color string (e.g., "ffffff" or "#ffffff").
///
/// # Returns
///
/// A `Result` containing the RGBA array on success, or a typed color parsing error.
pub fn parse_hex_color(hex: &str) -> Result<[u8; 4], ColorParseError> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(ColorParseError::InvalidLength);
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|source| ColorParseError::InvalidChannel { channel: "red", source })?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|source| ColorParseError::InvalidChannel {
        channel: "green",
        source,
    })?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|source| ColorParseError::InvalidChannel {
        channel: "blue",
        source,
    })?;
    Ok([r, g, b, 255])
}

/// Parses a string into a boolean value.
///
/// Accepts the set imgproxy accepts — `1`, `t`, `T`, `true`, `TRUE`, `True` —
/// so a URL written against imgproxy's documentation reads the same here.
/// Anything else, including an empty argument, is false.
pub fn parse_boolean(s: &str) -> bool {
    matches!(s, "1" | "t" | "T" | "true" | "TRUE" | "True")
}

/// Determines if the given dimensions represent a portrait orientation.
///
/// # Arguments
///
/// * `width` - The width of the image.
/// * `height` - The height of the image.
///
/// # Returns
///
/// `true` if the height is greater than the width, `false` otherwise.
pub fn is_portrait(width: u32, height: u32) -> bool {
    height > width
}
