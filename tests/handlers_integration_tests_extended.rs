use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use bytes::Bytes;
use http_body_util::BodyExt;
use image::{ImageBuffer, Rgba};
use imgforge::app::AppState;
use imgforge::caching::cache::ImgforgeCache;
use imgforge::caching::config::CacheConfig;
use imgforge::config::Config;
use imgforge::config::{SourcePattern, SourceRules};
use imgforge::handlers::{image_forge_handler, preflight_handler};
use imgforge::middleware::request_id_middleware;
use imgforge::MaxSourceFileSize;
use lazy_static::lazy_static;
use libvips::VipsApp;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OnceCell, Semaphore};
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
        watermark_cache: OnceCell::new(),
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
    config.max_src_file_size = Some(MaxSourceFileSize::new(100_000).unwrap());
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
async fn test_webp_options_are_accepted_without_encoder_crash() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(150, 150, [100, 200, 100, 255]);

    Mock::given(method("GET"))
        .and(path("/webpo.png"))
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

    let source_url = format!("{}/webpo.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/webpo:false:true:photo/{}.webp", encoded_url);

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

/// The oversized-padding guard, exercised through the handler rather than by
/// calling the transform directly: the directive has to survive parsing and the
/// service wiring and come back as a client error naming the problem.
///
/// Before the guard, this exact URL returned `200` with a 36x64 image — the
/// canvas wrapped negative in i32 and libvips quietly clipped it.
#[tokio::test]
async fn test_oversized_padding_returns_client_error() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(64, 64, [10, 200, 90, 255]);

    Mock::given(method("GET"))
        .and(path("/padding-overflow.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state_with_cache(config, ImgforgeCache::None).await;

    let source_url = format!("{}/padding-overflow.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    // 4_294_967_268 is -28 as i32.
    let path = format!("/unsafe/padding:0:4294967268:0:0/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body) = make_request(app, &path).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "oversized padding must not succeed");
    let message = String::from_utf8_lossy(&body);
    assert!(
        message.contains("padded canvas") && message.contains("exceeds the maximum"),
        "response should explain which limit was hit, got: {message}"
    );
}

/// A padded canvas that is large but legitimate still renders, so the guard
/// cannot be tightened into rejecting usable work without this failing.
#[tokio::test]
async fn test_large_but_valid_padding_still_renders() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(64, 64, [10, 200, 90, 255]);

    Mock::given(method("GET"))
        .and(path("/padding-ok.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state_with_cache(config, ImgforgeCache::None).await;

    let source_url = format!("{}/padding-ok.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/padding:100:100:100:100/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body) = make_request(app, &path).await;

    assert_eq!(status, StatusCode::OK);
    let decoded = image::load_from_memory(&body).expect("a decodable image");
    assert_eq!(
        image::GenericImageView::dimensions(&decoded),
        (264, 264),
        "64px source plus 100px on each side"
    );
}

/// `trim` end to end: a bordered source comes back without its border.
#[tokio::test]
async fn test_trim_removes_the_border_through_the_handler() {
    let mock_server = MockServer::start().await;

    // 200x140 white, with a 100x60 red block inset.
    let mut source: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(200, 140, Rgba([255, 255, 255, 255]));
    for y in 40..100 {
        for x in 50..150 {
            source.put_pixel(x, y, Rgba([200, 0, 0, 255]));
        }
    }
    let mut bytes: Vec<u8> = Vec::new();
    source
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/bordered.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(bytes)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let config = create_test_config(vec![], vec![], true);
    let state = create_test_state_with_cache(config, ImgforgeCache::None).await;

    let source_url = format!("{}/bordered.png", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let path = format!("/unsafe/trim:10/format:png/{}", encoded_url);

    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
        .layer(axum::middleware::from_fn(request_id_middleware));

    let (status, body) = make_request(app, &path).await;

    assert_eq!(status, StatusCode::OK);
    let decoded = image::load_from_memory(&body).expect("a decodable image");
    assert_eq!(
        image::GenericImageView::dimensions(&decoded),
        (100, 60),
        "the white border should be gone, leaving just the red block"
    );
}

/// A format alias picks the right encoder, so it has to pick the right media
/// type too. `format:tif` selected the TIFF encoder and then fell through
/// `format_to_content_type`'s catch-all, so clients received TIFF bytes
/// labelled `image/jpeg`.
#[tokio::test]
async fn format_aliases_are_described_by_their_own_media_type() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(64, 64, [90, 140, 200, 255]);

    Mock::given(method("GET"))
        .and(path("/alias.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image.clone())
                .insert_header("Content-Type", "image/jpeg"),
        )
        .expect(1..)
        .mount(&mock_server)
        .await;

    let source_url = format!("{}/alias.jpg", mock_server.uri());
    let encoded_url = URL_SAFE_NO_PAD.encode(source_url.as_bytes());

    for (requested, expected) in [("tif", "image/tiff"), ("tiff", "image/tiff"), ("jpg", "image/jpeg")] {
        let state = create_test_state_with_cache(create_test_config(vec![], vec![], true), ImgforgeCache::None).await;
        let app = axum::Router::new()
            .route("/{*path}", axum::routing::get(image_forge_handler))
            .with_state(state);

        let request = Request::builder()
            .uri(format!("/unsafe/format:{requested}/{encoded_url}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK, "format:{requested} should succeed");
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert_eq!(
            content_type, expected,
            "format:{requested} produced {expected} bytes but announced {content_type}"
        );
    }
}
// ---------------------------------------------------------------------------
// Delivery-layer coverage: content negotiation, caching headers, source
// restrictions and the pass-through paths.
// ---------------------------------------------------------------------------

fn delivery_png(width: u32, height: u32) -> Vec<u8> {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba([12, 34, 56, 255]));
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

fn delivery_config() -> Config {
    let mut config = Config::new(vec![0u8; 32], vec![0u8; 32]);
    config.workers = 2;
    config.allow_unsigned = true;
    config
}

async fn delivery_state(config: Config) -> Arc<AppState> {
    let http_client = imgforge::app::build_http_client(&config).expect("client builds");

    Arc::new(AppState {
        semaphore: Arc::new(Semaphore::new(config.workers)),
        cache: ImgforgeCache::None,
        metadata_cache: imgforge::caching::cache::MetadataCache::None,
        rate_limiter: None,
        config,
        vips_app: VIPS_APP.clone(),
        http_client,
        watermark_cache: OnceCell::new(),
    })
}

fn delivery_router(state: Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state)
}

struct DeliveryResponse {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: Bytes,
}

/// A delivery request against a caller-supplied cache, so two requests under
/// different configurations can share one.
async fn delivery_request_with_cache(
    config: Config,
    cache: ImgforgeCache,
    uri: &str,
    headers: &[(&str, &str)],
) -> DeliveryResponse {
    let http_client = imgforge::app::build_http_client(&config).expect("client builds");
    let state = Arc::new(AppState {
        semaphore: Arc::new(Semaphore::new(config.workers)),
        cache,
        metadata_cache: imgforge::caching::cache::MetadataCache::None,
        rate_limiter: None,
        config,
        vips_app: VIPS_APP.clone(),
        http_client,
        watermark_cache: OnceCell::new(),
    });
    delivery_request(state, uri, headers).await
}

async fn delivery_request(state: Arc<AppState>, uri: &str, headers: &[(&str, &str)]) -> DeliveryResponse {
    let mut builder = Request::builder().uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = delivery_router(state)
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();

    DeliveryResponse {
        status: response.status(),
        headers: response.headers().clone(),
        body: response.into_body().collect().await.unwrap().to_bytes(),
    }
}

fn delivery_header(response: &DeliveryResponse, name: &str) -> Option<String> {
    response
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// Serves a PNG and returns the imgforge path that fetches it.
async fn delivery_source(server: &MockServer, extra_headers: &[(&str, &str)]) -> String {
    let mut template = ResponseTemplate::new(200)
        .set_body_bytes(delivery_png(40, 30))
        .insert_header("Content-Type", "image/png");
    for (name, value) in extra_headers {
        template = template.insert_header(*name, *value);
    }

    Mock::given(method("GET"))
        .and(path("/image.png"))
        .respond_with(template)
        .mount(server)
        .await;

    URL_SAFE_NO_PAD.encode(format!("{}/image.png", server.uri()))
}

#[tokio::test]
async fn accept_negotiation_upgrades_the_format_and_marks_the_response_as_varying() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;

    let mut config = delivery_config();
    config.enable_webp_detection = true;
    let state = delivery_state(config).await;

    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");
    let response = delivery_request(state.clone(), &uri, &[("accept", "image/webp,image/*,*/*")]).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        delivery_header(&response, "content-type").as_deref(),
        Some("image/webp")
    );
    // Without this a shared cache would serve the WebP to a client that asked
    // for anything but.
    assert_eq!(delivery_header(&response, "vary").as_deref(), Some("Accept"));

    // A client that does not advertise WebP keeps the source's own format.
    let response = delivery_request(state, &uri, &[("accept", "image/png,*/*")]).await;
    assert_eq!(delivery_header(&response, "content-type").as_deref(), Some("image/png"));
}

#[tokio::test]
async fn an_explicit_format_is_only_overridden_by_enforcement() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;
    let uri = format!("/unsafe/format:png/{encoded}");
    let accept = [("accept", "image/webp")];

    let mut detecting = delivery_config();
    detecting.enable_webp_detection = true;
    let response = delivery_request(delivery_state(detecting).await, &uri, &accept).await;
    assert_eq!(delivery_header(&response, "content-type").as_deref(), Some("image/png"));

    let mut enforcing = delivery_config();
    enforcing.enforce_webp = true;
    let response = delivery_request(delivery_state(enforcing).await, &uri, &accept).await;
    assert_eq!(
        delivery_header(&response, "content-type").as_deref(),
        Some("image/webp")
    );
}

#[tokio::test]
async fn an_entity_tag_turns_a_repeat_request_into_a_304() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;

    let mut config = delivery_config();
    config.use_etag = true;
    let state = delivery_state(config).await;

    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");
    let first = delivery_request(state.clone(), &uri, &[]).await;
    let etag = delivery_header(&first, "etag").expect("an etag should be sent");
    assert!(!first.body.is_empty());

    let second = delivery_request(state.clone(), &uri, &[("if-none-match", &etag)]).await;
    assert_eq!(second.status, StatusCode::NOT_MODIFIED);
    assert!(second.body.is_empty(), "a 304 carries no body");
    assert_eq!(delivery_header(&second, "etag").as_deref(), Some(etag.as_str()));

    // A stale tag still gets the image.
    let stale = delivery_request(state, &uri, &[("if-none-match", "\"stale\"")]).await;
    assert_eq!(stale.status, StatusCode::OK);
    assert!(!stale.body.is_empty());
}

#[tokio::test]
async fn cache_control_comes_from_the_ttl_or_the_origin() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[("Cache-Control", "public, max-age=99")]).await;
    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");

    // Nothing configured: imgforge stays out of the client's caching decisions.
    let response = delivery_request(delivery_state(delivery_config()).await, &uri, &[]).await;
    assert_eq!(delivery_header(&response, "cache-control"), None);

    let mut with_ttl = delivery_config();
    with_ttl.ttl = Some(600);
    let response = delivery_request(delivery_state(with_ttl).await, &uri, &[]).await;
    assert_eq!(
        delivery_header(&response, "cache-control").as_deref(),
        Some("max-age=600, public")
    );

    let mut passthrough = delivery_config();
    passthrough.ttl = Some(600);
    passthrough.cache_control_passthrough = true;
    let response = delivery_request(delivery_state(passthrough).await, &uri, &[]).await;
    assert_eq!(
        delivery_header(&response, "cache-control").as_deref(),
        Some("public, max-age=99")
    );
}

/// A bearer-protected deployment must not mark responses shared-cacheable:
/// `public` invites a CDN to replay the authorised answer to a request that
/// carries no token. Neither the TTL default nor the origin's own policy gets
/// a say — the origin cannot know a token now guards it.
#[tokio::test]
async fn bearer_protected_responses_are_never_publicly_cacheable() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[("Cache-Control", "public, max-age=99")]).await;
    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");
    let auth = [("authorization", "Bearer token")];

    let mut config = delivery_config();
    config.secret = Some("token".to_string());
    config.ttl = Some(600);
    config.cache_control_passthrough = true;
    let response = delivery_request(delivery_state(config).await, &uri, &auth).await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        delivery_header(&response, "cache-control").as_deref(),
        Some("max-age=600, private"),
        "passthrough must not forward the origin's public policy"
    );

    // With no TTL the refusal still has to be said out loud, or a heuristic
    // cache decides for itself.
    let mut config = delivery_config();
    config.secret = Some("token".to_string());
    let response = delivery_request(delivery_state(config).await, &uri, &auth).await;
    assert_eq!(delivery_header(&response, "cache-control").as_deref(), Some("private"));
}

