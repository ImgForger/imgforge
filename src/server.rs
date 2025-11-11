use crate::app::Imgforge;
use crate::caching::config::CacheConfig;
use crate::config::Config;
use crate::constants::*;
use crate::handlers::{image_forge_handler, info_handler, status_handler};
use crate::middleware;
use crate::monitoring;
use axum::{extract::Request, routing::get, Router};
use axum_prometheus::PrometheusMetricLayer;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, info_span, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub async fn start() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_env(ENV_LOG_LEVEL))
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let config = Config::from_env().expect("Failed to load config");
    let cache_config = CacheConfig::from_env().expect("Failed to load cache config");

    let imgforge = Imgforge::new(config, cache_config)
        .await
        .expect("Failed to initialize imgforge");
    let state = imgforge.state();

    info!("Starting imgforge server with {} workers...", state.config.workers);

    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
    monitoring::register_metrics();

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
                let request_id = request
                    .extensions()
                    .get::<middleware::RequestId>()
                    .map(|id| id.0.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                info_span!(
                    "request",
                    id = %request_id,
                    method = %request.method(),
                    uri = %request.uri(),
                )
            }),
        )
        .layer(axum::middleware::from_fn(middleware::request_id_middleware))
        .layer(TimeoutLayer::new(Duration::from_secs(state.config.timeout)));
    let listener = TcpListener::bind(&state.config.bind_address).await.unwrap();
    info!("Listening on http://{}", &state.config.bind_address);

    let main_server = axum::serve(listener, app);

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
