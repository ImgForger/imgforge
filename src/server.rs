use crate::app::{Imgforge, InitError};
use crate::caching::config::CacheConfig;
use crate::caching::error::CacheError;
use crate::config::{Config, ConfigError};
use crate::constants::*;
use crate::handlers::{image_forge_handler, info_handler, status_handler};
use crate::middleware;
use crate::monitoring;
use axum::http::StatusCode;
use axum::{extract::Request, routing::get, Router};
use axum_prometheus::PrometheusMetricLayer;
use std::time::Duration;
use thiserror::Error;
use tokio::net::TcpListener;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, info_span, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// Fixed routes imgforge registers itself, which the health path cannot take
/// over.
///
/// `/status` is deliberately absent: it serves the health handler already, so
/// pointing the health path at it is a synonym rather than a conflict, and the
/// router simply skips the second registration.
const RESERVED_ROUTES: &[&str] = &["/metrics", "/info"];

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to install tracing subscriber: {0}")]
    Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("failed to load configuration: {0}")]
    Configuration(#[from] ConfigError),
    #[error("failed to load cache configuration: {0}")]
    CacheConfiguration(#[from] CacheError),
    #[error("failed to initialize imgforge: {0}")]
    Initialization(#[from] InitError),
    #[error("failed to bind main server to {address}: {source}")]
    Bind {
        address: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{name} server failed: {source}")]
    Serve {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("IMGFORGE_HEALTH_CHECK_PATH is {path}, which is already served by imgforge itself")]
    ReservedHealthCheckPath { path: String },
}

/// Configure and run the imgforge HTTP servers.
///
/// # Errors
///
/// Returns an error when tracing or application initialization fails, the main
/// listener cannot bind, or a running server encounters an I/O failure.
pub async fn start() -> Result<(), ServerError> {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_env(ENV_LOG_LEVEL))
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = Config::from_env()?;
    let cache_config = CacheConfig::from_env()?;
    info!("{}", CacheConfig::startup_log_message(cache_config.as_ref()));

    let imgforge = Imgforge::new(config, cache_config).await?;
    let state = imgforge.state();

    info!("Starting imgforge server with {} workers...", state.config.workers);

    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
    monitoring::register_metrics();

    let main_metric_handle = metric_handle.clone();
    let main_state = state.clone();

    let prefix = state.config.path_prefix.clone();
    let health_path = state.config.health_check_path.clone();
    let route = |path: &str| format!("{prefix}{path}");

    // Registering two GET handlers on one path makes axum panic while the
    // router is being built, so a health path that lands on a route imgforge
    // already owns takes the whole server down with a message about routing
    // rather than about configuration. Refusing it here says what is actually
    // wrong. `/status` is exempt: it is the same handler under its other name,
    // so it is a synonym rather than a collision.
    if RESERVED_ROUTES.contains(&health_path.as_str()) {
        return Err(ServerError::ReservedHealthCheckPath { path: health_path });
    }

    // `/status` is imgforge's own name for the liveness endpoint and `/health`
    // is imgproxy's, so a deployment can be moved either way without touching
    // its orchestration; IMGFORGE_HEALTH_CHECK_PATH renames the latter.
    let mut app = Router::new()
        .route(&route("/status"), get(status_handler))
        .route(&route("/info/{*path}"), get(info_handler));

    if health_path != "/status" {
        app = app.route(&route(&health_path), get(status_handler));
    }

    let app = app
        .route(
            &route("/{*path}"),
            get(image_forge_handler)
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::rate_limit_middleware,
                ))
                .layer(axum::middleware::from_fn(middleware::status_code_metric_middleware)),
        )
        .route(
            &route("/metrics"),
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
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(state.config.timeout),
        ));
    let bind_address = state.config.bind_address.clone();
    let listener = TcpListener::bind(&bind_address)
        .await
        .map_err(|source| ServerError::Bind {
            address: bind_address.clone(),
            source,
        })?;
    info!("Listening on http://{}", bind_address);

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

                let main_server = async {
                    main_server
                        .await
                        .map_err(|source| ServerError::Serve { name: "main", source })
                };
                let prometheus_server = async {
                    prometheus_server.await.map_err(|source| ServerError::Serve {
                        name: "prometheus",
                        source,
                    })
                };

                tokio::try_join!(main_server, prometheus_server)?;
            }
            Err(e) => {
                warn!(
                    "Failed to bind Prometheus to {}: {}. Prometheus metrics will not be available.",
                    prometheus_bind_address, e
                );
                main_server
                    .await
                    .map_err(|source| ServerError::Serve { name: "main", source })?;
            }
        }
    } else {
        main_server
            .await
            .map_err(|source| ServerError::Serve { name: "main", source })?;
    }

    Ok(())
}