/// `Authorization` is not a safelisted request header, so a browser holding a
/// bearer token sends OPTIONS before the real request. A router answering only
/// GET turned that preflight into a 405, and no header on the eventual GET
/// could repair it.
#[tokio::test]
async fn a_cors_preflight_is_answered_when_an_origin_is_allowed() {
    let preflight = |state: Arc<AppState>| async {
        let app = axum::Router::new()
            .route(
                "/{*path}",
                axum::routing::get(image_forge_handler).options(preflight_handler),
            )
            .with_state(state);
        app.oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/unsafe/rs:fit:20:20/whatever")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    };

    let mut config = delivery_config();
    config.allow_origin = Some("https://app.example.test".to_string());
    let response = preflight(delivery_state(config).await).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response.headers().get("access-control-allow-origin").unwrap(),
        "https://app.example.test"
    );
    let allowed_headers = response
        .headers()
        .get("access-control-allow-headers")
        .expect("the preflight names the headers it permits")
        .to_str()
        .unwrap();
    assert!(allowed_headers.contains("Authorization"), "got: {allowed_headers}");

    // Without a configured origin there is nothing to grant.
    let response = preflight(delivery_state(delivery_config()).await).await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn last_modified_and_canonical_headers_describe_the_source() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT")]).await;
    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");

    let mut config = delivery_config();
    config.last_modified_enabled = true;
    config.set_canonical_header = true;
    let response = delivery_request(delivery_state(config).await, &uri, &[]).await;

    assert_eq!(
        delivery_header(&response, "last-modified").as_deref(),
        Some("Wed, 21 Oct 2015 07:28:00 GMT")
    );
    let link = delivery_header(&response, "link").expect("a canonical link should be sent");
    assert!(link.starts_with('<') && link.ends_with("; rel=\"canonical\""), "{link}");
    assert!(link.contains("/image.png"));
}

