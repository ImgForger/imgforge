use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use bytes::Bytes;
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use image::{ImageBuffer, Rgba};
use imgforge::app::AppState;
use imgforge::caching::cache::ImgforgeCache;
use imgforge::config::Config;
use imgforge::handlers::{image_forge_handler, info_handler, status_handler};
use imgforge::middleware::request_id_middleware;
use lazy_static::lazy_static;
use libvips::VipsApp;
use serde_json::Value;
use sha2::Sha256;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

type HmacSha256 = Hmac<Sha256>;

lazy_static! {
    static ref VIPS_APP: Arc<VipsApp> =
        Arc::new(VipsApp::new("imgforge-test", false).expect("Failed to initialize libvips"));
}

/// Helper function to create a test PNG image
fn create_test_image(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (_x, _y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba(color);
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

/// Helper function to create a test JPEG image without alpha channel
fn create_test_jpeg_image(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let mut img: ImageBuffer<image::Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (_x, _y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image::Rgb(color);
    }
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .unwrap();
    bytes
}

/// Helper function to generate HMAC signature
fn generate_signature(key: &[u8], salt: &[u8], path: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(salt);
    mac.update(path.as_bytes());
    let signature_bytes = mac.finalize().into_bytes();
    URL_SAFE_NO_PAD.encode(signature_bytes)
}

/// Helper function to create test config
fn create_test_config(key: Vec<u8>, salt: Vec<u8>, allow_unsigned: bool) -> Config {
    let mut config = Config::new(key, salt);
    config.workers = 4;
    config.allow_unsigned = allow_unsigned;
    config.allow_security_options = true;
    config
}

/// Helper function to create test AppState
async fn create_test_state(config: Config) -> Arc<AppState> {
    let cache = ImgforgeCache::None;
    let metadata_cache = imgforge::caching::cache::MetadataCache::None;
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.download_timeout))
        .build()
        .expect("client builds");

    Arc::new(AppState {
        semaphore: Arc::new(Semaphore::new(config.workers)),
        cache,
        metadata_cache,
        rate_limiter: None,
        config,
        vips_app: VIPS_APP.clone(),
        http_client,
        watermark_cache: Mutex::new(None),
    })
}

/// Helper function to make a request and get response
async fn make_request(
    app: axum::Router,
    uri: &str,
    auth_token: Option<&str>,
) -> (StatusCode, String, axum::http::HeaderMap) {
    let (status, body_bytes, headers) = make_request_bytes(app, uri, auth_token).await;
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    (status, body_str, headers)
}

async fn make_request_bytes(
    app: axum::Router,
    uri: &str,
    auth_token: Option<&str>,
) -> (StatusCode, Bytes, axum::http::HeaderMap) {
    let mut request_builder = Request::builder().uri(uri);

    if let Some(token) = auth_token {
        request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
    }

    let request = request_builder.body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body, headers)
}

#[tokio::test]
async fn test_status_handler_success() {
    let app = axum::Router::new()
        .route("/status", axum::routing::get(status_handler))
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, headers) = make_request(app, "/status", None).await;

    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(headers.contains_key("X-Request-ID"));
}

#[tokio::test]
async fn test_info_handler_with_unsigned_url() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 300, [255, 0, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/test.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/test.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/info/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, headers) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["width"], 400);
    assert_eq!(json["height"], 300);
    assert!(headers.contains_key("X-Request-ID"));
}

#[tokio::test]
async fn test_info_handler_with_signed_url() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(200, 150, [0, 255, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/signed.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key_123";
    let salt = b"test_salt_456";
    let config = create_test_config(key.to_vec(), salt.to_vec(), false);
    let state = create_test_state(config).await;

    let source_url = format!("{}/signed.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/{}", encoded_url);
    let signature = generate_signature(key, salt, &path_to_sign);
    let full_path = format!("/info/{}{}", signature, path_to_sign);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &full_path, None).await;

    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["width"], 200);
    assert_eq!(json["height"], 150);
}

