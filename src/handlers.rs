use crate::caching::cache::ImgforgeCache as Cache;
use crate::config::Config;
use crate::constants::*;
use crate::fetch::fetch_image;
use crate::middleware::format_to_content_type;
use crate::processing::options::parse_all_options;
use crate::processing::presets::expand_presets;
use crate::processing::process_image;
use crate::url::{parse_path, validate_signature, ImgforgeUrl};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;
use governor::state::{InMemoryState, NotKeyed};
use governor::RateLimiter;
use libvips::{VipsApp, VipsImage};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

/// Application state shared across handlers.
pub struct AppState {
    /// Semaphore to limit the number of concurrent image processing tasks.
    pub semaphore: Semaphore,
    /// The image cache.
    pub cache: Cache,
    /// Optional rate limiter for incoming requests.
    pub rate_limiter: Option<RateLimiter<NotKeyed, InMemoryState, governor::clock::DefaultClock>>,
    /// The application config
    pub config: Config,
    /// The VipsApp instance for accessing libvips memory metrics
    pub vips_app: Arc<VipsApp>,
}

/// Handles the /status endpoint, returning a simple JSON status.
pub async fn status_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

/// Handles the /info/{*path} endpoint, returning metadata about the source image.
pub async fn info_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> impl IntoResponse {
    debug!("Info path captured: {}", path);

    let (_url_parts, decoded_url, image_bytes, _content_type) =
        match common_image_setup(&path, auth_header, &state.config).await {
            Ok(data) => data,
            Err(response) => return response,
        };
    debug!("Processing info request for URL: {}", decoded_url);

    let (width, height, image_format) = match VipsImage::new_from_buffer(&image_bytes, "") {
        Ok(img) => {
            let format_str = "unknown"; // libvips doesn't easily expose format info
            (img.get_width(), img.get_height(), format_str.to_string())
        }
        Err(_) => (0, 0, "unknown".to_string()),
    };

    let json_response = json!({
        "width": width,
        "height": height,
        "format": image_format.clone(),
    });

    info!(
        "Info handler served metadata path={} url={} dimensions={}x{} format={}",
        path, decoded_url, width, height, image_format
    );

    (StatusCode::OK, Json(json_response)).into_response()
}