#[tokio::test]
async fn a_source_outside_the_allow_list_is_refused() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;
    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");

    let mut config = delivery_config();
    config.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse("https://images.example.com/")],
    };

    let response = delivery_request(delivery_state(config).await, &uri, &[]).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&response.body).contains("not allowed"));
}

#[tokio::test]
async fn a_base_url_lets_a_url_carry_only_the_path() {
    let server = MockServer::start().await;
    delivery_source(&server, &[]).await;

    let mut config = delivery_config();
    config.source_rules = SourceRules {
        base_url: Some(server.uri()),
        allowed: Vec::new(),
    };

    let encoded = URL_SAFE_NO_PAD.encode("image.png");
    let response = delivery_request(
        delivery_state(config).await,
        &format!("/unsafe/rs:fit:20:20/{encoded}"),
        &[],
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(!response.body.is_empty());
}

#[tokio::test]
async fn cross_origin_and_debug_headers_are_emitted_when_configured() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;

    let mut config = delivery_config();
    config.allow_origin = Some("https://app.example.com".to_string());
    config.enable_debug_headers = true;

    let response = delivery_request(
        delivery_state(config).await,
        &format!("/unsafe/rs:fit:20:20/{encoded}"),
        &[],
    )
    .await;

    assert_eq!(
        delivery_header(&response, "access-control-allow-origin").as_deref(),
        Some("https://app.example.com")
    );
    assert_eq!(delivery_header(&response, "x-origin-width").as_deref(), Some("40"));
    assert_eq!(delivery_header(&response, "x-origin-height").as_deref(), Some("30"));
    assert_eq!(delivery_header(&response, "x-result-width").as_deref(), Some("20"));
    assert_eq!(delivery_header(&response, "x-result-height").as_deref(), Some("15"));
}

