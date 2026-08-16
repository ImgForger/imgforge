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
    #[error("source URL is not allowed")]
    SourceNotAllowed,
    #[error("source responded with status {status}")]
    UpstreamStatus { status: u16 },
}

/// A source image and the response headers worth carrying forward.
///
/// The caching headers are kept so the proxy can pass an upstream `Cache-Control`
/// or `Last-Modified` through to its own clients, which is the difference
/// between a CDN honouring the origin's policy and inventing one.
#[derive(Debug, Clone, Default)]
pub struct FetchedImage {
    pub bytes: Bytes,
    /// The URL the bytes actually came from, after any redirects.
    ///
    /// Not the same question as the URL that was requested: the allow list is
    /// checked against what a request *asks* for, and a redirect can move the
    /// answer somewhere else. The redirect policy revalidates each hop as it
    /// happens, which leaves only the cache — an entry outlives the fetch, so
    /// it has to remember where its bytes came from.
    pub final_url: String,
    pub content_type: Option<String>,
    pub cache_control: Option<String>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
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

fn header_string(headers: &header::HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

/// Fetches an image from a given URL using the provided HTTP client.
pub async fn fetch_image(
    client: &reqwest::Client,
    url: &str,
    max_bytes: Option<usize>,
) -> Result<FetchedImage, FetchError> {
    let fetch_start = std::time::Instant::now();

    let mut response = client.get(url).send().await.map_err(|source| {
        record_fetch_metrics(fetch_start, "error");
        FetchError::Request(source)
    })?;

    // An error page is not an image. Returning its bytes meant the failure
    // surfaced later as "failed to decode source image", which told the caller
    // nothing about the 404 that actually happened.
    let final_url = response.url().to_string();

    if !response.status().is_success() {
        record_fetch_metrics(fetch_start, "error");
        return Err(FetchError::UpstreamStatus {
            status: response.status().as_u16(),
        });
    }

    let headers = response.headers().clone();
    let content_type = header_string(&headers, header::CONTENT_TYPE);
    let cache_control = header_string(&headers, header::CACHE_CONTROL);
    let last_modified = header_string(&headers, header::LAST_MODIFIED);
    let etag = header_string(&headers, header::ETAG);

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

    record_fetch_metrics(fetch_start, "success");
    Ok(FetchedImage {
        bytes: image_bytes.freeze(),
        final_url,
        content_type,
        cache_control,
        last_modified,
        etag,
    })
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
                    .insert_header("Content-Type", "image/jpeg")
                    .insert_header("Cache-Control", "max-age=600")
                    .insert_header("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT"),
            )
            .mount(&server)
            .await;

        let client = client_with_timeout(Duration::from_secs(5));
        let fetched = fetch_image(&client, &format!("{}/image.jpg", server.uri()), None)
            .await
            .expect("request should succeed");

        assert_eq!(fetched.bytes.len(), 3);
        assert_eq!(fetched.content_type.as_deref(), Some("image/jpeg"));
        assert_eq!(fetched.cache_control.as_deref(), Some("max-age=600"));
        assert_eq!(fetched.last_modified.as_deref(), Some("Wed, 21 Oct 2015 07:28:00 GMT"));
    }

    #[tokio::test]
    async fn test_fetch_image_404_is_reported_as_an_upstream_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing.jpg"))
            .respond_with(ResponseTemplate::new(404).set_body_string("nope"))
            .mount(&server)
            .await;

        let client = client_with_timeout(Duration::from_secs(5));
        let result = fetch_image(&client, &format!("{}/missing.jpg", server.uri()), None).await;

        // Returning the error page's bytes would have surfaced as "failed to
        // decode source image", hiding the 404 behind a decoder complaint.
        assert!(matches!(result, Err(FetchError::UpstreamStatus { status: 404 })));
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
        let fetched = fetch_image(&client, &format!("{}/image.png", server.uri()), None)
            .await
            .expect("request should succeed");

        assert_eq!(fetched.bytes.len(), 3);
        assert_eq!(fetched.content_type.as_deref(), Some("image/png"));
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
