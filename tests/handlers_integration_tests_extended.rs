use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use http_body_util::BodyExt;
use image::{ImageBuffer, Rgba};
use imgforge::app::AppState;
use imgforge::caching::cache::ImgforgeCache;
use imgforge::caching::config::CacheConfig;
use imgforge::config::Config;
use imgforge::handlers::image_forge_handler;
use imgforge::middleware::request_id_middleware;
use lazy_static::lazy_static;
use libvips::VipsApp;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

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

/// Helper function to create test config
fn create_test_config(key: Vec<u8>, salt: Vec<u8>, allow_unsigned: bool) -> Config {
    let mut config = Config::new(key, salt);
    config.workers = 4;
    config.allow_unsigned = allow_unsigned;
    config.allow_security_options = true;
    config
}

/// Helper function to create test AppState with specific cache
async fn create_test_state_with_cache(config: Config, cache: ImgforgeCache) -> Arc<AppState> {
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.download_timeout))
        .build()
        .expect("client builds");

    let metadata_cache = imgforge::caching::cache::MetadataCache::None;

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
async fn make_request(app: axum::Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();

    (status, body.to_vec())
}

#[tokio::test]
async fn test_image_caching_with_memory_cache() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [128, 128, 128, 255]);

    Mock::given(method("GET"))
        .and(path("/cache.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image.clone())
                .insert_header("Content-Type", "image/jpeg"),
        )
        .expect(1..)
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache_config = CacheConfig::Memory { capacity: 1024 * 1024 };
    let cache = ImgforgeCache::new(Some(cache_config)).await.unwrap();
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/cache.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state.clone());

    // First request - should hit the mock server
    let (status1, _body1) = make_request(app.clone(), &path).await;
    assert_eq!(status1, StatusCode::OK);

    // Second request - should hit the cache (but mock will still verify it's called only once if we want)
    let app2 = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));
    let (status2, _body2) = make_request(app2, &path).await;
    assert_eq!(status2, StatusCode::OK);
}

#[tokio::test]
async fn test_concurrent_image_processing() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(200, 200, [100, 150, 200, 255]);

    Mock::given(method("GET"))
        .and(path("/concurrent.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .expect(3)
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/concurrent.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());

    let mut handles = vec![];
    for i in 1..=3 {
        let state_clone = state.clone();
        let encoded_url_clone = encoded_url.clone();
        let handle = tokio::spawn(async move {
            let path = format!("/unsafe/resize:fit:{}:100/{}", i * 50, encoded_url_clone);
            let app = axum::Router::new()
                .route("/{*path}", axum::routing::get(image_forge_handler))
                .with_state(state_clone);
            make_request(app, &path).await
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    for result in results {
        let (status, _) = result.unwrap();
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
async fn test_image_forge_handler_with_all_options() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(800, 600, [255, 128, 64, 255]);

    Mock::given(method("GET"))
        .and(path("/all_options.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/all_options.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!(
        "/unsafe/resize:fit:300:200/quality:80/blur:1/sharpen:0.5/rotation:90/dpr:1.5/background:ffffff/padding:5:10/{}",
        encoded_url
    );

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_security_options_not_allowed() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(50, 50, [64, 128, 192, 255]);

    Mock::given(method("GET"))
        .and(path("/secure.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let mut config = create_test_config(vec![], vec![], true);
    config.allow_security_options = false;
    config.max_src_file_size = Some(100_000);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/secure.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    // Try to override with a larger limit, but it should be ignored
    let path = format!("/unsafe/max_src_file_size:999999/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _) = make_request(app, &path).await;
    // Should still work since the actual file size is within the server limit
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_large_image_processing() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(2000, 1500, [200, 100, 50, 255]);

    Mock::given(method("GET"))
        .and(path("/large.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/large.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/resize:fit:800:600/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_format_conversion_webp() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(150, 150, [100, 200, 100, 255]);

    Mock::given(method("GET"))
        .and(path("/convert.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/convert.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}.webp", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_image_with_transparency() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [255, 0, 0, 128]);

    Mock::given(method("GET"))
        .and(path("/transparent.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/transparent.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/resize:fit:50:50/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_complex_path_with_special_characters() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(100, 100, [50, 100, 150, 255]);

    Mock::given(method("GET"))
        .and(path("/path/to/image%20with%20spaces.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/path/to/image with spaces.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_resize_with_different_modes() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 300, [75, 150, 225, 255]);

    Mock::given(method("GET"))
        .and(path("/resize_modes.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image.clone())
                .insert_header("Content-Type", "image/jpeg"),
        )
        .expect(3)
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/resize_modes.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());

    let modes = vec!["fit", "fill", "auto"];
    for mode in modes {
        let path = format!("/unsafe/resize:{}:200:200/{}", mode, encoded_url);
        let app = axum::Router::new()
            .route("/{*path}", axum::routing::get(image_forge_handler))
            .with_state(state.clone());

        let (status, _) = make_request(app, &path).await;
        assert_eq!(status, StatusCode::OK, "Failed for resize mode: {}", mode);
    }
}

#[tokio::test]
async fn test_pixelate_effect() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(200, 200, [255, 128, 0, 255]);

    Mock::given(method("GET"))
        .and(path("/pixelate.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/pixelate.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/pixelate:15/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, _) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_brightness_effect() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(150, 150, [128, 128, 128, 255]);

    Mock::given(method("GET"))
        .and(path("/brightness.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image.clone())
                .insert_header("Content-Type", "image/jpeg"),
        )
        .expect(3..)
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let cache = ImgforgeCache::None;
    let state = create_test_state_with_cache(config, cache).await;

    let source_url = format!("{}/brightness.jpg", mock_server.uri());

    // Test increasing brightness
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/brightness:100/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    // Test decreasing brightness
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/brightness:-80/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());

    // Test with shorthand br
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/br:50/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body) = make_request(app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}
