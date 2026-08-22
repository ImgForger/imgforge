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
use crate::negotiation::{apply_client_hints, negotiate_format, vary_headers, RequestHints};
use crate::processing::metadata;
use crate::processing::options::{parse_all_options_with_defaults, OptionDefaults, ParsedOptions};
use crate::processing::presets::expand_presets;
use crate::processing::watermark::{self, CachedWatermark};
use crate::processing::{process_image, ProcessingError};
use crate::response::{DeliveryHeaders, SourceMetadata};
use crate::url::{parse_path, validate_signature_of_size, ImgforgeUrl};
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
    /// Caching and provenance headers derived from the configuration.
    pub headers: DeliveryHeaders,
    /// What the source was and what it became, when debug headers are on.
    pub debug: Option<DebugInfo>,
}

/// Sizes worth reporting back when `IMGFORGE_ENABLE_DEBUG_HEADERS` is set.
///
/// Answers the question a cache-efficiency investigation always starts with:
/// how big was the thing we downloaded, and how big is the thing we sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DebugInfo {
    pub origin_bytes: usize,
    pub origin_width: u32,
    pub origin_height: u32,
    pub result_width: u32,
    pub result_height: u32,
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
    /// What the client's headers said it can accept and how large it needs it.
    pub hints: RequestHints,
}

