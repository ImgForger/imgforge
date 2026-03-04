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

pub fn content_type_to_format(content_type: &str) -> Option<&'static str> {
    let mime = content_type.split(';').next()?.trim().to_ascii_lowercase();
    match mime.as_str() {
        "image/jpeg" => Some("jpeg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/tiff" => Some("tiff"),
        "image/avif" => Some("avif"),
        "image/heif" | "image/heic" => Some("heif"),
        _ => None,
    }
}
