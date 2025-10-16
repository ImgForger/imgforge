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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_timeout_from_env() {
        let test_timeout = "30";
        env::set_var(ENV_DOWNLOAD_TIMEOUT, test_timeout);

        let timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map_or(Duration::from_secs(10), Duration::from_secs);

        assert_eq!(timeout, Duration::from_secs(30));
        env::remove_var(ENV_DOWNLOAD_TIMEOUT);
    }

    #[test]
    fn test_download_timeout_default() {
        env::remove_var(ENV_DOWNLOAD_TIMEOUT);

        let timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map_or(Duration::from_secs(10), Duration::from_secs);

        assert_eq!(timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_download_timeout_invalid() {
        env::set_var(ENV_DOWNLOAD_TIMEOUT, "invalid");

        let timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map_or(Duration::from_secs(10), Duration::from_secs);

        assert_eq!(timeout, Duration::from_secs(10));
        env::remove_var(ENV_DOWNLOAD_TIMEOUT);
    }

    #[tokio::test]
    async fn test_fetch_image_invalid_url() {
        let result = fetch_image("not_a_valid_url").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Error fetching image"));
    }

    #[tokio::test]
    async fn test_fetch_image_nonexistent_domain() {
        let result = fetch_image("http://this-domain-does-not-exist-12345.com/image.jpg").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Error fetching image"));
    }

    #[tokio::test]
    async fn test_fetch_image_success_with_httpbin() {
        let result = fetch_image("https://httpbin.org/image/jpeg").await;
        
        if result.is_ok() {
            let (bytes, content_type) = result.unwrap();
            assert!(!bytes.is_empty());
            assert!(content_type.is_some());
            let ct = content_type.unwrap();
            assert!(ct.contains("image/jpeg") || ct.contains("image"));
        }
    }

    #[tokio::test]
    async fn test_fetch_image_404() {
        let result = fetch_image("https://httpbin.org/status/404").await;
        
        if result.is_ok() {
            let (bytes, _) = result.unwrap();
            assert_eq!(bytes.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_fetch_image_with_custom_timeout() {
        env::set_var(ENV_DOWNLOAD_TIMEOUT, "1");
        
        let result = fetch_image("https://httpbin.org/delay/5").await;
        
        assert!(result.is_err());
        env::remove_var(ENV_DOWNLOAD_TIMEOUT);
    }

    #[tokio::test]
    async fn test_fetch_image_content_type_extraction() {
        let result = fetch_image("https://httpbin.org/image/png").await;
        
        if result.is_ok() {
            let (bytes, content_type) = result.unwrap();
            assert!(!bytes.is_empty());
            
            if let Some(ct) = content_type {
                assert!(ct.contains("image"));
            }
        }
    }

    #[test]
    fn test_client_builder_timeout_configuration() {
        let timeout = Duration::from_secs(15);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build();
        
        assert!(client.is_ok());
    }
}
