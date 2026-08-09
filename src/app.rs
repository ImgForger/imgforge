use crate::caching::cache::{ImgforgeCache as Cache, MetadataCache};
use crate::caching::config::CacheConfig;
use crate::caching::error::CacheError;
use crate::config::{Config, ConfigError};
use crate::monitoring;
use crate::processing::watermark::CachedWatermark;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use libvips::VipsApp;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{OnceCell, Semaphore};
use tracing::{info, warn};

pub type RequestRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Shared application state for imgforge.
pub struct AppState {
    pub semaphore: Arc<Semaphore>,
    pub cache: Cache,
    pub metadata_cache: MetadataCache,
    pub rate_limiter: Option<RequestRateLimiter>,
    pub config: Config,
    pub vips_app: Arc<VipsApp>,
    pub http_client: reqwest::Client,
    pub watermark_cache: OnceCell<CachedWatermark>,
}

#[derive(Clone)]
pub struct Imgforge {
    state: Arc<AppState>,
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("configuration error: {0}")]
    Configuration(#[from] ConfigError),
    #[error("failed to initialize libvips")]
    Libvips(#[source] libvips::error::Error),
    #[error("failed to build HTTP client: {0}")]
    HttpClient(#[from] reqwest::Error),
    #[error("failed to initialize cache: {0}")]
    Cache(#[from] CacheError),
}

impl Imgforge {
    /// Create a new imgforge instance from an explicit configuration.
    pub async fn new(config: Config, cache_config: Option<CacheConfig>) -> Result<Self, InitError> {
        validate_worker_count(config.workers)?;

        monitoring::register_metrics();
        monitoring::set_image_operation_concurrency_limit(config.workers);
        warn_if_worker_count_is_high(config.workers);

        let semaphore = Arc::new(Semaphore::new(config.workers));
        let cache = Cache::new(cache_config.clone()).await?;
        let metadata_cache = MetadataCache::new(cache_config).await?;
        let vips_app = Arc::new(init_vips()?);
        let http_client = build_http_client(config.download_timeout)?;
        let rate_limiter = build_rate_limiter(config.rate_limit_per_minute);
        let watermark_cache = OnceCell::new();

        let state = Arc::new(AppState {
            semaphore,
            cache,
            metadata_cache,
            rate_limiter,
            config,
            vips_app,
            http_client,
            watermark_cache,
        });

        Ok(Self { state })
    }

    /// Construct imgforge using environment-derived configuration.
    pub async fn from_env() -> Result<Self, InitError> {
        let config = Config::from_env()?;
        let cache_config = CacheConfig::from_env().map_err(InitError::Cache)?;
        Self::new(config, cache_config).await
    }

    /// Access the shared application state.
    pub fn state(&self) -> Arc<AppState> {
        self.state.clone()
    }

    /// Access the effective configuration.
    pub fn config(&self) -> &Config {
        &self.state.config
    }

    /// Process an imgproxy-compatible path using the configured state.
    pub async fn process_path(
        &self,
        path: &str,
    ) -> Result<crate::service::ProcessedImage, crate::service::ServiceError> {
        self.process_path_with_token(path, None).await
    }

    /// Process an imgproxy-compatible path with an optional bearer token.
    pub async fn process_path_with_token(
        &self,
        path: &str,
        bearer_token: Option<&str>,
    ) -> Result<crate::service::ProcessedImage, crate::service::ServiceError> {
        let request = crate::service::ProcessRequest { path, bearer_token };
        crate::service::process_path(self.state.clone(), request).await
    }

    /// Retrieve source image metadata for an imgproxy-compatible path.
    pub async fn image_info(&self, path: &str) -> Result<crate::service::ImageInfo, crate::service::ServiceError> {
        self.image_info_with_token(path, None).await
    }

    /// Retrieve source image metadata with an optional bearer token.
    pub async fn image_info_with_token(
        &self,
        path: &str,
        bearer_token: Option<&str>,
    ) -> Result<crate::service::ImageInfo, crate::service::ServiceError> {
        let request = crate::service::ProcessRequest { path, bearer_token };
        crate::service::image_info(self.state.clone(), request).await
    }
}

fn warn_if_worker_count_is_high(workers: usize) {
    let cpu_count = num_cpus::get().max(1);
    let automatic_workers = cpu_count.saturating_mul(2);
    if workers > automatic_workers {
        warn!(
            workers,
            cpu_count,
            automatic_workers,
            "Configured image-processing concurrency is high relative to CPU count; verify memory headroom under representative load"
        );
    }
}

fn validate_worker_count(workers: usize) -> Result<(), ConfigError> {
    if workers == 0 {
        return Err(ConfigError::ZeroWorkers);
    }
    if workers > Semaphore::MAX_PERMITS {
        return Err(ConfigError::WorkerCountTooLarge {
            value: workers,
            max: Semaphore::MAX_PERMITS,
        });
    }
    Ok(())
}

fn init_vips() -> Result<VipsApp, InitError> {
    VipsApp::new("imgforge", false).map_err(InitError::Libvips)
}

fn build_http_client(timeout_secs: u64) -> Result<reqwest::Client, reqwest::Error> {
    let timeout = Duration::from_secs(timeout_secs);
    reqwest::Client::builder().timeout(timeout).build()
}

fn build_rate_limiter(limit_per_minute: Option<u32>) -> Option<RequestRateLimiter> {
    match limit_per_minute {
        Some(limit) if limit > 0 => {
            if let Some(non_zero) = NonZeroU32::new(limit) {
                info!("Rate limiting enabled: {} requests per minute", limit);
                Some(RateLimiter::direct(Quota::per_minute(non_zero)))
            } else {
                warn!("Rate limiting disabled due to zero limit");
                None
            }
        }
        Some(_) => {
            info!("Rate limiting disabled: limit configured as 0");
            None
        }
        None => {
            info!("Rate limiting disabled: not configured");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_count_must_be_positive() {
        assert!(matches!(validate_worker_count(0), Err(ConfigError::ZeroWorkers)));
        assert!(validate_worker_count(1).is_ok());
    }

    #[test]
    fn worker_count_must_fit_tokio_semaphore() {
        assert!(validate_worker_count(Semaphore::MAX_PERMITS).is_ok());
        assert!(matches!(
            validate_worker_count(Semaphore::MAX_PERMITS.saturating_add(1)),
            Err(ConfigError::WorkerCountTooLarge { .. })
        ));
    }
}
