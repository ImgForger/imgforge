use crate::handlers::AppState;
use crate::monitoring::increment_status_code;
use axum::body::Body;
use axum::extract::State;
use axum::{http::Request, http::StatusCode, middleware::Next, response::Response};
use rand::distr::Alphanumeric;
use rand::Rng;
use std::sync::Arc;

#[derive(Clone)]
pub struct RequestId(pub String);

fn generate_request_id() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

pub async fn request_id_middleware(mut req: Request<Body>, next: Next) -> Response {
    let request_id = generate_request_id();
    req.extensions_mut().insert(RequestId(request_id));
    next.run(req).await
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
