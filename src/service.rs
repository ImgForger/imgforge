use crate::app::AppState;
use crate::caching::cache::{CachedImage, ImgforgeCache};
use crate::fetch::fetch_image;
use crate::processing::options::{parse_all_options, ParsedOptions};
use crate::processing::presets::expand_presets;
use crate::processing::process_image;
use crate::url::{parse_path, validate_signature, ImgforgeUrl};
use crate::utils::format_to_content_type;
use axum::http::StatusCode;
use bytes::Bytes;
use libvips::VipsImage;
use std::error::Error;
use std::fmt::Display;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, error, info};

/// Indicates whether the response was served from cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    Hit,
    Miss,
}

impl CacheStatus {
    pub fn as_header_value(&self) -> &'static str {
        match self {
            CacheStatus::Hit => "HIT",
            CacheStatus::Miss => "MISS",
        }
    }
}

/// Result of processing an image request.
pub struct ProcessedImage {
    pub bytes: Bytes,
    pub content_type: &'static str,
    pub cache_status: CacheStatus,
}

/// Result of fetching image metadata.
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
}

/// Request context for processing or info retrieval.
pub struct ProcessRequest<'a> {
    pub path: &'a str,
    pub bearer_token: Option<&'a str>,
}

#[derive(Debug)]
pub struct ServiceError {
    status: StatusCode,
    message: String,
}

impl ServiceError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ServiceError {}

/// Process an imgproxy-compatible path using the provided application state.
pub async fn process_path(state: Arc<AppState>, request: ProcessRequest<'_>) -> Result<ProcessedImage, ServiceError> {
    let config = &state.config;
    let path = request.path;

    info!("Imgforge request received path={}", path);

    let url_parts = parse_and_authorize(config, path, request.bearer_token)?;

    if let Some(cached_image) = state.cache.get(path).await {
        debug!("Image found in cache for path={}", path);

        return Ok(ProcessedImage {
            bytes: cached_image.bytes,
            content_type: cached_image.content_type,
            cache_status: CacheStatus::Hit,
        });
    }

    let decoded_url = url_parts.source_url.decode().map_err(|e| {
        error!("Error decoding URL: {}", e);
        ServiceError::new(StatusCode::BAD_REQUEST, format!("Error decoding URL: {}", e))
    })?;

    let expanded_options = expand_presets(
        url_parts.processing_options.clone(),
        &config.presets,
        config.only_presets,
    )
    .map_err(|e| {
        error!("Error expanding presets: {}", e);
        ServiceError::new(StatusCode::BAD_REQUEST, e)
    })?;

    debug!("Processing image forge request for URL: {}", decoded_url);

    let parsed_options = parse_all_options(expanded_options).map_err(|e| {
        error!("Error parsing processing options: {}", e);
        ServiceError::new(StatusCode::BAD_REQUEST, e)
    })?;

    let (image_bytes, source_content_type) = fetch_image(&state.http_client, &decoded_url).await.map_err(|e| {
        error!("Error fetching image: {}", e);
        ServiceError::new(StatusCode::BAD_REQUEST, format!("Error fetching image: {}", e))
    })?;

    debug!(
        "Source image MIME type: {:?}, size: {} bytes",
        source_content_type,
        image_bytes.len()
    );

    let watermark_bytes = resolve_watermark(&parsed_options, &state.config, &state.http_client).await?;

    let _permit = if parsed_options.raw {
        None
    } else {
        Some(
            state
                .semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "Semaphore closed"))?,
        )
    };

    let output_format = parsed_options.format.clone().unwrap_or_else(|| "jpeg".to_string());

    let processed_image_bytes = {
        let source_image = VipsImage::new_from_buffer(&image_bytes, "").map_err(|e| {
            let response = format!("Error loading image from memory: {}", e);
            error!("{}", response);
            ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, response)
        })?;

        enforce_security_constraints(
            state.as_ref(),
            &parsed_options,
            &image_bytes,
            source_content_type.as_deref(),
            Some(&source_image),
        )?;

        process_image(source_image, parsed_options, &image_bytes, watermark_bytes.as_ref()).map_err(|e| {
            error!("Error processing image: {}", e);
            ServiceError::new(StatusCode::BAD_REQUEST, format!("Error processing image: {}", e))
        })?
    };

    let content_type = format_to_content_type(&output_format);
    if !matches!(state.cache, ImgforgeCache::None) {
        if let Err(err) = state
            .cache
            .insert(
                path.to_string(),
                CachedImage {
                    bytes: processed_image_bytes.clone(),
                    content_type,
                },
            )
            .await
        {
            error!("Failed to cache image: {}", err);
        }
    }

    info!(
        "Imgforge processed path={} output_format={} bytes={}",
        path,
        output_format,
        processed_image_bytes.len()
    );

    Ok(ProcessedImage {
        bytes: processed_image_bytes,
        content_type,
        cache_status: CacheStatus::Miss,
    })
}

