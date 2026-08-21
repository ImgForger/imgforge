//! Request handling between the HTTP layer and the processing pipeline.

pub mod cache_key;
pub mod error;
pub mod security;
pub mod source;

pub use error::ServiceError;

use crate::app::AppState;
use crate::caching::cache::{CachedImage, CachedMetadata, ImgforgeCache, MetadataCache};
use crate::config::{Config, DefaultOutputFormat};
use crate::fetch::{fetch_image, FetchedImage};
use crate::limits::MaxSourceFileSize;
use crate::monitoring::{ImageOperation, ImageOperationActivityGuard, ImageOperationPhase, ImageOperationTimer};
use crate::processing::metadata;
use crate::processing::options::{parse_all_options_with_defaults, OptionDefaults, ParsedOptions};
use crate::processing::presets::expand_presets;
use crate::processing::watermark::{self, CachedWatermark};
use crate::processing::{process_image, ProcessingError};
use crate::url::{parse_path, validate_signature, ImgforgeUrl};
use crate::utils::{format_to_content_type, read_exif_orientation};
use axum::http::StatusCode;
use bytes::Bytes;
use cache_key::CacheKeyParts;
use libvips::VipsImage;
use security::{
    apply_effective_limits, enforce_expiration, enforce_security_constraints, enforce_source_constraints,
    resolve_max_src_file_size, resolve_max_src_resolution,
};
use source::{
    can_skip_processing, detect_image_format, image_has_alpha, loader_options, shrink_source_on_load,
    sniff_image_format,
};
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
    pub pages: u32,
}

