use crate::monitoring::{increment_source_images_fetched, observe_source_image_fetch_duration};
use bytes::{Bytes, BytesMut};
use reqwest::header;
use tracing::error;

/// Fetches an image from a given URL using the provided HTTP client.
pub async fn fetch_image(
    client: &reqwest::Client,
    url: &str,
    max_bytes: Option<usize>,
) -> Result<(Bytes, Option<String>), String> {
    let fetch_start = std::time::Instant::now();

    let mut response = match client.get(url).send().await {
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

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| ct.to_string());

    let mut image_bytes = BytesMut::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if let Some(limit) = max_bytes {
                    if image_bytes.len() + chunk.len() > limit {
                        error!(
                            "Fetched image exceeds configured max size limit ({} bytes) for url={}",
                            limit, url
                        );
                        return Err(format!(
                            "Source image exceeds the maximum allowed size of {} bytes",
                            limit
                        ));
                    }
                }

                image_bytes.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => {
                error!("Error reading image bytes: {}", e);
                return Err(format!("Error reading image bytes: {}", e));
            }
        }
    }

    Ok((image_bytes.freeze(), content_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_with_timeout(timeout: Duration) -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("client builds")
    }

    #[tokio::test]
    async fn test_fetch_image_invalid_url() {
        let client = client_with_timeout(Duration::from_secs(5));
        let result = fetch_image(&client, "not_a_valid_url", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Error fetching image"));
    }

    #[tokio::test]
    async fn test_fetch_image_nonexistent_domain() {
        let client = client_with_timeout(Duration::from_secs(5));
        let result = fetch_image(&client, "http://this-domain-does-not-exist-12345.com/image.jpg", None).await;
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

        let client = client_with_timeout(Duration::from_secs(5));
        let (bytes, content_type) = fetch_image(&client, &format!("{}/image.jpg", server.uri()), None)
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

        let client = client_with_timeout(Duration::from_secs(5));
        let (bytes, _) = fetch_image(&client, &format!("{}/missing.jpg", server.uri()), None)
            .await
            .expect("404 responses should still return bytes");

        assert_eq!(bytes.len(), 0);
    }

    #[tokio::test]
    async fn test_fetch_image_with_custom_timeout() {
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

        let client = client_with_timeout(Duration::from_secs(1));
        let result = fetch_image(&client, &format!("{}/slow.jpg", server.uri()), None).await;

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

        let client = client_with_timeout(Duration::from_secs(5));
        let (bytes, content_type) = fetch_image(&client, &format!("{}/image.png", server.uri()), None)
            .await
            .expect("request should succeed");

        assert_eq!(bytes.len(), 3);
        assert_eq!(content_type.as_deref(), Some("image/png"));
    }

    #[tokio::test]
    async fn test_fetch_image_enforces_max_size() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/large.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; 5]))
            .mount(&server)
            .await;

        let client = client_with_timeout(Duration::from_secs(5));
        let result = fetch_image(&client, &format!("{}/large.jpg", server.uri()), Some(3)).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maximum allowed size"));
    }

    #[test]
    fn test_client_builder_timeout_configuration() {
        let timeout = Duration::from_secs(15);
        let client = reqwest::Client::builder().timeout(timeout).build();

        assert!(client.is_ok());
    }
}
