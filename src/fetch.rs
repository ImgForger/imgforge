use crate::constants::*;
use crate::monitoring::{increment_source_images_fetched, observe_source_image_fetch_duration};
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
            observe_source_image_fetch_duration(fetch_duration);
            if res.status().is_success() {
                increment_source_images_fetched("success");
            } else {
                increment_source_images_fetched("error");
            }
            res
        }
        Err(e) => {
            let fetch_duration = fetch_start.elapsed().as_secs_f64();
            observe_source_image_fetch_duration(fetch_duration);
            increment_source_images_fetched("error");
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
    async fn test_fetch_image_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/image.jpg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![1u8, 2, 3])
                    .insert_header("Content-Type", "image/jpeg"),
            )
            .mount(&server)
            .await;

        let (bytes, content_type) = fetch_image(&format!("{}/image.jpg", server.uri()))
            .await
            .expect("request should succeed");

        assert_eq!(bytes.len(), 3);
        assert_eq!(content_type.as_deref(), Some("image/jpeg"));
    }

    #[tokio::test]
    async fn test_fetch_image_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing.jpg"))
            .respond_with(ResponseTemplate::new(404).set_body_bytes(Vec::<u8>::new()))
            .mount(&server)
            .await;

        let (bytes, _) = fetch_image(&format!("{}/missing.jpg", server.uri()))
            .await
            .expect("404 responses should still return bytes");

        assert_eq!(bytes.len(), 0);
    }

    #[tokio::test]
    async fn test_fetch_image_with_custom_timeout() {
        env::set_var(ENV_DOWNLOAD_TIMEOUT, "1");

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow.jpg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(3))
                    .set_body_bytes(vec![0u8; 1]),
            )
            .mount(&server)
            .await;

        let result = fetch_image(&format!("{}/slow.jpg", server.uri())).await;

        env::remove_var(ENV_DOWNLOAD_TIMEOUT);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_image_content_type_extraction() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/image.png"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![9u8, 8, 7])
                    .insert_header("Content-Type", "image/png"),
            )
            .mount(&server)
            .await;

        let (bytes, content_type) = fetch_image(&format!("{}/image.png", server.uri()))
            .await
            .expect("request should succeed");

        assert_eq!(bytes.len(), 3);
        assert_eq!(content_type.as_deref(), Some("image/png"));
    }

    #[test]
    fn test_client_builder_timeout_configuration() {
        let timeout = Duration::from_secs(15);
        let client = reqwest::Client::builder().timeout(timeout).build();

        assert!(client.is_ok());
    }
}
