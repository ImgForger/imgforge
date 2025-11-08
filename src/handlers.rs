use crate::app::AppState;
use crate::service::{self, CacheStatus, ProcessRequest};
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json};
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
        },
    )
    .await
    {
        Ok(info) => {
            let response = json!({
                "width": info.width,
                "height": info.height,
                "format": info.format,
            });
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            error!("Info handler error path={} error={}", path, err);
            (err.status(), err.message().to_string()).into_response()
        }
    }
}

/// Handles the main image processing endpoint.
pub async fn image_forge_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> impl IntoResponse {
    let bearer = auth_header.map(|TypedHeader(auth)| auth.token().to_string());

    match service::process_path(
        state,
        ProcessRequest {
            path: &path,
            bearer_token: bearer.as_deref(),
        },
    )
    .await
    {
        Ok(result) => {
            let mut headers = header::HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(result.content_type));
            if result.cache_status == CacheStatus::Hit {
                headers.insert(
                    header::CACHE_STATUS,
                    HeaderValue::from_static(CacheStatus::Hit.as_header_value()),
                );
            }

            (StatusCode::OK, headers, result.bytes).into_response()
        }
        Err(err) => {
            error!("Image handler error path={} error={}", path, err);
            (err.status(), err.message().to_string()).into_response()
        }
    }
}
