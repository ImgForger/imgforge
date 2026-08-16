use crate::app::AppState;
use crate::negotiation::RequestHints;
use crate::response::{matches_if_modified_since, matches_if_none_match};
use crate::service::{self, CacheStatus, DebugInfo, ProcessRequest, ProcessedImage};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;
use serde_json::json;
use std::sync::Arc;
use tracing::error;

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
    let bearer = auth_header.map(|TypedHeader(auth)| auth.token().to_string());

    match service::image_info(
        state.clone(),
        ProcessRequest {
            path: &path,
            bearer_token: bearer.as_deref(),
            hints: RequestHints::default(),
        },
    )
    .await
    {
        Ok(info) => {
            let response = json!({
                "width": info.width,
                "height": info.height,
                "format": info.format,
                "content_type": info.content_type,
                "size_bytes": info.size_bytes,
                "channels": info.channels,
                "has_alpha": info.has_alpha,
                "orientation": info.orientation,
                "pages": info.pages,
            });
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            error!(path, error = ?err, "Info handler error");
            error_response(state.as_ref(), &err)
        }
    }
}

/// Handles the main image processing endpoint.
pub async fn image_forge_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    request_headers: HeaderMap,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> impl IntoResponse {
    let bearer = auth_header.map(|TypedHeader(auth)| auth.token().to_string());
    let hints = RequestHints::from_headers(&request_headers, state.config.enable_client_hints);

    match service::process_path(
        state.clone(),
        ProcessRequest {
            path: &path,
            bearer_token: bearer.as_deref(),
            hints,
        },
    )
    .await
    {
        Ok(result) => image_response(state.as_ref(), &request_headers, result),
        Err(err) => {
            error!(path, error = ?err, "Image handler error");
            error_response(state.as_ref(), &err)
        }
    }
}

/// Builds the response for a processed image, including the conditional-request
/// short circuit.
fn image_response(state: &AppState, request_headers: &HeaderMap, result: ProcessedImage) -> Response {
    let mut headers = header::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(result.content_type));

    if result.cache_status == CacheStatus::Hit {
        headers.insert(
            header::CACHE_STATUS,
            HeaderValue::from_static(CacheStatus::Hit.as_header_value()),
        );
    }

    if let Some(content_disposition) = result.content_disposition.as_deref() {
        insert_header(&mut headers, header::CONTENT_DISPOSITION, content_disposition);
    }

    let delivery = &result.headers;
    if let Some(cache_control) = delivery.cache_control.as_deref() {
        insert_header(&mut headers, header::CACHE_CONTROL, cache_control);
    }
    if let Some(last_modified) = delivery.last_modified.as_deref() {
        insert_header(&mut headers, header::LAST_MODIFIED, last_modified);
    }
    if let Some(canonical) = delivery.canonical.as_deref() {
        insert_header(&mut headers, header::LINK, canonical);
    }
    if !delivery.vary.is_empty() {
        insert_header(&mut headers, header::VARY, &delivery.vary.join(", "));
    }
    if let Some(origin) = state.config.allow_origin.as_deref() {
        insert_header(&mut headers, header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    if let Some(debug) = result.debug {
        insert_debug_headers(&mut headers, debug);
    }

    // The body was produced either way — the saving is bandwidth, not work —
    // but for a large image over a slow link that is the saving that matters.
    //
    // RFC 9110 makes `If-None-Match` take precedence over `If-Modified-Since`
    // only when the request *carries* one. Keying that on whether an ETag was
    // emitted instead meant a deployment with both features on never evaluated
    // `If-Modified-Since` at all, and answered a conditional request that sent
    // only that header with the whole body.
    let mut not_modified = false;
    if let Some(etag) = delivery.etag.as_deref() {
        insert_header(&mut headers, header::ETAG, etag);
        not_modified = matches_if_none_match(request_headers, etag);
    }
    if !not_modified && !request_headers.contains_key(header::IF_NONE_MATCH) {
        if let Some(last_modified) = delivery.last_modified.as_deref() {
            not_modified = matches_if_modified_since(request_headers, last_modified);
        }
    }

    if not_modified {
        return (StatusCode::NOT_MODIFIED, headers).into_response();
    }

    (StatusCode::OK, headers, result.bytes).into_response()
}

/// Emits the sizes the response actually knows.
///
/// A zero is "not measured" rather than a measurement: a cache hit never made
/// the source request, so it can report the result it is holding but nothing
/// about the origin. Sending `x-origin-width: 0` there would be a false
/// statement dressed as a diagnostic, so the unknown fields are simply omitted.
fn insert_debug_headers(headers: &mut header::HeaderMap, debug: DebugInfo) {
    let values = [
        ("x-origin-content-length", debug.origin_bytes as u64),
        ("x-origin-width", u64::from(debug.origin_width)),
        ("x-origin-height", u64::from(debug.origin_height)),
        ("x-result-width", u64::from(debug.result_width)),
        ("x-result-height", u64::from(debug.result_height)),
    ];

    for (name, value) in values {
        if value == 0 {
            continue;
        }
        if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
            headers.insert(name, HeaderValue::from(value));
        }
    }
}

/// A header value that cannot be represented is dropped rather than failing the
/// response: the image is still correct without it.
fn insert_header(headers: &mut header::HeaderMap, name: HeaderName, value: &str) {
    match HeaderValue::from_str(value) {
        Ok(value) => {
            headers.insert(name, value);
        }
        Err(err) => error!("Invalid value for the {} header: {}", name, err),
    }
}

/// Turns a service failure into a response.
///
/// The client normally sees only the curated message; development-errors mode
/// adds the underlying cause, which is what makes a misconfigured deployment
/// diagnosable without reading the server's logs.
fn error_response(state: &AppState, err: &service::ServiceError) -> Response {
    let mut body = err.message().into_owned();
    if state.config.development_errors_mode {
        body.push_str(&format!("\n\n{err:?}"));
    }

    let mut headers = header::HeaderMap::new();
    if let Some(origin) = state.config.allow_origin.as_deref() {
        insert_header(&mut headers, header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }

    (err.status(), headers, body).into_response()
}