#[tokio::test]
async fn test_info_handler_invalid_signature() {
    let mock_server = MockServer::start().await;

    let key = b"test_key_123";
    let salt = b"test_salt_456";
    let config = create_test_config(key.to_vec(), salt.to_vec(), false);
    let state = create_test_state(config).await;

    let source_url = format!("{}/test.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/info/invalid_signature/{}", encoded_url);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Invalid signature"));
}

#[tokio::test]
async fn test_info_handler_unsigned_not_allowed() {
    let config = create_test_config(b"key".to_vec(), b"salt".to_vec(), false);
    let state = create_test_state(config).await;

    let encoded_url = URL_SAFE_NO_PAD.encode(b"http://example.com/test.jpg");
    let path = format!("/info/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Unsigned URLs are not allowed"));
}

#[tokio::test]
async fn test_info_handler_with_bearer_auth() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [255, 255, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/auth.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let mut config = create_test_config(vec![], vec![], true);
    config.secret = Some("my_secret_token".to_string());
    let state = create_test_state(config).await;

    let source_url = format!("{}/auth.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/info/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, Some("my_secret_token")).await;

    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["width"], 100);
    assert_eq!(json["height"], 100);
}

#[tokio::test]
async fn test_info_handler_invalid_bearer_token() {
    let mut config = create_test_config(vec![], vec![], true);
    config.secret = Some("my_secret_token".to_string());
    let state = create_test_state(config).await;

    let encoded_url = URL_SAFE_NO_PAD.encode(b"http://example.com/test.jpg");
    let path = format!("/info/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, Some("wrong_token")).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Invalid authorization token"));
}

#[tokio::test]
async fn test_info_handler_missing_bearer_token() {
    let mut config = create_test_config(vec![], vec![], true);
    config.secret = Some("my_secret_token".to_string());
    let state = create_test_state(config).await;

    let encoded_url = URL_SAFE_NO_PAD.encode(b"http://example.com/test.jpg");
    let path = format!("/info/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Missing authorization token"));
}

#[tokio::test]
async fn test_info_handler_invalid_url_format() {
    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let path = "/info/unsafe";

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, path, None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("Invalid URL format"));
}

#[tokio::test]
async fn test_info_handler_fetch_error() {
    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = "http://nonexistent-domain-12345.com/test.jpg";
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/info/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/info/{*path}", axum::routing::get(info_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("Error fetching image"));
}

#[tokio::test]
async fn test_image_forge_handler_unsigned_url() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 300, [255, 0, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/forge.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/forge.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, headers) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(headers.contains_key("X-Request-ID"));
}

