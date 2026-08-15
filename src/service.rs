use crate::app::AppState;
use crate::caching::cache::{CachedImage, CachedMetadata, ImgforgeCache, MetadataCache};
use crate::config::DefaultOutputFormat;
use crate::fetch::{fetch_image, FetchError};
use crate::limits::{MaxResultDimension, MaxSourceFileSize, MaxSourceResolution};
use crate::monitoring::{ImageOperation, ImageOperationActivityGuard, ImageOperationPhase, ImageOperationTimer};
use crate::processing::options::{parse_all_options, OptionParseError, ParsedOptions};
use crate::processing::presets::{expand_presets, PresetError};
use crate::processing::save::SaveError;
use crate::processing::watermark::{self, CachedWatermark};
use crate::processing::{process_image, ProcessingError};
use crate::url::{parse_path, validate_signature, ImgforgeUrl, SourceUrlDecodeError};
use crate::utils::{content_type_to_format, format_to_content_type, read_exif_orientation};
use axum::http::StatusCode;
use bytes::Bytes;
use libvips::VipsImage;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
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
    pub content_disposition: Option<String>,
}

/// Result of fetching image metadata.
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub content_type: Option<String>,
    pub size_bytes: usize,
    pub channels: u32,
    pub has_alpha: bool,
    pub orientation: Option<u32>,
}

/// Request context for processing or info retrieval.
pub struct ProcessRequest<'a> {
    pub path: &'a str,
    pub bearer_token: Option<&'a str>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ServiceError {
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("failed to fetch watermark image")]
    WatermarkFetch {
        #[source]
        source: FetchError,
    },
    #[error(transparent)]
    Preset(#[from] PresetError),
    #[error(transparent)]
    OptionParse(#[from] OptionParseError),
    #[error(transparent)]
    SourceUrlDecode(#[from] SourceUrlDecodeError),
    #[error(transparent)]
    Processing(#[from] ProcessingError),
    #[error("failed to decode source image")]
    SourceImageDecode {
        #[source]
        source: libvips::error::Error,
    },
    #[error("{operation} blocking task failed")]
    BlockingTask {
        operation: &'static str,
        #[source]
        source: tokio::task::JoinError,
    },
    #[error("{message}")]
    Response { status: StatusCode, message: String },
}

impl ServiceError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self::Response {
            status,
            message: message.into(),
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::Fetch(_)
            | Self::WatermarkFetch { .. }
            | Self::Preset(_)
            | Self::OptionParse(_)
            | Self::SourceUrlDecode(_)
            | Self::SourceImageDecode { .. } => StatusCode::BAD_REQUEST,
            Self::Processing(ProcessingError::Save(SaveError::Vips { .. } | SaveError::EncoderPanicked { .. })) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::BlockingTask { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Processing(_) => StatusCode::BAD_REQUEST,
            Self::Response { status, .. } => *status,
        }
    }

    pub fn message(&self) -> Cow<'_, str> {
        match self {
            Self::Fetch(FetchError::Request(_)) => Cow::Borrowed("Error fetching image"),
            Self::Fetch(FetchError::ResponseBody(_)) => Cow::Borrowed("Error reading image bytes"),
            Self::Fetch(FetchError::SourceTooLarge { limit, .. }) => Cow::Owned(format!(
                "Source image exceeds the maximum allowed size of {limit} bytes"
            )),
            Self::WatermarkFetch { .. } => Cow::Borrowed("Failed to fetch watermark image"),
            Self::Preset(error) => Cow::Owned(error.to_string()),
            Self::OptionParse(error) => Cow::Owned(error.to_string()),
            Self::SourceUrlDecode(_) => Cow::Borrowed("Error decoding URL"),
            Self::Processing(ProcessingError::Save(SaveError::UnsupportedFormat { format })) => {
                Cow::Owned(format!("Unsupported output format: {format}"))
            }
            Self::Processing(ProcessingError::Save(_)) => Cow::Borrowed("Failed to encode image"),
            Self::Processing(ProcessingError::ResultTooLarge { width, height, limit }) => Cow::Owned(format!(
                "Processed image would be {width}x{height}, over the {limit}px result dimension limit"
            )),
            Self::Processing(_) => Cow::Borrowed("Error processing image"),
            Self::SourceImageDecode { .. } => Cow::Borrowed("Failed to decode source image"),
            Self::BlockingTask { .. } => Cow::Borrowed("Image operation failed"),
            Self::Response { message, .. } => Cow::Borrowed(message),
        }
    }
}

/// Default output format when the URL requests none (#45): the source
/// image's format (imgproxy-compatible — a transparent PNG stays a PNG
/// instead of being flattened to JPEG), or a fixed format when
/// IMGFORGE_DEFAULT_FORMAT names one. Returns None (-> JPEG fallback)
/// when the source can't be sniffed or this build can't encode it.
fn default_output_format(configured: DefaultOutputFormat, image_bytes: &[u8]) -> Option<&'static str> {
    if let Some(format) = configured.fixed_format() {
        return Some(format);
    }

    sniff_image_format(image_bytes).filter(|format| crate::processing::save::is_format_supported(format))
}