/// Handles the main image processing endpoint.
pub async fn image_forge_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> impl IntoResponse {
    debug!("Full path captured: {}", path);
    info!("Imgforge request received path={}", path);

    if !matches!(state.cache, Cache::None) {
        if let Some(cached_image) = state.cache.get(&path).await {
            debug!("Image found in cache for path: {}", path);

            let url_parts = match parse_path(&path) {
                Some(parts) => parts,
                None => {
                    error!("Invalid URL format: {}", path);
                    return (StatusCode::BAD_REQUEST, "Invalid URL format".to_string()).into_response();
                }
            };

            let expanded_options = match expand_presets(
                url_parts.processing_options,
                &state.config.presets,
                state.config.only_presets,
            ) {
                Ok(opts) => opts,
                Err(_) => {
                    let mut headers = header::HeaderMap::new();
                    headers.insert(header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
                    return (StatusCode::OK, headers, cached_image).into_response();
                }
            };

            let parsed_options = match parse_all_options(expanded_options) {
                Ok(options) => options,
                Err(_) => {
                    let mut headers = header::HeaderMap::new();
                    headers.insert(header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
                    return (StatusCode::OK, headers, cached_image).into_response();
                }
            };

            let output_format = parsed_options.format.as_deref().unwrap_or("jpeg");
            let content_type = format_to_content_type(output_format);
            let mut headers = header::HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
            headers.insert(header::CACHE_STATUS, "HIT".parse().unwrap());

            info!("Imgforge cache hit path={} output_format={}", path, output_format);

            return (StatusCode::OK, headers, cached_image).into_response();
        }
    }

    let (url_parts, _decoded_url, image_bytes, source_content_type) =
        match common_image_setup(&path, auth_header, &state.config).await {
            Ok(data) => data,
            Err(response) => return response,
        };
    debug!("Processing image forge request for URL: {}", _decoded_url);

    let expanded_options = match crate::processing::presets::expand_presets(
        url_parts.processing_options,
        &state.config.presets,
        state.config.only_presets,
    ) {
        Ok(opts) => opts,
        Err(e) => {
            error!("Error expanding presets: {}", e);
            return (StatusCode::BAD_REQUEST, e).into_response();
        }
    };

    let parsed_options = match crate::processing::options::parse_all_options(expanded_options) {
        Ok(options) => options,
        Err(e) => {
            error!("Error parsing processing options: {}", e);
            return (StatusCode::BAD_REQUEST, e).into_response();
        }
    };
    debug!("Parsed options: {:?}", parsed_options);

    debug!("Source image MIME type: {:?}", source_content_type);

    let max_src_file_size = if state.config.allow_security_options {
        parsed_options.max_src_file_size.or(state.config.max_src_file_size)
    } else {
        state.config.max_src_file_size
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

    if let Some(allowed_types) = &state.config.allowed_mime_types {
        if let Some(ref content_type) = source_content_type {
            if !allowed_types.contains(&content_type.to_string()) {
                error!("Source image MIME type is not allowed: {}", content_type);
                return (
                    StatusCode::BAD_REQUEST,
                    "Source image MIME type is not allowed".to_string(),
                )
                    .into_response();
            }
        }
    }

    let max_src_resolution = if state.config.allow_security_options {
        parsed_options.max_src_resolution.or(state.config.max_src_resolution)
    } else {
        state.config.max_src_resolution
    };

    if let Some(max_res) = max_src_resolution {
        match VipsImage::new_from_buffer(&image_bytes, "") {
            Ok(img) => {
                let (w, h) = (img.get_width(), img.get_height());
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
            Err(_) => {
                error!("Failed to load image for resolution check");
                return (
                    StatusCode::BAD_REQUEST,
                    "Failed to load image for resolution check".to_string(),
                )
                    .into_response();
            }
        }
    }

    let watermark_bytes = if let Some(url) = &parsed_options.watermark_url {
        debug!("Fetching watermark from URL: {}", url);
        match fetch_image(url).await {
            Ok((bytes, _)) => Some(bytes),
            Err(e) => {
                error!("Failed to fetch watermark image: {}", e);
                return (StatusCode::BAD_REQUEST, "Failed to fetch watermark image".to_string()).into_response();
            }
        }
    } else if let Ok(path) = env::var(ENV_WATERMARK_PATH) {
        debug!("Loading watermark from path: {}", path);
        match tokio::fs::read(path).await {
            Ok(bytes) => Some(Bytes::from(bytes)),
            Err(e) => {
                error!("Failed to read watermark image from path: {}", e);
                return (
                    StatusCode::BAD_REQUEST,
                    "Failed to read watermark image from path".to_string(),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    let _permit = if parsed_options.raw {
        None
    } else {
        Some(state.semaphore.acquire().await.unwrap())
    };

    // Get the output format before processing
    let output_format = parsed_options.format.clone().unwrap_or_else(|| "jpeg".to_string());

    let processed_image_bytes = match process_image(image_bytes.into(), parsed_options, watermark_bytes.as_ref()).await
    {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Error processing image: {}", e);
            return (StatusCode::BAD_REQUEST, format!("Error processing image: {}", e)).into_response();
        }
    };

    if !matches!(state.cache, Cache::None) {
        if let Err(e) = state.cache.insert(path.clone(), processed_image_bytes.clone()).await {
            error!("Failed to cache image: {}", e);
        }
    }

    // Set the content-type header based on the output format
    let content_type = format_to_content_type(&output_format);
    let mut headers = header::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());

    info!(
        "Imgforge processed path={} output_format={} bytes={}",
        path,
        output_format,
        processed_image_bytes.len()
    );

    (StatusCode::OK, headers, processed_image_bytes).into_response()
}

/// Performs common setup steps for image handling
async fn common_image_setup(
    path: &str,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
    config: &Config,
) -> Result<(ImgforgeUrl, String, Bytes, Option<String>), Response> {
    if let Some(secret) = &config.secret {
        if !secret.is_empty() {
            if let Some(TypedHeader(auth)) = auth_header {
                if auth.token() != secret {
                    error!("Invalid authorization token");
                    return Err((StatusCode::FORBIDDEN, "Invalid authorization token".to_string()).into_response());
                }
            } else {
                error!("Missing authorization token");
                return Err((StatusCode::FORBIDDEN, "Missing authorization token".to_string()).into_response());
            }
        }
    }

    let url_parts = match parse_path(path) {
        Some(parts) => parts,
        None => {
            error!("Invalid URL format: {}", path);
            return Err((StatusCode::BAD_REQUEST, "Invalid URL format".to_string()).into_response());
        }
    };

    if url_parts.signature == "unsafe" {
        if !config.allow_unsigned {
            error!("Unsigned URLs are not allowed");
            return Err((StatusCode::FORBIDDEN, "Unsigned URLs are not allowed".to_string()).into_response());
        }
    } else {
        let path_to_sign = format!("/{}", &path[path.find('/').unwrap() + 1..]);
        if !validate_signature(&config.key, &config.salt, &url_parts.signature, &path_to_sign) {
            error!("Invalid signature for path: {}", path_to_sign);
            return Err((StatusCode::FORBIDDEN, "Invalid signature".to_string()).into_response());
        }
    }

    let decoded_url = match url_parts.source_url.decode() {
        Ok(url) => url,
        Err(e) => {
            error!("Error decoding URL: {}", e);
            return Err((StatusCode::BAD_REQUEST, format!("Error decoding URL: {}", e)).into_response());
        }
    };

    let (image_bytes, content_type) = match fetch_image(&decoded_url).await {
        Ok(data) => data,
        Err(e) => {
            error!("Error fetching image: {}", e);
            return Err((StatusCode::BAD_REQUEST, format!("Error fetching image: {}", e)).into_response());
        }
    };

    Ok((url_parts, decoded_url, image_bytes, content_type))
}