impl<'a> ProcessRequest<'a> {
    /// A request carrying nothing but a path, for callers using imgforge as a
    /// library rather than over HTTP.
    pub fn new(path: &'a str) -> Self {
        Self {
            path,
            bearer_token: None,
            hints: RequestHints::default(),
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

    // Recorded before the hints are folded in, so the key can name exactly what
    // they contributed rather than what the URL already asked for.
    let client_hints = config.enable_client_hints.then(|| {
        apply_client_hints(&mut parsed_options, &request.hints);
        (
            parsed_options.resize.map(|resize| resize.width).unwrap_or(0),
            (parsed_options.dpr_factor() * 1000.0).round() as u32,
        )
    });

    // Negotiation can replace the output format, so the key has to record which
    // format this response was actually built for.
    let has_explicit_format = parsed_options.format.is_some();
    let negotiated_format = negotiate_format(config, &request.hints, has_explicit_format);
    if let Some(format) = negotiated_format {
        parsed_options.format = Some(format.to_string());
    }
    let vary = vary_headers(config);

    // Resolved before the cache lookup: a source the deployment no longer
    // permits must stop being served, and a persistent cache would otherwise
    // keep answering for it long after `IMGFORGE_ALLOWED_SOURCES` was tightened
    // — the request never reaches the check because it never reaches the fetch.
    let decoded_url = resolve_source_url(config, &url_parts)?;

    // The watermark's URL is part of the request too, so it is checked where
    // the main URL is checked — before the cache can answer. Validating it
    // only on the miss path let a cached composite keep serving pixels from a
    // host the allow list no longer names. The resolved form is kept: it is
    // part of the entry's identity for the same reason the main source's is.
    let watermark_url = match parsed_options.watermark_url.as_deref() {
        Some(url) => Some(permitted_url(config, url)?),
        None => None,
    };

    let cache_key = cache_key::processed_cache_key(CacheKeyParts {
        path,
        source_url: &decoded_url,
        default_format: config.default_format,
        has_explicit_format: has_explicit_format && negotiated_format.is_none(),
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
        watermark_url: watermark_url.as_deref(),
        // Only when the deployment changes them, so a default configuration
        // keeps the keys it already had.
        option_defaults: Some(config.option_defaults()).filter(|defaults| *defaults != OptionDefaults::default()),
        negotiated_format,
        client_hints,
    });

    // A hit is only usable if the source it came from is still permitted. The
    // request's own URL was checked above, but a redirect can have moved the
    // actual source elsewhere, and an entry outlives the policy that admitted
    // it. Treated as a miss rather than a refusal: the re-fetch follows the
    // redirect again and the redirect policy gives the accurate answer, which
    // is right whether the destination moved somewhere permitted or nowhere.
    let cached_image = state.cache.get(cache_key.as_ref()).await.filter(|cached| {
        let source_ok = cached.source_url.is_empty() || config.source_rules.permits(&cached.source_url);
        // The watermark's pixels are in the composite, so where they came from
        // counts exactly as much as where the image's own did.
        let watermark_ok =
            cached.watermark_source_url.is_empty() || config.source_rules.permits(&cached.watermark_source_url);
        let permitted = source_ok && watermark_ok;
        if !permitted {
            debug!("Ignoring a cached entry fetched from a source that is no longer allowed");
        }
        permitted
    });

    if let Some(cached_image) = cached_image {
        debug!("Image found in cache for path={}", path);

        // A cache hit has no source response to draw on, so the origin's own
        // caching headers are not available; the configured policy still is,
        // and the entity tag comes from the bytes either way.
        // The origin's delivery metadata was stored with the entry, so a hit
        // keeps saying what the origin said — a passthrough `no-store` must
        // not vanish the moment the cache starts answering.
        let cached_source = SourceMetadata {
            cache_control: (!cached_image.origin_cache_control.is_empty())
                .then(|| cached_image.origin_cache_control.clone()),
            last_modified: (!cached_image.origin_last_modified.is_empty())
                .then(|| cached_image.origin_last_modified.clone()),
            url: None,
        };
        let headers = DeliveryHeaders::for_cache_hit(config, &cached_source, &cached_image.etag, &vary);

        // Enabling the debug headers should not make them appear and disappear
        // with the cache. A hit never made the source request, so nothing about
        // the origin can be reported — but the result is right here, and its
        // header is cheap to read.
        let debug = config.enable_debug_headers.then(|| {
            let mut debug = DebugInfo::default();
            if let Ok(result) = VipsImage::new_from_buffer(&cached_image.bytes, "") {
                debug.result_width = result.get_width().max(0) as u32;
                debug.result_height = result.get_height().max(0) as u32;
            }
            debug
        });

        return Ok(ProcessedImage {
            bytes: cached_image.bytes,
            content_type: cached_image.content_type,
            cache_status: CacheStatus::Hit,
            content_disposition: None,
            headers,
            debug,
        });
    }

    let content_disposition = content_disposition_for(&parsed_options);

    debug!("Processing image forge request for URL: {}", decoded_url);

    let max_src_file_size = resolve_max_src_file_size(config, &parsed_options).map(MaxSourceFileSize::get);
    let fetched = fetch_image(&state.http_client, &decoded_url, max_src_file_size).await?;
    let source_metadata = SourceMetadata::from_fetch(&fetched, &decoded_url);
    // Where the bytes came from, which a redirect can move away from what was
    // asked for. The canonical header keeps naming the requested URL — that is
    // the address callers should use — but the cache entry has to remember the
    // one it actually fetched, so a hit can be rechecked against the allow list.
    let fetched_from = fetched.final_url.clone();
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
            &source_metadata,
            &fetched_from,
            &vary,
        )
        .await;
    }

