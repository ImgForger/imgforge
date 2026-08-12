use crate::monitoring::{increment_source_images_fetched, observe_source_image_fetch_duration};
use bytes::{Bytes, BytesMut};
use reqwest::header;
use thiserror::Error;

const DEFAULT_INITIAL_BUFFER_CAPACITY: usize = 64 * 1024;

/// Errors that can occur while fetching a source image.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FetchError {
    #[error("failed to fetch source image")]
    Request(#[source] reqwest::Error),
    #[error("failed to read source image response body")]
    ResponseBody(#[source] reqwest::Error),
    #[error("source image exceeds the maximum allowed size of {limit} bytes")]
    SourceTooLarge { limit: usize, actual: Option<u64> },
}

fn record_fetch_metrics(fetch_start: std::time::Instant, status: &str) {
    // Record full fetch time, including streaming the response body, not just time-to-headers.
    observe_source_image_fetch_duration(fetch_start.elapsed().as_secs_f64());
    increment_source_images_fetched(status);
}

fn initial_buffer_capacity(content_length: Option<usize>, max_bytes: Option<usize>) -> usize {
    match (content_length, max_bytes) {
        (Some(len), Some(limit)) => len.min(limit),
        (Some(len), None) => len,
        // Avoid reserving an unbounded amount up front when the server omits Content-Length.
        (None, Some(limit)) => limit.min(DEFAULT_INITIAL_BUFFER_CAPACITY),
        (None, None) => 0,
    }
}

/// Fetches an image from a given URL using the provided HTTP client.
pub async fn fetch_image(
    client: &reqwest::Client,
    url: &str,
    max_bytes: Option<usize>,
) -> Result<(Bytes, Option<String>), FetchError> {
    let fetch_start = std::time::Instant::now();

    let mut response = client.get(url).send().await.map_err(|source| {
        record_fetch_metrics(fetch_start, "error");
        FetchError::Request(source)
    })?;
    let fetch_status = if response.status().is_success() {
        "success"
    } else {
        "error"
    };

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| ct.to_string());

    let advertised_length = response.content_length().map(|len| len as usize);
    if let (Some(limit), Some(len)) = (max_bytes, advertised_length) {
        if len > limit {
            record_fetch_metrics(fetch_start, "error");
            return Err(FetchError::SourceTooLarge {
                limit,
                actual: Some(len as u64),
            });
        }
    }

    // Reserve based on the tightest known size bound so large responses do not repeatedly grow the buffer.
    let mut image_bytes = BytesMut::with_capacity(initial_buffer_capacity(advertised_length, max_bytes));
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if let Some(limit) = max_bytes {
                    let fetched_size = image_bytes.len().checked_add(chunk.len());
                    if fetched_size.is_none_or(|size| size > limit) {
                        record_fetch_metrics(fetch_start, "error");
                        return Err(FetchError::SourceTooLarge {
                            limit,
                            actual: fetched_size.map(|size| size as u64),
                        });
                    }
                }

                image_bytes.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => {
                record_fetch_metrics(fetch_start, "error");
                return Err(FetchError::ResponseBody(e));
            }
        }
    }

    record_fetch_metrics(fetch_start, fetch_status);
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
        assert!(matches!(result, Err(FetchError::Request(_))));
    }

    #[tokio::test]
    async fn test_fetch_image_nonexistent_domain() {
        let client = client_with_timeout(Duration::from_secs(5));
        let result = fetch_image(&client, "http://this-domain-does-not-exist-12345.com/image.jpg", None).await;
        assert!(matches!(result, Err(FetchError::Request(_))));
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

        assert!(matches!(
            result,
            Err(FetchError::SourceTooLarge {
                limit: 3,
                actual: Some(5)
            })
        ));
    }

    #[tokio::test]
    async fn test_fetch_image_rejects_large_content_length_before_streaming() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/advertised-large.jpg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Length", "10")
                    .set_body_bytes(vec![0u8; 10]),
            )
            .mount(&server)
            .await;

        let client = client_with_timeout(Duration::from_secs(5));
        let result = fetch_image(&client, &format!("{}/advertised-large.jpg", server.uri()), Some(3)).await;

        assert!(matches!(
            result,
            Err(FetchError::SourceTooLarge {
                limit: 3,
                actual: Some(10)
            })
        ));
    }

    #[test]
    fn test_client_builder_timeout_configuration() {
        let timeout = Duration::from_secs(15);
        let client = reqwest::Client::builder().timeout(timeout).build();

        assert!(client.is_ok());
    }
}