/// Request context for processing or info retrieval.
pub struct ProcessRequest<'a> {
    pub path: &'a str,
    pub bearer_token: Option<&'a str>,
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

    let mut parsed_options = parse_all_options_with_defaults(expanded_options, config.option_defaults())?;

    enforce_expiration(&parsed_options)?;

    // Resolved before the cache lookup: the key depends on the ceilings.
    apply_effective_limits(config, &mut parsed_options);

    let cache_key = cache_key::processed_cache_key(CacheKeyParts {
        path,
        default_format: config.default_format,
        has_explicit_format: parsed_options.format.is_some(),
        is_raw: parsed_options.raw,
        max_result_dimension: parsed_options.max_result_dimension,
        max_animation_frames: parsed_options.max_animation_frames,
        max_animation_frame_resolution: parsed_options.max_animation_frame_resolution,
        // The source ceilings decide whether these bytes may be served at all,
        // and every one of them is checked after this lookup — so without them
        // in the key a tightened policy is simply outrun by the cache.
        max_src_resolution: resolve_max_src_resolution(config, &parsed_options),
        max_src_file_size: resolve_max_src_file_size(config, &parsed_options),
        allowed_mime_types: config.allowed_mime_types.as_deref(),
        // Only when the request actually composites the server-side watermark:
        // a `watermark_url` brings its own image, and a request using neither is
        // unaffected by the setting.
        watermark_path: config
            .watermark_path
            .as_deref()
            .filter(|_| parsed_options.watermark.is_some() && parsed_options.watermark_url.is_none()),
        // Only when the deployment changes them, so a default configuration
        // keeps the keys it already had.
        option_defaults: Some(config.option_defaults()).filter(|defaults| *defaults != OptionDefaults::default()),
        negotiated_format: None,
    });

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
    let fetched = fetch_image(&state.http_client, &decoded_url, max_src_file_size).await?;
    let FetchedImage {
        bytes: image_bytes,
        content_type: source_content_type,
        ..
    } = fetched;

    debug!(
        "Source image MIME type: {:?}, size: {} bytes",
        source_content_type,
        image_bytes.len()
    );

    let source_format = sniff_image_format(&image_bytes);

    // `raw` returns the source untouched; `skip_processing` does the same for
    // the formats it names, which is how imgproxy keeps an already-optimised
    // asset from being re-encoded.
    if parsed_options.raw || can_skip_processing(&parsed_options, source_format, config.default_format) {
        // Skipping the pipeline is not a way around the limits that describe
        // the source. `allowed_mime_types` and `max_src_resolution` say what
        // this deployment is willing to serve, not merely what it is willing to
        // re-encode, so a URL that opts out of processing still has to satisfy
        // them before any bytes go back.
        enforce_source_constraints(
            state.as_ref(),
            &parsed_options,
            &image_bytes,
            source_content_type.as_deref(),
        )?;

        return serve_source_response(
            state.as_ref(),
            path,
            &cache_key,
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
        // Normalised once, here, because everything downstream keys off this
        // string — the response `Content-Type`, the `format_quality` lookup and
        // the metrics label. A URL naming an alias such as `format:tif` would
        // otherwise pick the right encoder and then be described by the wrong
        // MIME type, and be counted under a format name of its own.
        let output_format = parsed_options
            .format
            .as_deref()
            .and_then(crate::processing::save::canonical_format_name)
            .map(str::to_owned)
            .or_else(|| parsed_options.format.clone())
            .unwrap_or_else(|| "jpeg".to_string());
        parsed_options.format = Some(output_format.clone());

        let opened = open_source(&image_bytes, &parsed_options, &output_format)?;

        // Measured on the source as fetched, never on a substituted stand-in:
        // the ceilings say what this deployment will accept, and a 10000px
        // source is exactly as unacceptable when its thumbnail is small.
        enforce_security_constraints(
            blocking_state.as_ref(),
            &parsed_options,
            &opened.metadata_bytes,
            source_content_type.as_deref(),
            Some(opened.constraint_image()),
        )?;

        let OpenedSource {
            image: source_image,
            decode_bytes,
            metadata_bytes,
            ..
        } = opened;

        // Scale-on-load. The guards above must see the *original* dimensions,
        // so this comes after them: shrinking first would let a source sneak
        // past a resolution limit by arriving smaller than it really is.
        //
        // Opening an image does not decode it — libvips is demand-driven, so
        // everything so far has read the header and nothing more. Reopening
        // with a shrink means the full-resolution pixels are never unpacked at
        // all. Same ordering imgproxy uses: load, check, then scale on load.
        let load_options = loader_options(&parsed_options, sniff_image_format(&decode_bytes), &output_format);
        let source_image = shrink_source_on_load(
            source_image,
            &decode_bytes,
            &metadata_bytes,
            &mut parsed_options,
            &load_options,
        );

        let processed_image_bytes = process_image(source_image, parsed_options, &metadata_bytes, watermark.as_ref())?;
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

/// What opening the source produced: the image and the two byte views the
/// stages after it read.
struct OpenedSource {
    image: VipsImage,
    /// The source as opened for the ceiling checks, present when a stand-in
    /// occupies `image`. The ceilings describe the source as fetched, so they
    /// are measured on it — judging them on the stand-in let a source over
    /// `max_src_resolution` slip under the limit at its thumbnail's size.
    original: Option<VipsImage>,
    /// The bytes the pixels came from — the embedded thumbnail when it was
    /// substituted. Format sniffing and reduced-scale reopening have to see
    /// these, because they describe the image actually in hand.
    decode_bytes: Bytes,
    /// The bytes EXIF-driven behaviour reads — always the original source. A
    /// thumbnail carries no metadata of its own, so reading it made
    /// `keep_copyright` silently lose the source's copyright and auto-rotation
    /// lose the orientation that still applies to the thumbnail's pixels.
    metadata_bytes: Bytes,
}

impl OpenedSource {
    /// The image the source ceilings are measured on.
    fn constraint_image(&self) -> &VipsImage {
        self.original.as_ref().unwrap_or(&self.image)
    }
}

/// Opens the source, honouring `enforce_thumbnail` and the multi-page plan.
fn open_source(
    image_bytes: &Bytes,
    parsed_options: &ParsedOptions,
    output_format: &str,
) -> Result<OpenedSource, ServiceError> {
    if parsed_options.enforce_thumbnail {
        if let Some(thumbnail) = metadata::embedded_thumbnail(image_bytes) {
            if let Some(opened) = thumbnail_stand_in(image_bytes, Bytes::from(thumbnail), parsed_options) {
                return Ok(opened);
            }
        }
    }

    let load_options = loader_options(parsed_options, sniff_image_format(image_bytes), output_format);
    let image = VipsImage::new_from_buffer(image_bytes, &load_options)
        .map_err(|source| ServiceError::SourceImageDecode { source })?;

    Ok(OpenedSource {
        image,
        original: None,
        decode_bytes: image_bytes.clone(),
        metadata_bytes: image_bytes.clone(),
    })
}

/// The embedded thumbnail as a stand-in for the source, when it qualifies.
///
/// `None` means the full image should be opened instead — never a failed
/// request, because the full image is still there.
fn thumbnail_stand_in(image_bytes: &Bytes, thumbnail: Bytes, parsed_options: &ParsedOptions) -> Option<OpenedSource> {
    let img = match VipsImage::new_from_buffer(&thumbnail, "") {
        Ok(img) => img,
        Err(err) => {
            debug!("Embedded thumbnail did not decode ({}); using the full image", err);
            return None;
        }
    };

    // The gate below is written against what the viewer sees. The parent's
    // EXIF orientation applies to the thumbnail's pixels too — same scene,
    // same sensor — and orientations 5-8 transpose them after decoding, so a
    // stored 160x120 answers a portrait request as 120x160.
    let (width, height) = if source::swaps_axes(parsed_options, image_bytes) {
        (img.get_height(), img.get_width())
    } else {
        (img.get_width(), img.get_height())
    };

    // The stand-in is only taken when it covers what the request asks of it.
    // An undersized one used to be taken anyway, and `enlarge:false` then
    // capped a 1000px request at the 160px the thumbnail could provide.
    if !crate::processing::thumbnail_covers(parsed_options, width, height) {
        debug!(
            "Embedded thumbnail ({}x{}) cannot satisfy the request; using the full image",
            width, height
        );
        return None;
    }

    // The source itself still has to be open for the ceiling checks. One that
    // will not open cannot be measured against them, so it does not get to
    // stand behind a thumbnail that would pass.
    let original = match VipsImage::new_from_buffer(image_bytes, "") {
        Ok(original) => original,
        Err(err) => {
            debug!(
                "Source did not open for the ceiling checks ({}); using the full image",
                err
            );
            return None;
        }
    };

    debug!("Using the source's embedded thumbnail ({} bytes)", thumbnail.len());
    Some(OpenedSource {
        image: img,
        original: Some(original),
        decode_bytes: thumbnail,
        metadata_bytes: image_bytes.clone(),
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
            pages: cached_metadata.pages.max(1),
        });
    }

    let decoded_url = url_parts.source_url.decode()?;

    let fetched = fetch_image(&state.http_client, &decoded_url, None).await?;
    let image_bytes = fetched.bytes;
    let content_type = fetched.content_type;

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
    let (metadata, cacheable) = tokio::task::spawn_blocking(move || {
        drop(blocking_queue);
        drop(waiting);
        let _active = ImageOperationActivityGuard::active(ImageOperation::Info);
        let _execution = ImageOperationTimer::start(ImageOperation::Info, ImageOperationPhase::Execution);
        let _span_guard = span.enter();
        let _permit = permit;
        let size_bytes = image_bytes.len();

        // A multi-page source is opened whole so the reported page count is the
        // real one rather than the single page the default load returns.
        let load_options = match sniff_image_format(&image_bytes) {
            Some(format) if crate::processing::animation::supports_pages(format) => "n=-1",
            _ => "",
        };

        match VipsImage::new_from_buffer(&image_bytes, load_options) {
            Ok(img) => {
                let channels = img.get_bands() as u32;
                (
                    CachedMetadata {
                        width: img.get_width() as u32,
                        height: img.get_page_height().max(1) as u32,
                        format: detect_image_format(info_content_type.as_deref(), &image_bytes),
                        content_type: info_content_type.unwrap_or_default(),
                        size_bytes,
                        channels,
                        has_alpha: image_has_alpha(channels),
                        orientation: read_exif_orientation(&image_bytes).unwrap_or(0),
                        pages: img.get_n_pages().max(1) as u32,
                    },
                    true,
                )
            }
            Err(err) => {
                error!("Failed to decode image for info: {}", err);
                (
                    CachedMetadata {
                        format: "unknown".to_string(),
                        content_type: info_content_type.unwrap_or_default(),
                        size_bytes,
                        pages: 1,
                        ..CachedMetadata::default()
                    },
                    false,
                )
            }
        }
    })
    .await
    .map_err(|source| ServiceError::BlockingTask {
        operation: "image metadata",
        source,
    })?;

    if cacheable && !matches!(state.metadata_cache, MetadataCache::None) {
        if let Err(err) = state.metadata_cache.insert(path.to_string(), metadata.clone()) {
            error!("Failed to cache metadata: {}", err);
        }
    }

    info!(
        "Imgforge info served path={} width={} height={} format={} size_bytes={} channels={} has_alpha={} pages={}",
        path,
        metadata.width,
        metadata.height,
        metadata.format,
        metadata.size_bytes,
        metadata.channels,
        metadata.has_alpha,
        metadata.pages
    );

    Ok(ImageInfo {
        width: metadata.width,
        height: metadata.height,
        format: metadata.format,
        content_type: (!metadata.content_type.is_empty()).then_some(metadata.content_type),
        size_bytes: metadata.size_bytes,
        channels: metadata.channels,
        has_alpha: metadata.has_alpha,
        orientation: (metadata.orientation != 0).then_some(metadata.orientation),
        pages: metadata.pages.max(1),
    })
}

fn parse_and_authorize(config: &Config, path: &str, bearer_token: Option<&str>) -> Result<ImgforgeUrl, ServiceError> {
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
            Ok(fetched) => Ok(Some(CachedWatermark::from_bytes(fetched.bytes))),
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

/// Returns the source bytes as they arrived, for `raw` and `skip_processing`.
async fn serve_source_response(
    state: &AppState,
    path: &str,
    cache_key: &str,
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
            cache_key.to_string(),
            CachedImage {
                bytes: image_bytes.clone(),
                content_type,
            },
        ) {
            error!("Failed to cache raw image: {}", err);
        }
    }

    info!("Imgforge served source path={} bytes={}", path, image_bytes.len());

    Ok(ProcessedImage {
        bytes: image_bytes,
        content_type,
        cache_status: CacheStatus::Miss,
        content_disposition,
    })
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
mod tests;
