//! Main application module for the imgforge server.
//! This module handles HTTP requests, URL parsing, signature validation, and delegates image processing.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use image::AnimationDecoder;
use image::ImageReader;
use percent_encoding::percent_decode_str;
use serde_json::json;
use sha2::Sha256;
use std::env;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

mod processing;

const ENV_IMGFORGE_LOG_LEVEL: &str = "IMGFORGE_LOG_LEVEL";
const ENV_IMGFORGE_KEY: &str = "IMGFORGE_KEY";
const ENV_IMGFORGE_SALT: &str = "IMGFORGE_SALT";
const ENV_IMGFORGE_SECRET: &str = "IMGFORGE_SECRET";
const ENV_ALLOW_UNSIGNED: &str = "ALLOW_UNSIGNED";
const ENV_MAX_SRC_FILE_SIZE: &str = "IMGFORGE_MAX_SRC_FILE_SIZE";
const ENV_ALLOWED_MIME_TYPES: &str = "IMGFORGE_ALLOWED_MIME_TYPES";
const ENV_MAX_SRC_RESOLUTION: &str = "IMGFORGE_MAX_SRC_RESOLUTION";
const ENV_MAX_ANIMATION_FRAMES: &str = "IMGFORGE_MAX_ANIMATION_FRAMES";
const ENV_MAX_ANIMATION_FRAME_RESOLUTION: &str = "IMGFORGE_MAX_ANIMATION_FRAME_RESOLUTION";
const ENV_ALLOW_SECURITY_OPTIONS: &str = "IMGFORGE_ALLOW_SECURITY_OPTIONS";
const ENV_WORKERS: &str = "IMGFORGE_WORKERS";

/// Application state shared across handlers.
struct AppState {
    /// Semaphore to limit the number of concurrent image processing tasks.
    semaphore: Semaphore,
}

/// Information about the source URL, including its type and extension.
#[derive(Debug)]
enum SourceUrlInfo {
    /// A plain (percent-encoded) source URL.
    Plain { url: String },
    /// A Base64-encoded source URL.
    Base64 { encoded_url: String },
}

impl SourceUrlInfo {
    /// Decodes the source URL based on its type.
    /// Returns the decoded URL as a String or an error message.
    fn decode(&self) -> Result<String, String> {
        match self {
            SourceUrlInfo::Plain { url, .. } => percent_decode_str(url)
                .decode_utf8()
                .map(|s| s.to_string())
                .map_err(|e| e.to_string()),
            SourceUrlInfo::Base64 { encoded_url, .. } => general_purpose::URL_SAFE_NO_PAD
                .decode(encoded_url)
                .map_err(|e| e.to_string())
                .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string())),
        }
    }
}

/// Represents a single image processing option from the URL path.
#[derive(Debug)]
pub struct ProcessingOption {
    /// The name of the processing option (e.g., "resize", "quality").
    pub name: String,
    /// Arguments for the processing option.
    pub args: Vec<String>,
}

/// Represents the parsed components of an imgforge URL.
#[derive(Debug)]
struct ImgforgeUrl {
    /// The signature used for URL validation.
    signature: String,
    /// A list of processing options to apply to the image.
    processing_options: Vec<ProcessingOption>,
    /// Information about the source image URL.
    source_url: SourceUrlInfo,
}