    let (watermark, watermark_fetched_from) = if needs_watermark(&parsed_options) {
        match resolve_watermark(state.as_ref(), &parsed_options).await? {
            Some((watermark, fetched_from)) => (Some(watermark), fetched_from),
            None => (None, String::new()),
        }
    } else {
        (None, String::new())
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
    let want_debug = config.enable_debug_headers;
    let (processed_image_bytes, output_format, debug, etag) = tokio::task::spawn_blocking(move || {
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
        // The origin the debug headers describe is the source as fetched, so
        // they measure it even when a thumbnail stands in for decoding.
        let mut debug = want_debug.then(|| {
            let origin = opened.constraint_image();
            DebugInfo {
                origin_bytes: opened.metadata_bytes.len(),
                origin_width: origin.get_width().max(0) as u32,
                origin_height: origin.get_height().max(0) as u32,
                ..DebugInfo::default()
            }
        });

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

        // Reading the result's dimensions means decoding its header, which is
        // only worth doing when someone asked to see them.
        if let Some(debug) = debug.as_mut() {
            if let Ok(result) = VipsImage::new_from_buffer(&processed_image_bytes, "") {
                debug.result_width = result.get_width().max(0) as u32;
                debug.result_height = result.get_height().max(0) as u32;
            }
        }

        // Hashed here, inside the blocking task that just produced the bytes,
        // and regardless of the current ETag setting — the cache entry
        // outlives the configuration, and neither a hit nor the async worker
        // should ever have to hash a body.
        let etag = crate::response::entity_tag(&processed_image_bytes);

        Ok::<_, ServiceError>((processed_image_bytes, output_format, debug, etag))
    })
    .await
    .map_err(|source| ServiceError::BlockingTask {
        operation: "image processing",
        source,
    })??;

    let content_type = format_to_content_type(&output_format);
    let headers = DeliveryHeaders::with_stored_etag(config, &source_metadata, &etag, &vary);

    if content_disposition.is_none() && !matches!(state.cache, ImgforgeCache::None) {
        if let Err(err) = state.cache.insert(
            cache_key.into_owned(),
            CachedImage {
                bytes: processed_image_bytes.clone(),
                content_type,
                source_url: fetched_from.clone(),
                watermark_source_url: watermark_fetched_from.clone(),
                etag,
                origin_cache_control: source_metadata.cache_control.clone().unwrap_or_default(),
                origin_last_modified: source_metadata.last_modified.clone().unwrap_or_default(),
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
        headers,
        debug,
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

/// Resolves the source URL a request names, applying the base URL and the
/// allow list.
///
/// The allow list is checked after the base URL is applied, because that is the
/// URL that will actually be fetched — checking the shorthand form would let a
/// relative reference sidestep the restriction entirely.
fn resolve_source_url(config: &Config, url_parts: &ImgforgeUrl) -> Result<String, ServiceError> {
    let decoded = url_parts.source_url.decode()?;
    permitted_url(config, &decoded)
}

/// Applies the base URL to a reference and checks the result against the allow
/// list, returning the URL that may actually be fetched.
///
/// Every outbound fetch a request can cause goes through here, not just the one
/// for the main image. `watermark_url` names an arbitrary URL that imgforge
/// fetches server-side, so leaving it out made the allow list trivially
/// sidesteppable: the option pointed at `169.254.169.254` or any internal host
/// and imgforge fetched it. The redirect policy guards the hops *after* the
/// first request, which is no help when the first request is already the
/// attack.
fn permitted_url(config: &Config, url: &str) -> Result<String, ServiceError> {
    let resolved = config.source_rules.resolve(url);

    if !config.source_rules.permits(&resolved) {
        error!("Source URL is not in IMGFORGE_ALLOWED_SOURCES");
        return Err(ServiceError::Fetch(crate::fetch::FetchError::SourceNotAllowed));
    }

    Ok(resolved)
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

    // Resolved before the cache lookup, for the same reason the processed path
    // does it: a source the deployment no longer permits must stop being
    // described, and a persistent metadata cache would otherwise keep answering
    // for it long after `IMGFORGE_ALLOWED_SOURCES` was tightened.
    let decoded_url = resolve_source_url(config, &url_parts)?;

    // Keyed by the resolved source rather than the path, for the same reason
    // the processed cache is: a relative reference means a different image the
    // moment `IMGFORGE_BASE_URL` changes.
    let metadata_key = format!("{decoded_url}\u{0}{path}");

    // Same rule as the image cache: the request's own URL was checked above,
    // but a redirect can have moved the described source somewhere the allow
    // list no longer names, and an entry outlives the policy that admitted it.
    let cached_metadata = state.metadata_cache.get(&metadata_key).await.filter(|cached| {
        let permitted = cached.source_url.is_empty() || config.source_rules.permits(&cached.source_url);
        if !permitted {
            debug!("Ignoring cached metadata read from a source that is no longer allowed");
        }
        permitted
    });

    if let Some(cached_metadata) = cached_metadata {
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

    let fetched = fetch_image(&state.http_client, &decoded_url, None).await?;
    let fetched_from = fetched.final_url;
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
                        source_url: fetched_from,
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
        if let Err(err) = state.metadata_cache.insert(metadata_key.clone(), metadata.clone()) {
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
        if !validate_signature_of_size(
            &config.key,
            &config.salt,
            &url_parts.signature,
            &path_to_sign,
            config.signature_size,
        ) {
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

/// The watermark to composite, alongside the URL it was actually fetched
/// from — empty for the configured file watermark, which no allow list
/// governs.
async fn resolve_watermark(
    state: &AppState,
    parsed_options: &ParsedOptions,
) -> Result<Option<(CachedWatermark, String)>, ServiceError> {
    if let Some(url) = &parsed_options.watermark_url {
        // A watermark is a source like any other: the deployment decided which
        // hosts it will fetch from, and that decision cannot depend on which
        // option named the URL.
        let url = permitted_url(&state.config, url)?;
        debug!("Fetching watermark from URL: {}", url);
        match fetch_image(&state.http_client, &url, None).await {
            Ok(fetched) => Ok(Some((CachedWatermark::from_bytes(fetched.bytes), fetched.final_url))),
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
            Ok(Some((watermark.clone(), String::new())))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

/// Returns the source bytes as they arrived, for `raw` and `skip_processing`.
#[allow(clippy::too_many_arguments)]
async fn serve_source_response(
    state: &AppState,
    path: &str,
    cache_key: &str,
    image_bytes: Bytes,
    source_content_type: Option<String>,
    content_disposition: Option<String>,
    source_metadata: &SourceMetadata,
    fetched_from: &str,
    vary: &[&'static str],
) -> Result<ProcessedImage, ServiceError> {
    let content_type = source_content_type
        .as_deref()
        .map(format_to_content_type)
        .unwrap_or("image/jpeg");

    // Hashed once and shared by the header and the cache entry — a
    // passthrough has no blocking task to hide the cost in, but it does not
    // have to pay it twice.
    let etag = crate::response::entity_tag(&image_bytes);

    if content_disposition.is_none() && !matches!(state.cache, ImgforgeCache::None) {
        if let Err(err) = state.cache.insert(
            cache_key.to_string(),
            CachedImage {
                bytes: image_bytes.clone(),
                content_type,
                source_url: fetched_from.to_string(),
                // A passthrough composites nothing.
                watermark_source_url: String::new(),
                etag: etag.clone(),
                origin_cache_control: source_metadata.cache_control.clone().unwrap_or_default(),
                origin_last_modified: source_metadata.last_modified.clone().unwrap_or_default(),
            },
        ) {
            error!("Failed to cache raw image: {}", err);
        }
    }

    info!("Imgforge served source path={} bytes={}", path, image_bytes.len());

    let headers = DeliveryHeaders::with_stored_etag(&state.config, source_metadata, &etag, vary);

    // A passthrough returns the source as the result, so both halves of the
    // diagnostics describe the same bytes. Leaving the whole struct out
    // silently switched the feature off for raw and skip_processing requests.
    let debug = state.config.enable_debug_headers.then(|| {
        let mut debug = DebugInfo {
            origin_bytes: image_bytes.len(),
            ..DebugInfo::default()
        };
        if let Ok(img) = VipsImage::new_from_buffer(&image_bytes, "") {
            let (width, height) = (img.get_width().max(0) as u32, img.get_height().max(0) as u32);
            debug.origin_width = width;
            debug.origin_height = height;
            debug.result_width = width;
            debug.result_height = height;
        }
        debug
    });

    Ok(ProcessedImage {
        bytes: image_bytes,
        content_type,
        cache_status: CacheStatus::Miss,
        content_disposition,
        headers,
        debug,
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
