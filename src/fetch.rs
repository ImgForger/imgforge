use crate::constants::*;
use axum::body::Bytes;
use axum::http::header;
use std::env;
use std::time::Duration;
use tracing::error;

/// Fetches an image from a given URL.
pub async fn fetch_image(url: &str) -> Result<(Bytes, Option<String>), String> {
    let fetch_start = std::time::Instant::now();

    let download_timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map_or(Duration::from_secs(10), Duration::from_secs);

    let client = reqwest::Client::builder()
        .timeout(download_timeout)
        .build()
        .expect("Failed to build reqwest client");

    let response = match client.get(url).send().await {
        Ok(res) => {
            let fetch_duration = fetch_start.elapsed().as_secs_f64();
            crate::monitoring::SOURCE_IMAGE_FETCH_DURATION_SECONDS
                .with_label_values(&[] as &[&str])
                .observe(fetch_duration);
            if res.status().is_success() {
                crate::monitoring::SOURCE_IMAGES_FETCHED_TOTAL
                    .with_label_values(&["success"])
                    .inc();
            } else {
                crate::monitoring::SOURCE_IMAGES_FETCHED_TOTAL
                    .with_label_values(&["error"])
                    .inc();
            }
            res
        }
        Err(e) => {
            let fetch_duration = fetch_start.elapsed().as_secs_f64();
            crate::monitoring::SOURCE_IMAGE_FETCH_DURATION_SECONDS
                .with_label_values(&[] as &[&str])
                .observe(fetch_duration);
            crate::monitoring::SOURCE_IMAGES_FETCHED_TOTAL
                .with_label_values(&["error"])
                .inc();
            error!("Error fetching image: {}", e);
            return Err(format!("Error fetching image: {}", e));
        }
    };

    let headers = response.headers().clone();

    let image_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Error reading image bytes: {}", e);
            return Err(format!("Error reading image bytes: {}", e));
        }
    };

    let mut content_type: Option<String> = None;
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        if let Ok(ct_str) = ct.to_str() {
            content_type = Some(ct_str.to_string());
        }
    }

    Ok((image_bytes, content_type))
}
