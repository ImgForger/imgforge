use axum::{
    body::Bytes,
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use image::codecs::gif::GifDecoder;
use image::io::Reader as ImageReader;
use image::{AnimationDecoder, ImageDecoder as _};
use percent_encoding::percent_decode_str;
use sha2::Sha256;
use std::env;
use std::io::Cursor;

mod processing;

const ENV_IMGFORGE_KEY: &str = "IMGFORGE_KEY";
const ENV_IMGFORGE_SALT: &str = "IMGFORGE_SALT";
const ENV_IMGFORGE_AUTH_TOKEN: &str = "IMGFORGE_AUTH_TOKEN";
const ENV_ALLOW_UNSIGNED: &str = "ALLOW_UNSIGNED";
const ENV_MAX_SRC_FILE_SIZE: &str = "IMGFORGE_MAX_SRC_FILE_SIZE";
const ENV_ALLOWED_MIME_TYPES: &str = "IMGFORGE_ALLOWED_MIME_TYPES";
const ENV_MAX_SRC_RESOLUTION: &str = "IMGFORGE_MAX_SRC_RESOLUTION";
const ENV_MAX_ANIMATION_FRAMES: &str = "IMGFORGE_MAX_ANIMATION_FRAMES";
const ENV_MAX_ANIMATION_FRAME_RESOLUTION: &str = "IMGFORGE_MAX_ANIMATION_FRAME_RESOLUTION";

#[derive(Debug)]
enum SourceUrlInfo {
    Plain {
        url: String,
        extension: Option<String>,
    },
    Base64 {
        encoded_url: String,
        extension: Option<String>,
    },
}

impl SourceUrlInfo {
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

#[derive(Debug)]
pub struct ProcessingOption {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
struct ImgforgeUrl {
    signature: String,
    processing_options: Vec<ProcessingOption>,
    source_url: SourceUrlInfo,
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/{*path}", get(image_forge_handler));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn image_forge_handler(
    Path(path): Path<String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> impl IntoResponse {
    println!("Full path captured: {}", path);

    if let Some(token) = env::var(ENV_IMGFORGE_AUTH_TOKEN).ok() {
        if !token.is_empty() {
            if let Some(TypedHeader(auth)) = auth_header {
                if auth.token() != token {
                    return (StatusCode::FORBIDDEN, "Invalid authorization token".to_string()).into_response();
                }
            } else {
                return (StatusCode::FORBIDDEN, "Missing authorization token".to_string()).into_response();
            }
        }
    }

    let key_str = env::var(ENV_IMGFORGE_KEY).unwrap_or_default();
    let salt_str = env::var(ENV_IMGFORGE_SALT).unwrap_or_default();
    let allow_unsigned = env::var(ENV_ALLOW_UNSIGNED).unwrap_or_default().to_lowercase() == "true";

    let key = match hex::decode(key_str) {
        Ok(k) => k,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid IMGFORGE_KEY".to_string(),
            )
                .into_response()
        }
    };
    let salt = match hex::decode(salt_str) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid IMGFORGE_SALT".to_string(),
            )
                .into_response()
        }
    };

    let url_parts = match parse_path(&path) {
        Some(parts) => parts,
        None => return (StatusCode::BAD_REQUEST, "Invalid URL format".to_string()).into_response(),
    };

    if url_parts.signature == "unsafe" {
        if !allow_unsigned {
            return (
                StatusCode::FORBIDDEN,
                "Unsigned URLs are not allowed".to_string(),
            )
                .into_response();
        }
    } else {
        let path_to_sign = &path[path.find('/').unwrap() + 1..];
        if !validate_signature(&key, &salt, &url_parts.signature, path_to_sign) {
            return (StatusCode::FORBIDDEN, "Invalid signature".to_string()).into_response();
        }
    }

    let decoded_url = match url_parts.source_url.decode() {
        Ok(url) => url,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Error decoding URL: {}", e),
            )
                .into_response()
        }
    };

    let response = match reqwest::get(&decoded_url).await {
        Ok(res) => res,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Error fetching image: {}", e),
            )
                .into_response()
        }
    };

    let mut headers = header::HeaderMap::new();
    if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
        headers.insert(header::CONTENT_TYPE, content_type.clone());
    }

    let image_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Error reading image bytes: {}", e),
            )
                .into_response()
        }
    };

    if let Ok(max_size) = env::var(ENV_MAX_SRC_FILE_SIZE) {
        if let Ok(max_size) = max_size.parse::<usize>() {
            if image_bytes.len() > max_size {
                return (StatusCode::BAD_REQUEST, "Source image file size is too large".to_string()).into_response();
            }
        }
    }

    if let Ok(allowed_types) = env::var(ENV_ALLOWED_MIME_TYPES) {
        if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
            let content_type_str = content_type.to_str().unwrap_or("");
            let allowed_types: Vec<&str> = allowed_types.split(',').collect();
            if !allowed_types.contains(&content_type_str) {
                return (StatusCode::BAD_REQUEST, "Source image MIME type is not allowed".to_string()).into_response();
            }

            if content_type_str == "image/gif" {
                if let Ok(max_frames) = env::var(ENV_MAX_ANIMATION_FRAMES) {
                    if let Ok(max_frames) = max_frames.parse::<usize>() {
                        let decoder = GifDecoder::new(Cursor::new(&image_bytes)).unwrap();
                        if decoder.into_frames().count() > max_frames {
                            return (StatusCode::BAD_REQUEST, "Too many frames in animated image".to_string()).into_response();
                        }
                    }
                }

                if let Ok(max_res) = env::var(ENV_MAX_ANIMATION_FRAME_RESOLUTION) {
                    if let Ok(max_res) = max_res.parse::<f32>() {
                        let decoder = GifDecoder::new(Cursor::new(&image_bytes)).unwrap();
                        for frame in decoder.into_frames() {
                            let frame = frame.unwrap();
                            let (w, h) = frame.buffer().dimensions();
                            let res_mp = (w * h) as f32 / 1_000_000.0;
                            if res_mp > max_res {
                                return (StatusCode::BAD_REQUEST, "Animated image frame resolution is too large".to_string()).into_response();
                            }
                        }
                    }
                }
            }
        }
    }

    if let Ok(max_res) = env::var(ENV_MAX_SRC_RESOLUTION) {
        if let Ok(max_res) = max_res.parse::<f32>() {
            let reader = ImageReader::new(Cursor::new(&image_bytes)).with_guessed_format();
            if let Ok(reader) = reader {
                if let Ok((w, h)) = reader.into_dimensions() {
                    let res_mp = (w * h) as f32 / 1_000_000.0;
                    if res_mp > max_res {
                        return (StatusCode::BAD_REQUEST, "Source image resolution is too large".to_string()).into_response();
                    }
                }
            }
        }
    }

    let processed_image_bytes =
        match processing::process_image(image_bytes.into(), url_parts.processing_options).await {
            Ok(bytes) => bytes,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Error processing image: {}", e),
                )
                    .into_response();
            }
        };

    (StatusCode::OK, headers, processed_image_bytes).into_response()
}

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

    let processing_options = processing_options_parts
        .iter()
        .map(|s| {
            let mut parts = s.split(':');
            let name = parts.next().unwrap_or("").to_string();
            let args = parts.map(|s| s.to_string()).collect();
            ProcessingOption { name, args }
        })
        .collect();

    let source_url = parse_source_url_path(source_url_parts)?;

    Some(ImgforgeUrl {
        signature,
        processing_options,
        source_url,
    })
}

fn parse_source_url_path(parts: &[&str]) -> Option<SourceUrlInfo> {
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
        Some(SourceUrlInfo::Plain { url, extension })
    } else {
        let path = parts.join("/");
        let (encoded_url, extension) = match path.rsplit_once('.') {
            Some((url, ext)) => (url.to_string(), Some(ext.to_string())),
            None => (path.to_string(), None),
        };
        Some(SourceUrlInfo::Base64 {
            encoded_url,
            extension,
        })
    }
}