#[tokio::test]
async fn test_image_forge_handler_with_resize() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 300, [0, 0, 255, 255]);

    Mock::given(method("GET"))
        .and(path("/resize.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/resize.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/resize:fit:200:150/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_quality() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(200, 200, [128, 128, 128, 255]);

    Mock::given(method("GET"))
        .and(path("/quality.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/quality.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/quality:80/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_blur() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(150, 150, [255, 128, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/blur.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/blur.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/blur:5/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_raw_option() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [255, 255, 255, 255]);

    Mock::given(method("GET"))
        .and(path("/raw.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image.clone())
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/raw.jpg", mock_server.uri());
    let path = format!("/unsafe/raw:/plain/{}", source_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, headers) = make_request_bytes(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), test_image.as_slice());
    assert_eq!(
        headers.get("content-type").and_then(|value| value.to_str().ok()),
        Some("image/jpeg")
    );
}

#[tokio::test]
async fn test_image_forge_handler_invalid_processing_option() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [0, 0, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/invalid.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/invalid.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/resize:fit:abc:def/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_image_forge_handler_max_file_size_exceeded() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 400, [255, 0, 255, 255]);

    Mock::given(method("GET"))
        .and(path("/large.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image.clone())
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let mut config = create_test_config(vec![], vec![], true);
    config.max_src_file_size = Some(100);
    let state = create_test_state(config).await;

    let source_url = format!("{}/large.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("Source image exceeds the maximum allowed size of"));
}

#[tokio::test]
async fn test_image_forge_handler_max_resolution_exceeded() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(2000, 2000, [100, 100, 100, 255]);

    Mock::given(method("GET"))
        .and(path("/highres.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let mut config = create_test_config(vec![], vec![], true);
    config.max_src_resolution = Some(1.0);
    let state = create_test_state(config).await;

    let source_url = format!("{}/highres.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("Source image resolution is too large"));
}

#[tokio::test]
async fn test_image_forge_handler_mime_type_restriction() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [50, 150, 250, 255]);

    Mock::given(method("GET"))
        .and(path("/test.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/gif"),
        )
        .mount(&mock_server)
        .await;

    let mut config = create_test_config(vec![], vec![], true);
    config.allowed_mime_types = Some(vec!["image/jpeg".to_string(), "image/png".to_string()]);
    let state = create_test_state(config).await;

    let source_url = format!("{}/test.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("Source image MIME type is not allowed"));
}

#[tokio::test]
async fn test_image_forge_handler_signed_url() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(300, 200, [200, 100, 50, 255]);

    Mock::given(method("GET"))
        .and(path("/signed.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let key = b"secure_key_789";
    let salt = b"secure_salt_012";
    let config = create_test_config(key.to_vec(), salt.to_vec(), false);
    let state = create_test_state(config).await;

    let source_url = format!("{}/signed.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/resize:fit:100:100/{}", encoded_url);
    let signature = generate_signature(key, salt, &path_to_sign);
    let full_path = format!("/{}{}", signature, path_to_sign);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &full_path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_multiple_processing_options() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(500, 400, [70, 130, 180, 255]);

    Mock::given(method("GET"))
        .and(path("/multi.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/multi.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/resize:fit:250:200/quality:85/blur:2/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_format_conversion() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(200, 200, [255, 200, 100, 255]);

    Mock::given(method("GET"))
        .and(path("/convert.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/convert.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/format:png/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_crop() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 400, [0, 200, 200, 255]);

    Mock::given(method("GET"))
        .and(path("/crop.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/crop.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/crop:50:50:200:200/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_rotation() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(300, 200, [180, 90, 45, 255]);

    Mock::given(method("GET"))
        .and(path("/rotate.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/rotate.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/rotation:90/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_dpr() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [255, 100, 200, 255]);

    Mock::given(method("GET"))
        .and(path("/dpr.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/dpr.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/resize:fit:100:100/dpr:2/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_plain_url() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(150, 150, [60, 120, 180, 255]);

    Mock::given(method("GET"))
        .and(path("/plain.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/plain.jpg", mock_server.uri());
    let path = format!("/unsafe/plain/{}", source_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_extension() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [220, 180, 140, 255]);

    Mock::given(method("GET"))
        .and(path("/ext.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/ext.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}.webp", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_sharpen() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(200, 200, [100, 150, 200, 255]);

    Mock::given(method("GET"))
        .and(path("/sharpen.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/sharpen.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/sharpen:0.5/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_padding() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [255, 128, 64, 255]);

    Mock::given(method("GET"))
        .and(path("/padding.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/padding.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/padding:10:20/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_background_color() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [200, 200, 200, 128]);

    Mock::given(method("GET"))
        .and(path("/bg.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/bg.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/background:ffffff/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_forge_handler_with_background_color_jpeg_source() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_jpeg_image(800, 600, [200, 100, 50]);

    Mock::given(method("GET"))
        .and(path("/bg.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state(config).await;

    let source_url = format!("{}/bg.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!(
        "/unsafe/resize:fill:800:600/gravity:center/quality:88/sharpen:1/background:FFFFFF/{}",
        encoded_url
    );

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _body, _) = make_request(app, &path, None).await;

    assert_eq!(status, StatusCode::OK);
}