#[tokio::test]
async fn skip_processing_returns_the_source_bytes_untouched() {
    let server = MockServer::start().await;
    let source = delivery_png(40, 30);
    Mock::given(method("GET"))
        .and(path("/image.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(source.clone())
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&server)
        .await;
    let encoded = URL_SAFE_NO_PAD.encode(format!("{}/image.png", server.uri()));

    // A resize is requested, and skipped, because the source format is listed.
    let response = delivery_request(
        delivery_state(delivery_config()).await,
        &format!("/unsafe/skp:png/rs:fit:10:10/{encoded}"),
        &[],
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body.as_ref(), source.as_slice());

    // Asking for a different format is a conversion, which cannot be skipped.
    let response = delivery_request(
        delivery_state(delivery_config()).await,
        &format!("/unsafe/skp:png/format:jpeg/rs:fit:10:10/{encoded}"),
        &[],
    )
    .await;
    assert_eq!(
        delivery_header(&response, "content-type").as_deref(),
        Some("image/jpeg")
    );
    assert_ne!(response.body.as_ref(), source.as_slice());
}

#[tokio::test]
async fn client_hints_size_a_url_that_left_the_width_open() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;

    let mut config = delivery_config();
    config.enable_client_hints = true;
    let state = delivery_state(config).await;

    let response = delivery_request(state.clone(), &format!("/unsafe/{encoded}"), &[("width", "20")]).await;
    let decoded = image::load_from_memory(&response.body).expect("result decodes");
    assert_eq!(decoded.width(), 20);

    // A URL that names its own width is not overridden by the hint.
    let response = delivery_request(state, &format!("/unsafe/rs:fit:10:10/{encoded}"), &[("width", "20")]).await;
    let decoded = image::load_from_memory(&response.body).expect("result decodes");
    assert_eq!(decoded.width(), 10);
}

#[tokio::test]
async fn an_upstream_failure_names_the_upstream_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing.png"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let encoded = URL_SAFE_NO_PAD.encode(format!("{}/missing.png", server.uri()));

    let response = delivery_request(
        delivery_state(delivery_config()).await,
        &format!("/unsafe/{encoded}"),
        &[],
    )
    .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(
        String::from_utf8_lossy(&response.body).contains("404"),
        "the response should name the upstream status, got: {}",
        String::from_utf8_lossy(&response.body)
    );
}

/// An allowed origin that redirects elsewhere must not carry the request past
/// the allow list. Checking only the URL the caller supplied is checking the
/// wrong thing — it is the classic way an image proxy becomes an SSRF gadget.
#[tokio::test]
async fn a_redirect_out_of_the_allow_list_is_refused() {
    let origin = MockServer::start().await;
    let elsewhere = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/internal.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(delivery_png(40, 30))
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&elsewhere)
        .await;

    Mock::given(method("GET"))
        .and(path("/image.png"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", format!("{}/internal.png", elsewhere.uri())))
        .mount(&origin)
        .await;

    let mut config = delivery_config();
    // Only the first server is permitted; the redirect points at the second.
    config.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse(&origin.uri())],
    };

    let encoded = URL_SAFE_NO_PAD.encode(format!("{}/image.png", origin.uri()));
    let response = delivery_request(
        delivery_state(config).await,
        &format!("/unsafe/rs:fit:20:20/{encoded}"),
        &[],
    )
    .await;

    assert_ne!(
        response.status,
        StatusCode::OK,
        "the redirect target was outside the allow list and must not have been fetched"
    );
}