fn processed_cache_key<'a>(
    path: &'a str,
    configured: DefaultOutputFormat,
    has_explicit_format: bool,
    is_raw: bool,
    max_result_dimension: Option<MaxResultDimension>,
) -> Cow<'a, str> {
    let base = if has_explicit_format || is_raw {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(format!("default-format={}:{}", configured.as_str(), path))
    };

    // A persistent cache outlives the configuration that filled it, so an entry
    // stored before the ceiling existed — or under a higher one — would other-
    // wise still be served, handing back the oversized image the limit exists
    // to refuse. Namespacing by the effective limit retires those entries.
    // Keys are left untouched when no limit applies, so enabling this feature
    // does not invalidate an existing cache.
    match max_result_dimension {
        Some(limit) => Cow::Owned(format!("mrd={}:{}", limit.get(), base)),
        None => base,
    }
}

fn detect_image_format(content_type: Option<&str>, image_bytes: &[u8]) -> String {
    if let Some(format) = content_type.and_then(content_type_to_format) {
        return format.to_string();
    }

    sniff_image_format(image_bytes).unwrap_or("unknown").to_string()
}

fn sniff_image_format(image_bytes: &[u8]) -> Option<&'static str> {
    if image_bytes.len() >= 3 && image_bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpeg");
    }

    if image_bytes.len() >= 8 && image_bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }

    if image_bytes.len() >= 6 && (image_bytes.starts_with(b"GIF87a") || image_bytes.starts_with(b"GIF89a")) {
        return Some("gif");
    }

    if image_bytes.len() >= 12 && image_bytes.starts_with(b"RIFF") && &image_bytes[8..12] == b"WEBP" {
        return Some("webp");
    }

    if image_bytes.len() >= 4 && (image_bytes.starts_with(b"II*\0") || image_bytes.starts_with(b"MM\0*")) {
        return Some("tiff");
    }

    if image_bytes.len() >= 12
        && image_bytes[4..8] == *b"ftyp"
        && matches!(
            &image_bytes[8..12],
            b"avif" | b"avis" | b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1"
        )
    {
        let brand = &image_bytes[8..12];
        return Some(if brand == b"avif" || brand == b"avis" {
            "avif"
        } else {
            "heif"
        });
    }

    None
}

fn image_has_alpha(channels: u32) -> bool {
    matches!(channels, 2 | 4)
}