/// Main entry point for the imgforge server application.
#[tokio::main]
async fn main() {
    let workers = env::var(ENV_WORKERS)
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .unwrap_or(0);
    let semaphore = if workers > 0 {
        Semaphore::new(workers)
    } else {
        // The default number of workers per instance is twice the number of CPUs on the machine that runs it.
        Semaphore::new(num_cpus::get() * 2)
    };

    let state = Arc::new(AppState { semaphore });

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_env(ENV_IMGFORGE_LOG_LEVEL))
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting imgforge server...");

    let app = Router::new()
        .route("/status", get(status_handler))
        .route("/info/{*path}", get(info_handler))
        .route("/{*path}", get(image_forge_handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

/// Handles the /status endpoint, returning a simple JSON status.
async fn status_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

/// Handles the /info/{*path} endpoint, returning metadata about the source image.
///
/// This handler parses the URL, validates the signature, fetches the image,
/// and extracts its width, height, and format, returning them as a JSON object.
async fn info_handler(
    State(_state): State<Arc<AppState>>,
    Path(path): Path<String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> impl IntoResponse {
    info!("Info path captured: {}", path);

    let (_url_parts, _decoded_url, image_bytes, _content_type) = match common_image_setup(&path, auth_header).await {
        Ok(data) => data,
        Err(response) => return response,
    };
    debug!("Processing info request for URL: {}", _decoded_url);

    let reader = ImageReader::new(Cursor::new(&image_bytes)).with_guessed_format();
    let (width, height, format_str) = if let Ok(reader) = reader {
        if let Some(format) = reader.format() {
            let format_str = match format {
                image::ImageFormat::Jpeg => "jpeg",
                image::ImageFormat::Png => "png",
                image::ImageFormat::Gif => "gif",
                image::ImageFormat::WebP => "webp",
                image::ImageFormat::Avif => "avif",
                image::ImageFormat::Tiff => "tiff",
                image::ImageFormat::Bmp => "bmp",
                _ => "unknown", // Handle other formats
            }
            .to_string();
            if let Ok((w, h)) = reader.into_dimensions() {
                (w, h, format_str)
            } else {
                (0, 0, "unknown".to_string())
            }
        } else {
            (0, 0, "unknown".to_string())
        }
    } else {
        (0, 0, "unknown".to_string())
    };

    let json_response = json!({
        "width": width,
        "height": height,
        "format": format_str,
    });

    (StatusCode::OK, Json(json_response)).into_response()
}

/// Handles the main image processing endpoint.
///
/// This handler parses the URL, validates the signature, fetches the image,
/// applies various processing options (resize, crop, etc.), and returns the processed image.
async fn image_forge_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> impl IntoResponse {
    info!("Full path captured: {}", path);

    let (url_parts, _decoded_url, image_bytes, content_type) = match common_image_setup(&path, auth_header).await {
        Ok(data) => data,
        Err(response) => return response,
    };
    debug!("Processing image forge request for URL: {}", _decoded_url);

    let allow_security_options = env::var(ENV_ALLOW_SECURITY_OPTIONS).unwrap_or_default().to_lowercase() == "true";

    let parsed_options = match processing::parse_all_options(url_parts.processing_options) {
        Ok(options) => options,
        Err(e) => {
            error!("Error parsing processing options: {}", e);
            return (StatusCode::BAD_REQUEST, e).into_response();
        }
    };
    debug!("Parsed options: {:?}", parsed_options);

    let mut headers = header::HeaderMap::new();
    if let Some(ct) = content_type {
        headers.insert(header::CONTENT_TYPE, ct.parse().unwrap());
    }
    debug!("Image MIME type: {:?}", headers.get(header::CONTENT_TYPE));

    let max_src_file_size = if allow_security_options {
        parsed_options
            .max_src_file_size
            .or_else(|| env::var(ENV_MAX_SRC_FILE_SIZE).ok().and_then(|s| s.parse().ok()))
    } else {
        env::var(ENV_MAX_SRC_FILE_SIZE).ok().and_then(|s| s.parse().ok())
    };
    debug!("Image size: {} bytes", image_bytes.len());

    if let Some(max_size) = max_src_file_size {
        if image_bytes.len() > max_size {
            error!("Source image file size is too large");
            return (
                StatusCode::BAD_REQUEST,
                "Source image file size is too large".to_string(),
            )
                .into_response();
        }
    }

    if let Ok(allowed_types) = env::var(ENV_ALLOWED_MIME_TYPES) {
        if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
            let content_type_str = content_type.to_str().unwrap_or("");
            let allowed_types: Vec<&str> = allowed_types.split(',').collect();
            if !allowed_types.contains(&content_type_str) {
                error!("Source image MIME type is not allowed: {}", content_type_str);
                return (
                    StatusCode::BAD_REQUEST,
                    "Source image MIME type is not allowed".to_string(),
                )
                    .into_response();
            }

            if content_type_str == "image/gif" {
                if let Ok(max_frames) = env::var(ENV_MAX_ANIMATION_FRAMES) {
                    if let Ok(max_frames) = max_frames.parse::<usize>() {
                        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(&image_bytes)).unwrap();
                        if decoder.into_frames().count() > max_frames {
                            error!("Too many frames in animated image");
                            return (StatusCode::BAD_REQUEST, "Too many frames in animated image".to_string())
                                .into_response();
                        }
                    }
                }

                if let Ok(max_res) = env::var(ENV_MAX_ANIMATION_FRAME_RESOLUTION) {
                    if let Ok(max_res) = max_res.parse::<f32>() {
                        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(&image_bytes)).unwrap();
                        for frame in decoder.into_frames() {
                            let frame = frame.unwrap();
                            let (w, h) = frame.buffer().dimensions();
                            let res_mp = (w * h) as f32 / 1_000_000.0;
                            if res_mp > max_res {
                                error!("Animated image frame resolution is too large");
                                return (
                                    StatusCode::BAD_REQUEST,
                                    "Animated image frame resolution is too large".to_string(),
                                )
                                    .into_response();
                            }
                        }
                    }
                }
            }
        }
    }

    let max_src_resolution = if allow_security_options {
        parsed_options
            .max_src_resolution
            .or_else(|| env::var(ENV_MAX_SRC_RESOLUTION).ok().and_then(|s| s.parse().ok()))
    } else {
        env::var(ENV_MAX_SRC_RESOLUTION).ok().and_then(|s| s.parse().ok())
    };

    if let Some(max_res) = max_src_resolution {
        let reader = ImageReader::new(Cursor::new(&image_bytes)).with_guessed_format();
        if let Ok(reader) = reader {
            if let Ok((w, h)) = reader.into_dimensions() {
                debug!("Image resolution: {}x{}", w, h);
                let res_mp = (w * h) as f32 / 1_000_000.0;
                if res_mp > max_res {
                    error!("Source image resolution is too large");
                    return (
                        StatusCode::BAD_REQUEST,
                        "Source image resolution is too large".to_string(),
                    )
                        .into_response();
                }
            }
        }
    }

    let _permit = if parsed_options.raw {
        None
    } else {
        Some(state.semaphore.acquire().await.unwrap())
    };

    let processed_image_bytes = match processing::process_image(image_bytes.into(), parsed_options).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Error processing image: {}", e);
            return (StatusCode::BAD_REQUEST, format!("Error processing image: {}", e)).into_response();
        }
    };

    (StatusCode::OK, headers, processed_image_bytes).into_response()
}