/// With no allow list configured, redirects are followed as before.
#[tokio::test]
async fn a_redirect_is_followed_when_no_allow_list_is_configured() {
    let origin = MockServer::start().await;
    let elsewhere = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/final.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(delivery_png(40, 30))
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&elsewhere)
        .await;
    Mock::given(method("GET"))
        .and(path("/start.png"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", format!("{}/final.png", elsewhere.uri())))
        .mount(&origin)
        .await;

    let encoded = URL_SAFE_NO_PAD.encode(format!("{}/start.png", origin.uri()));
    let response = delivery_request(
        delivery_state(delivery_config()).await,
        &format!("/unsafe/rs:fit:20:20/{encoded}"),
        &[],
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(!response.body.is_empty());
}

/// The response has to tell shared caches which request headers it varies by,
/// or a CDN reuses one client's dimensions for another.
#[tokio::test]
async fn client_hints_are_named_in_vary() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;

    let mut config = delivery_config();
    config.enable_client_hints = true;
    config.enable_webp_detection = true;

    let response = delivery_request(
        delivery_state(config).await,
        &format!("/unsafe/rs:fit:20:20/{encoded}"),
        &[],
    )
    .await;

    let vary = delivery_header(&response, "vary").expect("a vary header should be sent");
    for expected in ["Accept", "Width", "DPR"] {
        assert!(vary.contains(expected), "expected {expected} in Vary, got: {vary}");
    }
}

/// `Last-Modified` is a validator, so a client returning it should get a 304
/// rather than the whole image again.
#[tokio::test]
async fn if_modified_since_is_honoured() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT")]).await;

    let mut config = delivery_config();
    config.last_modified_enabled = true;

    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");
    let state = delivery_state(config).await;

    let first = delivery_request(state.clone(), &uri, &[]).await;
    let last_modified = delivery_header(&first, "last-modified").expect("a validator should be sent");

    let second = delivery_request(state, &uri, &[("if-modified-since", &last_modified)]).await;
    assert_eq!(second.status, StatusCode::NOT_MODIFIED);
    assert!(second.body.is_empty());
}

/// RFC 9110 gives `If-None-Match` precedence over `If-Modified-Since` when the
/// request carries both. Keying that on whether imgforge *emitted* an ETag
/// instead meant a deployment with both validators enabled never looked at
/// `If-Modified-Since` at all, and sent the whole body to a client that had
/// asked, correctly, whether it needed one.
#[tokio::test]
async fn if_modified_since_still_works_when_etags_are_enabled() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT")]).await;

    let mut config = delivery_config();
    config.last_modified_enabled = true;
    config.use_etag = true;

    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");
    let state = delivery_state(config).await;

    let first = delivery_request(state.clone(), &uri, &[]).await;
    let last_modified = delivery_header(&first, "last-modified").expect("a validator should be sent");
    let etag = delivery_header(&first, "etag").expect("an entity tag should be sent too");

    // The date alone, with no entity tag to fall back on.
    let by_date = delivery_request(state.clone(), &uri, &[("if-modified-since", &last_modified)]).await;
    assert_eq!(
        by_date.status,
        StatusCode::NOT_MODIFIED,
        "an ETag being available must not stop the date from being read"
    );
    assert!(by_date.body.is_empty());

    // The tag still wins when both are present.
    let by_tag = delivery_request(
        state.clone(),
        &uri,
        &[("if-none-match", &etag), ("if-modified-since", &last_modified)],
    )
    .await;
    assert_eq!(by_tag.status, StatusCode::NOT_MODIFIED);

    // A stale tag is a mismatch, and a mismatch means send the body — even
    // though the date would have said otherwise. That is the precedence rule,
    // and it is why the date cannot simply be checked as well.
    let stale_tag = delivery_request(
        state,
        &uri,
        &[
            ("if-none-match", "\"not-the-current-tag\""),
            ("if-modified-since", &last_modified),
        ],
    )
    .await;
    assert_eq!(stale_tag.status, StatusCode::OK);
    assert!(!stale_tag.body.is_empty());
}