/// Process an imgproxy-compatible path using the provided application state.
pub async fn process_path(state: Arc<AppState>, request: ProcessRequest<'_>) -> Result<ProcessedImage, ServiceError> {
    let config = &state.config;
    let path = request.path;

    info!("Imgforge request received path={}", path);

    let url_parts = parse_and_authorize(config, path, request.bearer_token)?;

    let expanded_options = expand_presets(
        url_parts.processing_options.clone(),
        &config.presets,
        config.only_presets,
    )?;

    let mut parsed_options = parse_all_options(expanded_options)?;

    enforce_expiration(&parsed_options)?;

    // Resolved before the cache lookup: the key depends on it.
    parsed_options.max_result_dimension = resolve_max_result_dimension(config, &parsed_options);

    let cache_key = processed_cache_key(
        path,
        config.default_format,
        parsed_options.format.is_some(),
        parsed_options.raw,
        parsed_options.max_result_dimension,
    );

    if let Some(cached_image) = state.cache.get(cache_key.as_ref()).await {
        debug!("Image found in cache for path={}", path);

        return Ok(ProcessedImage {
            bytes: cached_image.bytes,
            content_type: cached_image.content_type,
            cache_status: CacheStatus::Hit,
            content_disposition: None,
        });
    }

    let content_disposition = content_disposition_for(&parsed_options);

    let decoded_url = url_parts.source_url.decode()?;

    debug!("Processing image forge request for URL: {}", decoded_url);

    let max_src_file_size = resolve_max_src_file_size(config, &parsed_options).map(MaxSourceFileSize::get);
    let (image_bytes, source_content_type) = fetch_image(&state.http_client, &decoded_url, max_src_file_size).await?;

    debug!(
        "Source image MIME type: {:?}, size: {} bytes",
        source_content_type,
        image_bytes.len()
    );

    if parsed_options.raw {
        return serve_raw_response(
            state.as_ref(),
            path,
            image_bytes,
            source_content_type,
            content_disposition,
        )
        .await;
    }

    let watermark = if needs_watermark(&parsed_options) {
        resolve_watermark(state.as_ref(), &parsed_options).await?
    } else {
        None
    };

    let waiting = ImageOperationActivityGuard::waiting(ImageOperation::Process);
    let semaphore_wait = ImageOperationTimer::start(ImageOperation::Process, ImageOperationPhase::SemaphoreWait);
    let permit = state
        .semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "Semaphore closed"))?;
    drop(semaphore_wait);

    let blocking_state = state.clone();
    let span = tracing::Span::current();
    let blocking_queue = ImageOperationTimer::start(ImageOperation::Process, ImageOperationPhase::BlockingQueue);
    let (processed_image_bytes, output_format) = tokio::task::spawn_blocking(move || {
        drop(blocking_queue);
        drop(waiting);
        let _active = ImageOperationActivityGuard::active(ImageOperation::Process);
        let _execution = ImageOperationTimer::start(ImageOperation::Process, ImageOperationPhase::Execution);
        let _span_guard = span.enter();
        // Keep the concurrency slot until the blocking operation actually ends,
        // even if the async request future is cancelled while awaiting it.
        let _permit = permit;

        if parsed_options.format.is_none() {
            parsed_options.format =
                default_output_format(blocking_state.config.default_format, &image_bytes).map(str::to_owned);
        }
        let output_format = parsed_options.format.clone().unwrap_or_else(|| "jpeg".to_string());

        let source_image = VipsImage::new_from_buffer(&image_bytes, "")
            .map_err(|source| ServiceError::SourceImageDecode { source })?;

        enforce_security_constraints(
            blocking_state.as_ref(),
            &parsed_options,
            &image_bytes,
            source_content_type.as_deref(),
            Some(&source_image),
        )?;

        let processed_image_bytes = process_image(source_image, parsed_options, &image_bytes, watermark.as_ref())?;
        Ok::<_, ServiceError>((processed_image_bytes, output_format))
    })
    .await
    .map_err(|source| ServiceError::BlockingTask {
        operation: "image processing",
        source,
    })??;

    let content_type = format_to_content_type(&output_format);
    if content_disposition.is_none() && !matches!(state.cache, ImgforgeCache::None) {
        if let Err(err) = state.cache.insert(
            cache_key.into_owned(),
            CachedImage {
                bytes: processed_image_bytes.clone(),
                content_type,
            },
        ) {
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
        content_disposition,
    })
}