/// Validates the URL signature using HMAC-SHA256.
///
/// # Arguments
///
/// * `key` - The secret key for HMAC.
/// * `salt` - The salt for HMAC.
/// * `signature` - The signature extracted from the URL.
/// * `path` - The URL path segment to be signed.
///
/// # Returns
///
/// `true` if the signature is valid, `false` otherwise.
fn validate_signature(key: &[u8], salt: &[u8], signature: &str, path: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(salt);
    mac.update(path.as_bytes());

    let result = mac.finalize();
    let expected_signature_bytes = result.into_bytes();
    let expected_signature = &hex::encode(expected_signature_bytes)[..32];

    signature.get(..expected_signature.len()) == Some(expected_signature)
}

/// Performs common setup steps for image handling, including authorization, URL parsing,
/// signature validation, source URL decoding, and image fetching.
///
/// # Arguments
///
/// * `path` - The full URL path from the request.
/// * `auth_header` - Optional `Authorization` header for token validation.
///
/// # Returns
///
/// A `Result` containing a tuple of `(ImgforgeUrl, decoded_url, image_bytes, content_type)`
/// on success, or an `axum::response::Response` on error.
async fn common_image_setup(
    path: &str,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<(ImgforgeUrl, String, Bytes, Option<String>), Response> {
    // Authorization Header Check
    if let Some(token) = env::var(ENV_IMGFORGE_SECRET).ok() {
        if !token.is_empty() {
            if let Some(TypedHeader(auth)) = auth_header {
                if auth.token() != token {
                    error!("Invalid authorization token");
                    return Err((StatusCode::FORBIDDEN, "Invalid authorization token".to_string()).into_response());
                }
            } else {
                error!("Missing authorization token");
                return Err((StatusCode::FORBIDDEN, "Missing authorization token".to_string()).into_response());
            }
        }
    }

    // Key and Salt Decoding
    let key_str = env::var(ENV_IMGFORGE_KEY).unwrap_or_default();
    let salt_str = env::var(ENV_IMGFORGE_SALT).unwrap_or_default();
    let allow_unsigned = env::var(ENV_ALLOW_UNSIGNED).unwrap_or_default().to_lowercase() == "true";

    let key = match hex::decode(key_str) {
        Ok(k) => k,
        Err(_) => {
            error!("Invalid IMGFORGE_KEY");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid IMGFORGE_KEY".to_string()).into_response());
        }
    };
    let salt = match hex::decode(salt_str) {
        Ok(s) => s,
        Err(_) => {
            error!("Invalid IMGFORGE_SALT");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid IMGFORGE_SALT".to_string()).into_response());
        }
    };

    // URL Parsing
    let url_parts = match parse_path(path) {
        Some(parts) => parts,
        None => {
            error!("Invalid URL format: {}", path);
            return Err((StatusCode::BAD_REQUEST, "Invalid URL format".to_string()).into_response());
        }
    };

    // Signature Validation
    if url_parts.signature == "unsafe" {
        if !allow_unsigned {
            error!("Unsigned URLs are not allowed");
            return Err((StatusCode::FORBIDDEN, "Unsigned URLs are not allowed".to_string()).into_response());
        }
    } else {
        let path_to_sign = &path[path.find('/').unwrap() + 1..];
        if !validate_signature(&key, &salt, &url_parts.signature, path_to_sign) {
            error!("Invalid signature for path: {}", path);
            return Err((StatusCode::FORBIDDEN, "Invalid signature".to_string()).into_response());
        }
    }

    // Source URL Decoding
    let decoded_url = match url_parts.source_url.decode() {
        Ok(url) => url,
        Err(e) => {
            error!("Error decoding URL: {}", e);
            return Err((StatusCode::BAD_REQUEST, format!("Error decoding URL: {}", e)).into_response());
        }
    };

    // Image Fetching
    let response = match reqwest::get(&decoded_url).await {
        Ok(res) => res,
        Err(e) => {
            error!("Error fetching image: {}", e);
            return Err((StatusCode::BAD_REQUEST, format!("Error fetching image: {}", e)).into_response());
        }
    };

    let headers = response.headers().clone();

    let image_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Error reading image bytes: {}", e);
            return Err((StatusCode::BAD_REQUEST, format!("Error reading image bytes: {}", e)).into_response());
        }
    };

    let mut content_type: Option<String> = None;
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        if let Ok(ct_str) = ct.to_str() {
            content_type = Some(ct_str.to_string());
        }
    }

    Ok((url_parts, decoded_url, image_bytes, content_type))
}

