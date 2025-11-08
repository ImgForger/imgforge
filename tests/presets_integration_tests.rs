use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use image::{ImageBuffer, Rgba};
use imgforge::app::{AppState, DefaultWatermark};
use imgforge::caching::cache::ImgforgeCache;
use imgforge::config::Config;
use imgforge::handlers::image_forge_handler;
use imgforge::middleware::request_id_middleware;
use lazy_static::lazy_static;
use libvips::{VipsApp, VipsImage};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};
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

fn generate_signature(key: &[u8], salt: &[u8], path: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(salt);
    mac.update(path.as_bytes());
    let signature_bytes = mac.finalize().into_bytes();
    URL_SAFE_NO_PAD.encode(signature_bytes)
}

fn create_test_config(
    key: Vec<u8>,
    salt: Vec<u8>,
    allow_unsigned: bool,
    presets: HashMap<String, String>,
    only_presets: bool,
) -> Config {
    let mut config = Config::new(key, salt);
    config.workers = 4;
    config.allow_unsigned = allow_unsigned;
    config.allow_security_options = true;
    config.presets = presets;
    config.only_presets = only_presets;
    config
}

async fn create_test_state(config: Config) -> Arc<AppState> {
    let cache = ImgforgeCache::None;
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.download_timeout))
        .build()
        .expect("client builds");

    Arc::new(AppState {
        semaphore: Semaphore::new(config.workers),
        cache,
        rate_limiter: None,
        config,
        vips_app: VIPS_APP.clone(),
        http_client,
        default_watermark: DefaultWatermark::Unset,
        remote_watermarks: RwLock::new(HashMap::new()),
    })
}

async fn make_request(app: axum::Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

    let response = app.oneshot(req).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
}

#[tokio::test]
async fn test_preset_basic() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(800, 600, [255, 0, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let mut presets = HashMap::new();
    presets.insert("thumbnail".to_string(), "resize:fit:150:150/quality:80".to_string());

    let config = create_test_config(key.clone(), salt.clone(), false, presets, false);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/preset:thumbnail/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    let img = VipsImage::new_from_buffer(&body, "").unwrap();
    assert_eq!(img.get_width(), 150);
    assert_eq!(img.get_height(), 113);
}

#[tokio::test]
async fn test_preset_with_default() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(800, 600, [0, 255, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let mut presets = HashMap::new();
    presets.insert("default".to_string(), "quality:90/dpr:1".to_string());
    presets.insert("small".to_string(), "resize:fit:200:200".to_string());

    let config = create_test_config(key.clone(), salt.clone(), false, presets, false);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/preset:small/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    let img = VipsImage::new_from_buffer(&body, "").unwrap();
    assert_eq!(img.get_width(), 200);
    assert_eq!(img.get_height(), 150);
}

#[tokio::test]
async fn test_preset_default_only() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 300, [0, 0, 255, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let mut presets = HashMap::new();
    presets.insert("default".to_string(), "resize:fit:100:100".to_string());

    let config = create_test_config(key.clone(), salt.clone(), false, presets, false);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    let img = VipsImage::new_from_buffer(&body, "").unwrap();
    assert_eq!(img.get_width(), 100);
    assert_eq!(img.get_height(), 75);
}

#[tokio::test]
async fn test_preset_unknown_preset_error() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 300, [255, 255, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let presets = HashMap::new();
    let config = create_test_config(key.clone(), salt.clone(), false, presets, false);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/preset:nonexistent/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, _body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_only_presets_mode_allows_presets() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(800, 600, [255, 0, 255, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let mut presets = HashMap::new();
    presets.insert("thumbnail".to_string(), "resize:fit:150:150".to_string());

    let config = create_test_config(key.clone(), salt.clone(), false, presets, true);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/preset:thumbnail/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    let img = VipsImage::new_from_buffer(&body, "").unwrap();
    assert_eq!(img.get_width(), 150);
    assert_eq!(img.get_height(), 113);
}

#[tokio::test]
async fn test_only_presets_mode_rejects_non_presets() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(800, 600, [128, 128, 128, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let presets = HashMap::new();
    let config = create_test_config(key.clone(), salt.clone(), false, presets, true);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/resize:fit:300:300/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, _body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_only_presets_mode_allows_default() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 300, [200, 100, 50, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let mut presets = HashMap::new();
    presets.insert("default".to_string(), "resize:fit:50:50".to_string());

    let config = create_test_config(key.clone(), salt.clone(), false, presets, true);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    let img = VipsImage::new_from_buffer(&body, "").unwrap();
    assert_eq!(img.get_width(), 50);
    assert_eq!(img.get_height(), 38);
}

#[tokio::test]
async fn test_preset_short_form() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(800, 600, [100, 200, 50, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let mut presets = HashMap::new();
    presets.insert("thumb".to_string(), "resize:fit:100:100".to_string());

    let config = create_test_config(key.clone(), salt.clone(), false, presets, false);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/pr:thumb/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    let img = VipsImage::new_from_buffer(&body, "").unwrap();
    assert_eq!(img.get_width(), 100);
    assert_eq!(img.get_height(), 75);
}

#[tokio::test]
async fn test_preset_with_url_override() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(800, 600, [255, 128, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/test.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("content-type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let key = b"test_key".to_vec();
    let salt = b"test_salt".to_vec();

    let mut presets = HashMap::new();
    presets.insert("default".to_string(), "quality:80".to_string());

    let config = create_test_config(key.clone(), salt.clone(), false, presets, false);
    let state = create_test_state(config).await;

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(state);

    let source_url = format!("{}/test.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path_to_sign = format!("/resize:fit:200:200/quality:95/{}", encoded_url);
    let signature = generate_signature(&key, &salt, &path_to_sign);
    let uri = format!("/{}{}", signature, path_to_sign);

    let (status, body) = make_request(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    let img = VipsImage::new_from_buffer(&body, "").unwrap();
    assert_eq!(img.get_width(), 200);
    assert_eq!(img.get_height(), 150);
}