/// Retrieve metadata for an image without processing it.
pub async fn image_info(state: Arc<AppState>, request: ProcessRequest<'_>) -> Result<ImageInfo, ServiceError> {
    let config = &state.config;
    let path = request.path;

    debug!("Info path captured: {}", path);
    let url_parts = parse_and_authorize(config, path, request.bearer_token)?;

    if let Some(cached_metadata) = state.metadata_cache.get(path).await {
        debug!("Metadata found in cache for path={}", path);
        return Ok(ImageInfo {
            width: cached_metadata.width,
            height: cached_metadata.height,
            format: cached_metadata.format,
            content_type: (!cached_metadata.content_type.is_empty()).then_some(cached_metadata.content_type),
            size_bytes: cached_metadata.size_bytes,
            channels: cached_metadata.channels,
            has_alpha: cached_metadata.has_alpha,
            orientation: (cached_metadata.orientation != 0).then_some(cached_metadata.orientation),
        });
    }

    let decoded_url = url_parts.source_url.decode()?;

    let (image_bytes, content_type) = fetch_image(&state.http_client, &decoded_url, None).await?;

    let waiting = ImageOperationActivityGuard::waiting(ImageOperation::Info);
    let semaphore_wait = ImageOperationTimer::start(ImageOperation::Info, ImageOperationPhase::SemaphoreWait);
    let permit = state
        .semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "Semaphore closed"))?;
    drop(semaphore_wait);
    let info_content_type = content_type.clone();
    let span = tracing::Span::current();
    let blocking_queue = ImageOperationTimer::start(ImageOperation::Info, ImageOperationPhase::BlockingQueue);
    let (width, height, image_format, channels, has_alpha, orientation, cacheable, size_bytes) =
        tokio::task::spawn_blocking(move || {
            drop(blocking_queue);
            drop(waiting);
            let _active = ImageOperationActivityGuard::active(ImageOperation::Info);
            let _execution = ImageOperationTimer::start(ImageOperation::Info, ImageOperationPhase::Execution);
            let _span_guard = span.enter();
            let _permit = permit;
            let size_bytes = image_bytes.len();

            match VipsImage::new_from_buffer(&image_bytes, "") {
                Ok(img) => {
                    let format_str = detect_image_format(info_content_type.as_deref(), &image_bytes);
                    let channels = img.get_bands() as u32;
                    (
                        img.get_width() as u32,
                        img.get_height() as u32,
                        format_str,
                        channels,
                        image_has_alpha(channels),
                        read_exif_orientation(&image_bytes),
                        true,
                        size_bytes,
                    )
                }
                Err(err) => {
                    error!("Failed to decode image for info: {}", err);
                    (0, 0, "unknown".to_string(), 0, false, None, false, size_bytes)
                }
            }
        })
        .await
        .map_err(|source| ServiceError::BlockingTask {
            operation: "image metadata",
            source,
        })?;

    let metadata = CachedMetadata {
        width,
        height,
        format: image_format.clone(),
        content_type: content_type.clone().unwrap_or_default(),
        size_bytes,
        channels,
        has_alpha,
        orientation: orientation.unwrap_or(0),
    };

    if cacheable && !matches!(state.metadata_cache, MetadataCache::None) {
        if let Err(err) = state.metadata_cache.insert(path.to_string(), metadata) {
            error!("Failed to cache metadata: {}", err);
        }
    }

    info!(
        "Imgforge info served path={} width={} height={} format={} size_bytes={} channels={} has_alpha={} orientation={:?}",
        path, width, height, image_format, size_bytes, channels, has_alpha, orientation
    );

    Ok(ImageInfo {
        width,
        height,
        format: image_format,
        content_type,
        size_bytes,
        channels,
        has_alpha,
        orientation,
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

    let max_src_file_size = resolve_max_src_file_size(config, parsed_options);

    if let Some(max_size) = max_src_file_size {
        if image_bytes.len() > max_size.get() {
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

    let max_src_resolution = resolve_max_src_resolution(config, parsed_options);

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
        let source_pixels = checked_source_pixel_count(w, h)?;
        if source_pixels > max_res.pixels() {
            error!("Source image resolution is too large");
            return Err(ServiceError::new(
                StatusCode::BAD_REQUEST,
                "Source image resolution is too large",
            ));
        }
    }

    Ok(())
}

fn checked_source_pixel_count(width: i32, height: i32) -> Result<u64, ServiceError> {
    let width = u64::try_from(width)
        .map_err(|_| ServiceError::new(StatusCode::BAD_REQUEST, "Invalid source image dimensions"))?;
    let height = u64::try_from(height)
        .map_err(|_| ServiceError::new(StatusCode::BAD_REQUEST, "Invalid source image dimensions"))?;

    width
        .checked_mul(height)
        .ok_or_else(|| ServiceError::new(StatusCode::BAD_REQUEST, "Source image resolution is too large"))
}

fn resolve_max_src_file_size(
    config: &crate::config::Config,
    parsed_options: &ParsedOptions,
) -> Option<MaxSourceFileSize> {
    if config.allow_security_options {
        parsed_options.max_src_file_size.or(config.max_src_file_size)
    } else {
        config.max_src_file_size
    }
}

fn resolve_max_src_resolution(
    config: &crate::config::Config,
    parsed_options: &ParsedOptions,
) -> Option<MaxSourceResolution> {
    if config.allow_security_options {
        parsed_options.max_src_resolution.or(config.max_src_resolution)
    } else {
        config.max_src_resolution
    }
}

fn resolve_max_result_dimension(
    config: &crate::config::Config,
    parsed_options: &ParsedOptions,
) -> Option<MaxResultDimension> {
    if config.allow_security_options {
        parsed_options.max_result_dimension.or(config.max_result_dimension)
    } else {
        config.max_result_dimension
    }
}

fn needs_watermark(parsed_options: &ParsedOptions) -> bool {
    parsed_options.watermark.is_some() || parsed_options.watermark_url.is_some()
}

async fn resolve_watermark(
    state: &AppState,
    parsed_options: &ParsedOptions,
) -> Result<Option<CachedWatermark>, ServiceError> {
    if let Some(url) = &parsed_options.watermark_url {
        debug!("Fetching watermark from URL: {}", url);
        match fetch_image(&state.http_client, url, None).await {
            Ok((bytes, _)) => Ok(Some(CachedWatermark::from_bytes(bytes))),
            Err(source) => Err(ServiceError::WatermarkFetch { source }),
        }
    } else if parsed_options.watermark.is_some() {
        if let Some(path) = &state.config.watermark_path {
            let watermark =
                state
                    .watermark_cache
                    .get_or_try_init(|| async {
                        debug!("Loading watermark from path: {} (cached on first load)", path);
                        let bytes = fs::read(path).await.map(Bytes::from).map_err(|e| {
                            error!("Failed to read watermark image from path: {}", e);
                            ServiceError::new(StatusCode::BAD_REQUEST, "Failed to read watermark image from path")
                        })?;

                        let waiting = ImageOperationActivityGuard::waiting(ImageOperation::Watermark);
                        let semaphore_wait =
                            ImageOperationTimer::start(ImageOperation::Watermark, ImageOperationPhase::SemaphoreWait);
                        let permit =
                            state.semaphore.clone().acquire_owned().await.map_err(|_| {
                                ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "Semaphore closed")
                            })?;
                        drop(semaphore_wait);
                        let span = tracing::Span::current();
                        let blocking_queue =
                            ImageOperationTimer::start(ImageOperation::Watermark, ImageOperationPhase::BlockingQueue);
                        tokio::task::spawn_blocking(move || {
                            drop(blocking_queue);
                            drop(waiting);
                            let _active = ImageOperationActivityGuard::active(ImageOperation::Watermark);
                            let _execution =
                                ImageOperationTimer::start(ImageOperation::Watermark, ImageOperationPhase::Execution);
                            let _span_guard = span.enter();
                            let _permit = permit;
                            watermark::prepare_cached_watermark(bytes)
                        })
                        .await
                        .map_err(|source| ServiceError::BlockingTask {
                            operation: "watermark preparation",
                            source,
                        })?
                        .map_err(ProcessingError::from)
                        .map_err(ServiceError::from)
                    })
                    .await?;
            Ok(Some(watermark.clone()))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

async fn serve_raw_response(
    state: &AppState,
    path: &str,
    image_bytes: Bytes,
    source_content_type: Option<String>,
    content_disposition: Option<String>,
) -> Result<ProcessedImage, ServiceError> {
    let content_type = source_content_type
        .as_deref()
        .map(format_to_content_type)
        .unwrap_or("image/jpeg");

    if content_disposition.is_none() && !matches!(state.cache, ImgforgeCache::None) {
        if let Err(err) = state.cache.insert(
            path.to_string(),
            CachedImage {
                bytes: image_bytes.clone(),
                content_type,
            },
        ) {
            error!("Failed to cache raw image: {}", err);
        }
    }

    info!("Imgforge served raw path={} bytes={}", path, image_bytes.len());

    Ok(ProcessedImage {
        bytes: image_bytes,
        content_type,
        cache_status: CacheStatus::Miss,
        content_disposition,
    })
}

fn enforce_expiration(parsed_options: &ParsedOptions) -> Result<(), ServiceError> {
    let Some(expires) = parsed_options.expires else {
        return Ok(());
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "system clock is before unix epoch"))?
        .as_secs();

    if now > expires {
        return Err(ServiceError::new(StatusCode::NOT_FOUND, "URL has expired"));
    }

    Ok(())
}

fn content_disposition_for(parsed_options: &ParsedOptions) -> Option<String> {
    let filename = parsed_options.filename.as_ref()?;
    let disposition = if parsed_options.return_attachment {
        "attachment"
    } else {
        "inline"
    };
    Some(format!(
        "{}; filename=\"{}\"",
        disposition,
        filename.replace(['\\', '"', '\r', '\n'], "_")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn cache_keys_are_namespaced_by_the_effective_result_limit() {
        let path = "/unsafe/resize:fit:4000:4000/example";
        let unlimited = processed_cache_key(path, DefaultOutputFormat::Source, false, false, None);
        let limited = processed_cache_key(
            path,
            DefaultOutputFormat::Source,
            false,
            false,
            Some("1000".parse().unwrap()),
        );
        let raised = processed_cache_key(
            path,
            DefaultOutputFormat::Source,
            false,
            false,
            Some("8192".parse().unwrap()),
        );

        // A disk cache outlives the config that filled it. Entries stored under
        // one ceiling must not be served under another, or a request that the
        // limit should refuse comes straight back out of the cache.
        assert_ne!(unlimited, limited);
        assert_ne!(limited, raised);

        // Turning the feature on must not invalidate caches that never use it.
        assert_eq!(
            unlimited,
            processed_cache_key(path, DefaultOutputFormat::Source, false, false, None)
        );
    }

    #[test]
    fn max_result_dimension_override_requires_security_options() {
        let request_limit = "1000".parse::<MaxResultDimension>().unwrap();
        let server_limit = "4000".parse::<MaxResultDimension>().unwrap();

        let parsed_options = ParsedOptions {
            max_result_dimension: Some(request_limit),
            ..ParsedOptions::default()
        };

        let mut config = crate::config::Config::new(vec![0u8; 32], vec![0u8; 32]);
        config.max_result_dimension = Some(server_limit);

        // Locked down: the URL cannot set its own ceiling, so the server's stands.
        config.allow_security_options = false;
        assert_eq!(
            resolve_max_result_dimension(&config, &parsed_options),
            Some(server_limit)
        );

        // Opted in: the request wins, matching how max_src_* already behave.
        config.allow_security_options = true;
        assert_eq!(
            resolve_max_result_dimension(&config, &parsed_options),
            Some(request_limit)
        );

        // No server limit and no opt-in means no ceiling at all.
        config.allow_security_options = false;
        config.max_result_dimension = None;
        assert_eq!(resolve_max_result_dimension(&config, &parsed_options), None);
    }

    #[test]
    fn fetch_size_error_has_centralized_http_mapping() {
        let error = ServiceError::from(FetchError::SourceTooLarge {
            limit: 1024,
            actual: Some(2048),
        });

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message(),
            "Source image exceeds the maximum allowed size of 1024 bytes"
        );
        assert!(matches!(
            error,
            ServiceError::Fetch(FetchError::SourceTooLarge {
                limit: 1024,
                actual: Some(2048)
            })
        ));
    }

    #[tokio::test]
    async fn fetch_request_error_does_not_expose_network_details() {
        let source = reqwest::Client::new()
            .get("not_a_valid_url")
            .send()
            .await
            .expect_err("invalid URL should fail");
        let error = ServiceError::from(FetchError::Request(source));

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.message(), "Error fetching image");
        assert!(error.source().is_some());
    }

    #[tokio::test]
    async fn blocking_task_failure_maps_to_internal_server_error() {
        let source = tokio::task::spawn_blocking(|| panic!("test blocking-task panic"))
            .await
            .expect_err("panicking task should return a join error");
        let error = ServiceError::BlockingTask {
            operation: "test image operation",
            source,
        };

        assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.message(), "Image operation failed");
        assert!(error.source().is_some());
    }

    #[test]
    fn option_parse_error_has_centralized_http_mapping() {
        let error = ServiceError::from(OptionParseError::InvalidValue(
            "quality option requires one argument".to_string(),
        ));

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.message(), "quality option requires one argument");
    }

    #[test]
    fn source_url_error_uses_safe_client_message() {
        use base64::Engine as _;

        let source = crate::url::SourceUrlInfo::Base64 {
            encoded_url: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0xff]),
        }
        .decode()
        .expect_err("invalid UTF-8 should fail");
        let error = ServiceError::from(source);

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.message(), "Error decoding URL");
        assert!(error.source().is_some());
    }

    #[test]
    fn processing_error_preserves_vips_source_and_uses_safe_client_message() {
        let transform_error = crate::processing::transform::TransformError::Vips {
            operation: "test resize",
            source: libvips::error::Error::ResizeError,
        };
        let error = ServiceError::from(ProcessingError::from(transform_error));

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.message(), "Error processing image");
        assert!(error.source().is_some());
    }

    #[test]
    fn encoder_failure_maps_to_internal_server_error() {
        let save_error = SaveError::Vips {
            format: "JPEG",
            source: libvips::error::Error::JpegsaveBufferError,
        };
        let error = ServiceError::from(ProcessingError::from(save_error));

        assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.message(), "Failed to encode image");
        assert!(error.source().is_some());
    }

    #[test]
    fn pixel_count_does_not_overflow_i32_sized_dimensions() {
        assert_eq!(checked_source_pixel_count(50_000, 50_000).unwrap(), 2_500_000_000);
    }

    #[test]
    fn pixel_count_rejects_negative_dimensions() {
        assert!(checked_source_pixel_count(-1, 100).is_err());
        assert!(checked_source_pixel_count(100, -1).is_err());
    }

    #[test]
    fn fixed_default_format_is_resolved_without_sniffing() {
        assert_eq!(
            default_output_format(DefaultOutputFormat::Jpeg, b"not an image"),
            Some("jpeg")
        );
        assert_eq!(
            default_output_format(DefaultOutputFormat::Heif, b"not an image"),
            Some("heif")
        );
    }

    #[test]
    fn implicit_format_cache_keys_include_the_configured_default() {
        let source_key = processed_cache_key("/unsafe/example", DefaultOutputFormat::Source, false, false, None);
        let jpeg_key = processed_cache_key("/unsafe/example", DefaultOutputFormat::Jpeg, false, false, None);
        let explicit_key = processed_cache_key(
            "/unsafe/format:png/example",
            DefaultOutputFormat::Jpeg,
            true,
            false,
            None,
        );

        assert_ne!(source_key, jpeg_key);
        assert_eq!(explicit_key, "/unsafe/format:png/example");
    }
}