/// A watermark is a source like any other. `watermark_url` names a URL that
/// imgforge fetches server-side, and it went straight to the HTTP client with no
/// allow-list check — so the option pointed at an internal address and imgforge
/// fetched it, which is exactly the SSRF `IMGFORGE_ALLOWED_SOURCES` exists to
/// prevent. The redirect policy only guards the hops after the first request.
#[tokio::test]
async fn a_watermark_url_outside_the_allow_list_is_refused() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;

    // The watermark lives on the same mock server as the image, so the only
    // thing deciding the outcome is whether the allow list is consulted.
    let watermark_url = format!("{}/image.png", server.uri());
    let encoded_watermark = URL_SAFE_NO_PAD.encode(watermark_url.as_bytes());
    let uri = format!("/unsafe/rs:fit:20:20/wm:0.5/wmu:{encoded_watermark}/{encoded}");

    // With the mock server allowed, the watermark is fetched and the request
    // succeeds — proving the URL itself is good.
    let mut permissive = delivery_config();
    permissive.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse(&format!("{}/", server.uri()))],
    };
    let allowed = delivery_request(delivery_state(permissive).await, &uri, &[]).await;
    assert_eq!(
        allowed.status,
        StatusCode::OK,
        "the watermark URL is inside the allow list and should be fetched"
    );

    // Now allow only an unrelated host. The main image is refused, which is the
    // established behaviour...
    let mut restrictive = delivery_config();
    restrictive.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse("https://images.example.com/")],
    };
    let refused = delivery_request(delivery_state(restrictive).await, &uri, &[]).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&refused.body).contains("not allowed"));

    // ...and the watermark has to be refused on its own account too, not merely
    // because the image beside it was. Here the image is permitted and only the
    // watermark is out of bounds.
    let mut image_only = delivery_config();
    image_only.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse(&format!("{}/image.png", server.uri()))],
    };
    let watermark_elsewhere = URL_SAFE_NO_PAD.encode(b"http://169.254.169.254/latest/meta-data/");
    let smuggled = format!("/unsafe/rs:fit:20:20/wm:0.5/wmu:{watermark_elsewhere}/{encoded}");
    let response = delivery_request(delivery_state(image_only).await, &smuggled, &[]).await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "a watermark URL outside the allow list must not be fetched"
    );
    assert!(String::from_utf8_lossy(&response.body).contains("not allowed"));
}

/// The allow list has to be consulted before the cache answers. A persistent
/// cache outlives the configuration that filled it, so a source that is no
/// longer permitted would otherwise keep being served from the entry it left
/// behind — the request never reaches the check because it never reaches the
/// fetch.
#[tokio::test]
async fn a_cache_hit_does_not_outlive_the_source_allow_list() {
    let server = MockServer::start().await;
    let encoded = delivery_source(&server, &[]).await;
    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");

    let cache = ImgforgeCache::new(Some(CacheConfig::Memory { capacity: 1024 * 1024 }))
        .await
        .unwrap();

    // Fill the cache while the source is permitted.
    let mut permissive = delivery_config();
    permissive.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse(&format!("{}/", server.uri()))],
    };
    let warm = delivery_request_with_cache(permissive, cache.clone(), &uri, &[]).await;
    assert_eq!(warm.status, StatusCode::OK);

    // Tighten the rules over the same cache. The entry is still there, and must
    // not be reachable.
    let mut restrictive = delivery_config();
    restrictive.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse("https://images.example.com/")],
    };
    let response = delivery_request_with_cache(restrictive, cache, &uri, &[]).await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "a cached entry must not survive the source that produced it being disallowed"
    );
}

