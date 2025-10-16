use crate::handlers::AppState;
use crate::monitoring::STATUS_CODES_TOTAL;
use axum::body::Body;
use axum::extract::State;
use axum::{http::Request, http::StatusCode, middleware::Next, response::Response};
use std::sync::Arc;

pub async fn status_code_metric_middleware(req: Request<Body>, next: Next) -> Response {
    let response = next.run(req).await;
    let status = response.status();
    STATUS_CODES_TOTAL.with_label_values(&[status.as_str()]).inc();
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
        // If rate limiter is not configured, just proceed
        next.run(request).await
    }
}
