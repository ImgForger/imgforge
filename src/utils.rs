pub fn format_to_content_type(format: &str) -> &'static str {
    match format {
        "png" | "image/png" => "image/png",
        "webp" | "image/webp" => "image/webp",
        "gif" | "image/gif" => "image/gif",
        "tiff" | "image/tiff" => "image/tiff",
        "avif" | "image/avif" => "image/avif",
        "heif" | "image/heif" => "image/heif",
        "jpeg" | "jpg" | "image/jpeg" => "image/jpeg",
        _ => "image/jpeg",
    }
}