/// Retrieve metadata for an image without processing it.
pub async fn image_info(state: Arc<AppState>, request: ProcessRequest<'_>) -> Result<ImageInfo, ServiceError> {
    let config = &state.config;
    let path = request.path;

    debug!("Info path captured: {}", path);
    let url_parts = parse_and_authorize(config, path, request.bearer_token)?;

    let decoded_url = url_parts.source_url.decode().map_err(|e| {
        error!("Error decoding URL: {}", e);
        ServiceError::new(StatusCode::BAD_REQUEST, format!("Error decoding URL: {}", e))
    })?;

    let (image_bytes, _content_type) = crate::fetch::fetch_image(&state.http_client, &decoded_url)
        .await
        .map_err(|e| {
            error!("Error fetching image: {}", e);
            ServiceError::new(StatusCode::BAD_REQUEST, format!("Error fetching image: {}", e))
        })?;

    let (width, height, image_format) = match VipsImage::new_from_buffer(&image_bytes, "") {
        Ok(img) => {
            let format_str = "unknown";
            (img.get_width() as u32, img.get_height() as u32, format_str.to_string())
        }
        Err(_) => (0, 0, "unknown".to_string()),
    };

    info!(
        "Imgforge info served path={} width={} height={} format={}",
        path, width, height, image_format
    );

    Ok(ImageInfo {
        width,
        height,
        format: image_format,
    })
}

fn parse_and_authorize(
    config: &crate::config::Config,
    path: &str,
    bearer_token: Option<&str>,
) -> Result<ImgforgeUrl, ServiceError> {
    if let Some(secret) = config.secret.as_ref() {
        if !secret.is_empty() {
            match bearer_token {
                Some(token) if token == secret => {}
                Some(_) => {
                    error!("Invalid authorization token");
                    return Err(ServiceError::new(StatusCode::FORBIDDEN, "Invalid authorization token"));
                }
                None => {
                    error!("Missing authorization token");
                    return Err(ServiceError::new(StatusCode::FORBIDDEN, "Missing authorization token"));
                }
            }
        }
    }

    let url_parts = parse_path(path).ok_or_else(|| {
        error!("Invalid URL format: {}", path);
        ServiceError::new(StatusCode::BAD_REQUEST, "Invalid URL format")
    })?;

    if url_parts.signature == "unsafe" {
        if !config.allow_unsigned {
            error!("Unsigned URLs are not allowed");
            return Err(ServiceError::new(
                StatusCode::FORBIDDEN,
                "Unsigned URLs are not allowed",
            ));
        }
    } else {
        let path_to_sign = build_path_to_sign(path).ok_or_else(|| {
            error!("Invalid URL format: {}", path);
            ServiceError::new(StatusCode::BAD_REQUEST, "Invalid URL format")
        })?;
        if !validate_signature(&config.key, &config.salt, &url_parts.signature, &path_to_sign) {
            error!("Invalid signature for path: {}", path_to_sign);
            return Err(ServiceError::new(StatusCode::FORBIDDEN, "Invalid signature"));
        }
    }

    Ok(url_parts)
}

fn build_path_to_sign(path: &str) -> Option<String> {
    path.find('/').map(|idx| format!("/{}", &path[idx + 1..]))
}

fn enforce_security_constraints(
    state: &AppState,
    parsed_options: &ParsedOptions,
    image_bytes: &Bytes,
    source_content_type: Option<&str>,
    decoded_image: Option<&VipsImage>,
) -> Result<(), ServiceError> {
    let config = &state.config;

    let max_src_file_size = if config.allow_security_options {
        parsed_options.max_src_file_size.or(config.max_src_file_size)
    } else {
        config.max_src_file_size
    };

    if let Some(max_size) = max_src_file_size {
        if image_bytes.len() > max_size {
            error!("Source image file size is too large");
            return Err(ServiceError::new(
                StatusCode::BAD_REQUEST,
                "Source image file size is too large",
            ));
        }
    }

    if let Some(allowed_types) = &config.allowed_mime_types {
        if let Some(content_type) = source_content_type {
            if !allowed_types.contains(&content_type.to_string()) {
                error!("Source image MIME type is not allowed: {}", content_type);
                return Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "Source image MIME type is not allowed",
                ));
            }
        }
    }

    let max_src_resolution = if config.allow_security_options {
        parsed_options.max_src_resolution.or(config.max_src_resolution)
    } else {
        config.max_src_resolution
    };

    if let Some(max_res) = max_src_resolution {
        let (w, h) = match decoded_image {
            Some(img) => (img.get_width(), img.get_height()),
            None => {
                error!("Failed to load image for resolution check");
                return Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "Failed to load image for resolution check",
                ));
            }
        };
        debug!("Image resolution: {}x{}", w, h);
        let res_mp = (w * h) as f32 / 1_000_000.0;
        if res_mp > max_res {
            error!("Source image resolution is too large");
            return Err(ServiceError::new(
                StatusCode::BAD_REQUEST,
                "Source image resolution is too large",
            ));
        }
    }

    Ok(())
}

async fn resolve_watermark(
    parsed_options: &ParsedOptions,
    config: &crate::config::Config,
    client: &reqwest::Client,
) -> Result<Option<Bytes>, ServiceError> {
    if let Some(url) = &parsed_options.watermark_url {
        debug!("Fetching watermark from URL: {}", url);
        match crate::fetch::fetch_image(client, url).await {
            Ok((bytes, _)) => Ok(Some(bytes)),
            Err(e) => {
                error!("Failed to fetch watermark image: {}", e);
                Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "Failed to fetch watermark image",
                ))
            }
        }
    } else if let Some(path) = &config.watermark_path {
        debug!("Loading watermark from path: {}", path);
        match fs::read(path).await {
            Ok(bytes) => Ok(Some(Bytes::from(bytes))),
            Err(e) => {
                error!("Failed to read watermark image from path: {}", e);
                Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "Failed to read watermark image from path",
                ))
            }
        }
    } else {
        Ok(None)
    }
}
