use crate::monitoring::STATUS_CODES_TOTAL;
use axum::body::Body;
use axum::{http::Request, middleware::Next, response::Response};

pub async fn status_code_metric_middleware(req: Request<Body>, next: Next) -> Response {
    let response = next.run(req).await;
    let status = response.status();
    STATUS_CODES_TOTAL.with_label_values(&[status.as_str()]).inc();
    response
}