/// Parses the incoming URL path into its imgforge components.
///
/// # Arguments
///
/// * `path` - The URL path string.
///
/// # Returns
///
/// An `Option<ImgforgeUrl>` containing the parsed URL components if successful, `None` otherwise.
fn parse_path(path: &str) -> Option<ImgforgeUrl> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        return None;
    }

    let signature = parts[0].to_string();
    let rest = &parts[1..];

    let source_url_start_index = rest
        .iter()
        .position(|&s| s == "plain" || !s.contains(':'))
        .unwrap_or(rest.len());

    let processing_options_parts = &rest[..source_url_start_index];
    let source_url_parts = &rest[source_url_start_index..];

    let mut processing_options: Vec<ProcessingOption> = processing_options_parts
        .iter()
        .map(|s| {
            let mut parts = s.split(':');
            let name = parts.next().unwrap_or("").to_string();
            let args = parts.map(|s| s.to_string()).collect();
            ProcessingOption { name, args }
        })
        .collect();

    let (source_url, extension) = parse_source_url_path(source_url_parts)?;

    if let Some(ext) = extension {
        processing_options.push(ProcessingOption {
            name: "format".to_string(),
            args: vec![ext.clone()],
        });
    }

    Some(ImgforgeUrl {
        signature,
        processing_options,
        source_url,
    })
}

/// Parses the source URL path segment into `SourceUrlInfo`.
///
/// # Arguments
///
/// * `parts` - A slice of string slices representing the source URL path segments.
///
/// # Returns
///
/// An `Option<SourceUrlInfo>` containing the parsed source URL information if successful, `None` otherwise.
fn parse_source_url_path(parts: &[&str]) -> Option<(SourceUrlInfo, Option<String>)> {
    if parts.is_empty() {
        return None;
    }

    if parts[0] == "plain" {
        if parts.len() < 2 {
            return None;
        }
        let path = parts[1..].join("/");
        let (url, extension) = match path.rsplit_once('@') {
            Some((url, ext)) => (url.to_string(), Some(ext.to_string())),
            None => (path.to_string(), None),
        };
        Some((SourceUrlInfo::Plain { url }, extension))
    } else {
        let path = parts.join("/");
        let (encoded_url, extension) = match path.rsplit_once('.') {
            Some((url, ext)) => (url.to_string(), Some(ext.to_string())),
            None => (path.to_string(), None),
        };
        Some((SourceUrlInfo::Base64 { encoded_url }, extension))
    }
}
