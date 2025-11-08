use crate::app::AppState;
use crate::monitoring::increment_status_code;
use axum::body::Body;
use axum::extract::State;
use axum::{http::Request, http::StatusCode, middleware::Next, response::Response};
use rand::distr::Alphanumeric;
use rand::Rng;
use std::sync::Arc;

#[derive(Clone)]
pub struct RequestId(pub String);

#[derive(Clone)]
pub struct OutputFormat(pub String);

fn generate_request_id() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

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

pub async fn request_id_middleware(mut req: Request<Body>, next: Next) -> Response {
    let request_id = generate_request_id();
    req.extensions_mut().insert(RequestId(request_id.clone()));
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert("X-Request-ID", request_id.parse().unwrap());
    response
}

pub async fn content_type_middleware(req: Request<Body>, next: Next) -> Response {
    // Get the output format before consuming the request
    let output_format = req.extensions().get::<OutputFormat>().map(|f| f.0.clone());

    let mut response = next.run(req).await;

    // Check if the response already has a content-type header
    if response.headers().get("content-type").is_none() {
        // Check if an output format was set
        if let Some(format) = output_format {
            let content_type = format_to_content_type(&format);
            response
                .headers_mut()
                .insert("content-type", content_type.parse().unwrap());
        }
    }

    response
}

pub async fn status_code_metric_middleware(req: Request<Body>, next: Next) -> Response {
    let response = next.run(req).await;
    let status = response.status();
    increment_status_code(status.as_str());
    response
}

pub async fn rate_limit_middleware(State(state): State<Arc<AppState>>, request: Request<Body>, next: Next) -> Response {
    if let Some(rate_limiter) = &state.rate_limiter {
        match rate_limiter.check() {
            Ok(_) => next.run(request).await,
            Err(_) => Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .body(Body::from("Too Many Requests"))
                .unwrap(),
        }
    } else {
        // If the rate limiter is not configured, just proceed
        next.run(request).await
    }
}