/// A `raw` response returns origin bytes with nothing between them and the
/// client, and every source limit is checked after the cache lookup. Without the
/// limits in the cache identity, an entry stored under a loose policy kept being
/// served once the policy was tightened — the request was answered before the
/// check it should have failed.
#[tokio::test]
async fn a_cached_passthrough_does_not_outlive_the_source_limits() {
    let mock_server = MockServer::start().await;
    let test_image = create_test_image(400, 400, [10, 20, 30, 255]);

    Mock::given(method("GET"))
        .and(path("/passthrough.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(test_image)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&mock_server)
        .await;

    let source_url = format!("{}/passthrough.png", mock_server.uri());
    let encoded = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
    let uri = format!("/unsafe/raw:1/{encoded}");

    let cache = ImgforgeCache::new(Some(CacheConfig::Memory { capacity: 1024 * 1024 }))
        .await
        .unwrap();

    let respond = |config: Config, cache: ImgforgeCache, uri: String| async move {
        let state = create_test_state_with_cache(config, cache).await;
        let app = axum::Router::new()
            .route("/{*path}", axum::routing::get(image_forge_handler))
            .with_state(state);
        make_request(app, &uri).await.0
    };

    // Warm the cache while nothing restricts the source.
    let warm = respond(create_test_config(vec![], vec![], true), cache.clone(), uri.clone()).await;
    assert_eq!(warm, StatusCode::OK, "the passthrough should succeed while permitted");

    // Now forbid the source's resolution over the same cache. 400x400 is
    // 160,000 pixels, well over a 0.05 MP ceiling.
    let mut restrictive = create_test_config(vec![], vec![], true);
    restrictive.max_src_resolution = Some("0.05".parse().unwrap());
    assert_eq!(
        respond(restrictive, cache.clone(), uri.clone()).await,
        StatusCode::BAD_REQUEST,
        "a cached passthrough must not survive the limit that now forbids it"
    );

    // The same for a MIME restriction the source does not satisfy.
    let mut mime_restricted = create_test_config(vec![], vec![], true);
    mime_restricted.allowed_mime_types = Some(vec!["image/jpeg".to_string()]);
    assert_eq!(
        respond(mime_restricted, cache, uri).await,
        StatusCode::BAD_REQUEST,
        "a cached passthrough must not survive a MIME policy that now forbids it"
    );
}

/// `/info` has to consult the allow list before its cache, exactly as the image
/// path does. A persistent metadata cache outlives the configuration that filled
/// it, so a source removed from `IMGFORGE_ALLOWED_SOURCES` kept being described
/// from the entry it left behind — the endpoint answered before reaching the
/// check that should have refused it.
#[tokio::test]
async fn info_does_not_describe_a_source_the_allow_list_now_forbids() {
    let server = MockServer::start().await;
    let image = create_test_image(64, 48, [10, 20, 30, 255]);

    Mock::given(method("GET"))
        .and(path("/described.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(image)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&server)
        .await;

    let encoded = URL_SAFE_NO_PAD.encode(format!("{}/described.png", server.uri()).as_bytes());
    let uri = format!("/info/unsafe/{encoded}");

    let metadata_cache =
        imgforge::caching::cache::MetadataCache::new(Some(CacheConfig::Memory { capacity: 1024 * 1024 }))
            .await
            .unwrap();

    let respond = |config: Config, cache: imgforge::caching::cache::MetadataCache, uri: String| async move {
        let http_client = imgforge::app::build_http_client(&config).expect("client builds");
        let state = Arc::new(AppState {
            semaphore: Arc::new(Semaphore::new(config.workers)),
            cache: ImgforgeCache::None,
            metadata_cache: cache,
            rate_limiter: None,
            config,
            vips_app: VIPS_APP.clone(),
            http_client,
            watermark_cache: OnceCell::new(),
        });
        let app = axum::Router::new()
            .route("/info/{*path}", axum::routing::get(imgforge::handlers::info_handler))
            .with_state(state);
        make_request(app, &uri).await.0
    };

    // Populate the metadata cache while the source is permitted.
    let mut permissive = create_test_config(vec![], vec![], true);
    permissive.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse(&format!("{}/", server.uri()))],
    };
    assert_eq!(
        respond(permissive, metadata_cache.clone(), uri.clone()).await,
        StatusCode::OK,
        "the source is permitted, so /info should describe it"
    );

    // Tighten the allow list over the same cache.
    let mut restrictive = create_test_config(vec![], vec![], true);
    restrictive.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse("https://images.example.com/")],
    };
    assert_eq!(
        respond(restrictive, metadata_cache, uri).await,
        StatusCode::BAD_REQUEST,
        "cached metadata must not outlive the source being disallowed"
    );
}

