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

    // Warm the cache while nothing restricts the source.
    let permissive = create_test_config(vec![], vec![], true);
    let state = create_test_state_with_cache(permissive, cache.clone()).await;
    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state);
    let (warm, _) = make_request(app, &uri).await;
    assert_eq!(warm, StatusCode::OK, "the passthrough should succeed while permitted");

    // Now forbid the source's resolution, over the same cache. 400x400 is
    // 160,000 pixels, well over a 0.05 MP ceiling.
    let mut restrictive = create_test_config(vec![], vec![], true);
    restrictive.max_src_resolution = Some("0.05".parse().unwrap());
    let state = create_test_state_with_cache(restrictive, cache.clone()).await;
    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state);
    let (status, _) = make_request(app, &uri).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a cached passthrough must not survive the limit that now forbids it"
    );

    // The same for a MIME restriction that the source does not satisfy.
    let mut mime_restricted = create_test_config(vec![], vec![], true);
    mime_restricted.allowed_mime_types = Some(vec!["image/jpeg".to_string()]);
    let state = create_test_state_with_cache(mime_restricted, cache).await;
    let app = axum::Router::new()
        .route("/{*path}", axum::routing::get(image_forge_handler))
        .with_state(state);
    let (status, _) = make_request(app, &uri).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a cached passthrough must not survive a MIME policy that now forbids it"
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
