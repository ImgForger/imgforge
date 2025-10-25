use crate::caching::cache::ImgforgeCache as Cache;
use crate::caching::config::CacheConfig;
use crate::config::Config;
use crate::constants::*;
use crate::handlers::{image_forge_handler, info_handler, status_handler, AppState};
use crate::middleware;
use crate::monitoring;
use axum::{extract::Request, routing::get, Router};
use axum_prometheus::PrometheusMetricLayer;

use governor::{Quota, RateLimiter};
use libvips::VipsApp;
use rand::distr::Alphanumeric;
use rand::Rng;
use std::env;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, info_span, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

fn init_vips() -> VipsApp {
    match VipsApp::new("imgforge", false) {
        Ok(app) => app,
        Err(e) => {
            panic!("Failed to initialize libvips: {}", e);
        }
    }
}

fn generate_request_id() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

pub async fn start() {
    let config = Config::from_env().expect("Failed to load config");
    let semaphore = Semaphore::new(config.workers);
    let cache_config = CacheConfig::from_env().expect("Failed to load cache config");
    let cache = Cache::new(cache_config).await.expect("Failed to initialize cache");

    let rate_limiter = match env::var(ENV_RATE_LIMIT_PER_MINUTE) {
        Ok(s) => {
            let limit = s
                .parse::<u32>()
                .expect("IMGFORGE_RATE_LIMIT_PER_MINUTE must be a valid integer");
            if limit > 0 {
                info!("Rate limiting enabled: {} requests per minute", limit);
                Some(RateLimiter::direct(Quota::per_minute(
                    NonZeroU32::new(limit).expect("Rate limit must be greater than 0"),
                )))
            } else {
                info!("Rate limiting disabled: IMGFORGE_RATE_LIMIT_PER_MINUTE set to 0");
                None
            }
        }
        Err(_) => {
            info!("Rate limiting disabled: IMGFORGE_RATE_LIMIT_PER_MINUTE not set");
            None
        }
    };

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_env(ENV_LOG_LEVEL))
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting imgforge server with {} workers...", config.workers);

    // Initialize libvips once for the whole process
    let vips_app = Arc::new(init_vips());

    let state = Arc::new(AppState {
        semaphore,
        cache,
        rate_limiter,
        config,
        vips_app,
    });

    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
    monitoring::register_metrics(prometheus::default_registry());

    let main_metric_handle = metric_handle.clone();
    let main_state = state.clone();

    let app = Router::new()
        .route("/status", get(status_handler))
        .route("/info/{*path}", get(info_handler))
        .route(
            "/{*path}",
            get(image_forge_handler)
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::rate_limit_middleware,
                ))
                .layer(axum::middleware::from_fn(middleware::status_code_metric_middleware)),
        )
        .route(
            "/metrics",
            get(move || async move {
                monitoring::update_vips_metrics(&main_state.vips_app);
                main_metric_handle.render()
            }),
        )
        .with_state(state.clone())
        .layer(prometheus_layer)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<axum::body::Body>| {
                let request_id = generate_request_id();
                info_span!(
                    "request",
                    id = %request_id,
                    method = %request.method(),
                    uri = %request.uri(),
                )
            }),
        )
        .layer(TimeoutLayer::new(Duration::from_secs(state.config.timeout)));
    let listener = TcpListener::bind(&state.config.bind_address).await.unwrap();
    info!("Listening on http://{}", &state.config.bind_address);

    let main_server = axum::serve(listener, app);

    // Conditionally start Prometheus server
    if let Some(prometheus_bind_address) = &state.config.prometheus_bind_address {
        match TcpListener::bind(prometheus_bind_address).await {
            Ok(prometheus_listener) => {
                info!(
                    "Prometheus metrics will be exposed on http://{}",
                    prometheus_bind_address
                );

                let prometheus_state = state.clone();
                let prometheus_app = Router::new().route(
                    "/metrics",
                    get(move || async move {
                        monitoring::update_vips_metrics(&prometheus_state.vips_app);
                        metric_handle.render()
                    }),
                );

                let prometheus_server = axum::serve(prometheus_listener, prometheus_app);

                tokio::select! {
                    _ = main_server => {},
                    _ = prometheus_server => {},
                }
            }
            Err(e) => {
                warn!(
                    "Failed to bind Prometheus to {}: {}. Prometheus metrics will not be available.",
                    prometheus_bind_address, e
                );
                main_server.await.unwrap();
            }
        }
    } else {
        main_server.await.unwrap();
    }
}