/// The allow list is checked against the URL a request *asks* for, and a
/// redirect can move the answer somewhere else. The redirect policy catches that
/// as it happens, which leaves the cache: an entry outlives the policy that
/// admitted it, so a hit has to be rechecked against where its bytes came from.
#[tokio::test]
async fn a_cached_redirect_destination_is_revalidated_on_a_hit() {
    let origin = MockServer::start().await;
    let image = create_test_image(48, 48, [20, 90, 140, 255]);

    // /entry redirects to /final on the same server, so both are reachable and
    // only the allow list decides the outcome.
    Mock::given(method("GET"))
        .and(path("/final.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(image)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&origin)
        .await;
    Mock::given(method("GET"))
        .and(path("/entry.png"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", format!("{}/final.png", origin.uri()).as_str()),
        )
        .mount(&origin)
        .await;

    let encoded = URL_SAFE_NO_PAD.encode(format!("{}/entry.png", origin.uri()).as_bytes());
    let uri = format!("/unsafe/rs:fit:20:20/{encoded}");

    let cache = ImgforgeCache::new(Some(CacheConfig::Memory { capacity: 1024 * 1024 }))
        .await
        .unwrap();

    // Built through the real client factory so the redirect policy is active —
    // the point of the test is what happens across a redirect, and a plain
    // reqwest client follows them without consulting the allow list at all.
    let respond = |config: Config, cache: ImgforgeCache, uri: String| async move {
        let http_client = imgforge::app::build_http_client(&config).expect("client builds");
        let state = Arc::new(AppState {
            semaphore: Arc::new(Semaphore::new(config.workers)),
            cache,
            metadata_cache: imgforge::caching::cache::MetadataCache::None,
            rate_limiter: None,
            config,
            vips_app: VIPS_APP.clone(),
            http_client,
            watermark_cache: OnceCell::new(),
        });
        let app = axum::Router::new()
            .route("/{*path}", axum::routing::get(image_forge_handler))
            .with_state(state);
        make_request(app, &uri).await.0
    };

    // Both entry and destination permitted: the redirect is followed and cached.
    let mut permissive = create_test_config(vec![], vec![], true);
    permissive.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse(&format!("{}/", origin.uri()))],
    };
    assert_eq!(
        respond(permissive, cache.clone(), uri.clone()).await,
        StatusCode::OK,
        "both hops are allowed, so the fetch should succeed"
    );

    // Now permit only the entry point. The cached bytes came from /final.png,
    // which is no longer allowed, so the entry must not be served — and the
    // re-fetch fails at the redirect for the same reason.
    let mut entry_only = create_test_config(vec![], vec![], true);
    entry_only.source_rules = SourceRules {
        base_url: None,
        allowed: vec![SourcePattern::parse(&format!("{}/entry.png", origin.uri()))],
    };
    assert_eq!(
        respond(entry_only, cache, uri).await,
        StatusCode::BAD_REQUEST,
        "a cached entry must not outlive the destination it was fetched from"
    );
}

/// Both of these settings change the bytes of a response whose URL never
/// changes, and neither carries a version bump to retire what it invalidates.
/// The cache key has to carry them, and the request path has to actually pass
/// them — a key that accepts the input is no use if nothing supplies it.
#[tokio::test]
async fn cached_bytes_do_not_outlive_the_config_that_produced_them() {
    let server = MockServer::start().await;
    let source = create_test_image(80, 80, [200, 120, 40, 255]);

    Mock::given(method("GET"))
        .and(path("/subject.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(source)
                .insert_header("Content-Type", "image/png"),
        )
        .mount(&server)
        .await;

    let encoded = URL_SAFE_NO_PAD.encode(format!("{}/subject.png", server.uri()).as_bytes());

    let respond = |config: Config, cache: ImgforgeCache, uri: String| async move {
        let state = create_test_state_with_cache(config, cache).await;
        let app = axum::Router::new()
            .route("/{*path}", axum::routing::get(image_forge_handler))
            .with_state(state);
        make_request(app, &uri).await.1
    };

    // A configured default quality: lowering it must change what clients get,
    // not be masked by an entry stored under the old one.
    {
        let uri = format!("/unsafe/rs:fit:60:60/format:jpeg/{encoded}");
        let cache = ImgforgeCache::new(Some(CacheConfig::Memory { capacity: 1024 * 1024 }))
            .await
            .unwrap();

        let quality_config = |quality: u8| {
            let mut config = create_test_config(vec![], vec![], true);
            config.option_defaults.quality = Some(quality);
            config
        };

        // Compared by size rather than by bytes: a lower quality is a smaller
        // JPEG, and a failure then prints two numbers instead of two images.
        let high = respond(quality_config(95), cache.clone(), uri.clone()).await.len();
        let low = respond(quality_config(20), cache, uri).await.len();
        assert!(
            low < high,
            "lowering IMGFORGE_QUALITY must not be outrun by the entry stored under the old value \
             (q20 gave {low} bytes, q95 gave {high})"
        );
    }

    // A server-side watermark: repointing it must change every watermarked
    // response, and `watermark:1` names no image of its own.
    {
        let dir = tempfile::tempdir().expect("a temp dir");
        let red = dir.path().join("red.png");
        let blue = dir.path().join("blue.png");
        std::fs::write(&red, create_test_image(20, 20, [255, 0, 0, 255])).unwrap();
        std::fs::write(&blue, create_test_image(20, 20, [0, 0, 255, 255])).unwrap();

        let uri = format!("/unsafe/rs:fit:60:60/wm:1/format:png/{encoded}");
        let cache = ImgforgeCache::new(Some(CacheConfig::Memory { capacity: 1024 * 1024 }))
            .await
            .unwrap();

        let watermark_config = |path: &std::path::Path| {
            let mut config = create_test_config(vec![], vec![], true);
            config.watermark_path = Some(path.to_string_lossy().into_owned());
            config
        };

        // Compared as mean channel values: a red overlay and a blue one differ
        // in a way two byte vectors cannot report readably.
        let channel_means = |body: &[u8]| {
            let image = image::load_from_memory(body).expect("a decodable image").to_rgb8();
            let count = image.pixels().len() as f64;
            (
                image.pixels().map(|p| f64::from(p[0])).sum::<f64>() / count,
                image.pixels().map(|p| f64::from(p[2])).sum::<f64>() / count,
            )
        };
        let (red_r, red_b) = channel_means(&respond(watermark_config(&red), cache.clone(), uri.clone()).await);
        let (blue_r, blue_b) = channel_means(&respond(watermark_config(&blue), cache, uri).await);
        assert!(
            blue_b > red_b && red_r > blue_r,
            "repointing IMGFORGE_WATERMARK_PATH must retire the entries composited with the old logo \
             (red logo gave r={red_r:.1} b={red_b:.1}, blue logo gave r={blue_r:.1} b={blue_b:.1})"
        );
    }
}
