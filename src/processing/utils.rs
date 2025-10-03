/// Parses a hexadecimal color string into an RGBA array.
///
/// # Arguments
///
/// * `hex` - The hexadecimal color string (e.g., "ffffff" or "#ffffff").
///
/// # Returns
///
/// A `Result` containing the RGBA array on success, or an error message as a `String`.
pub fn parse_hex_color(hex: &str) -> Result<[u8; 4], String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Invalid hex color format".to_string());
    }
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex color".to_string())?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex color".to_string())?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex color".to_string())?;
    Ok([r, g, b, 255])
}

/// Parses a string into a boolean value.
///
/// # Arguments
///
/// * `s` - The string to parse ("1", "true" for true, anything else for false).
///
/// # Returns
///
/// `true` if the string is "1" or "true" (case-sensitive), `false` otherwise.
pub fn parse_boolean(s: &str) -> bool {
    matches!(s, "1" | "true")
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
