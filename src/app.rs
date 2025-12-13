use crate::caching::cache::{ImgforgeCache as Cache, MetadataCache};
use crate::caching::config::CacheConfig;
use crate::caching::error::CacheError;
use crate::config::Config;
use crate::monitoring;
use crate::processing::watermark::CachedWatermark;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use rs_vips::{Vips, VipsImage};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, Semaphore};
use tracing::{info, warn};

pub type RequestRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Shared application state for imgforge.
pub struct AppState {
    pub semaphore: Arc<Semaphore>,
    pub cache: Cache,
    pub metadata_cache: MetadataCache,
    pub rate_limiter: Option<RequestRateLimiter>,
    pub config: Config,
    pub http_client: reqwest::Client,
    pub watermark_cache: Mutex<Option<CachedWatermark>>,
}

#[derive(Clone)]
pub struct Imgforge {
    state: Arc<AppState>,
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("failed to initialize libvips: {0}")]
    Libvips(String),
    #[error("failed to build HTTP client: {0}")]
    HttpClient(#[from] reqwest::Error),
    #[error("failed to initialize cache: {0}")]
    Cache(#[from] CacheError),
}

impl Imgforge {
    /// Create a new imgforge instance from an explicit configuration.
    pub async fn new(config: Config, cache_config: Option<CacheConfig>) -> Result<Self, InitError> {
        monitoring::register_metrics();

        let semaphore = Arc::new(Semaphore::new(config.workers));
        let cache = Cache::new(cache_config.clone()).await?;
        let metadata_cache = MetadataCache::new(cache_config).await?;
        init_vips()?;
        let http_client = build_http_client(config.download_timeout)?;
        let rate_limiter = build_rate_limiter(config.rate_limit_per_minute);
        let watermark_cache = Mutex::new(None);

        let state = Arc::new(AppState {
            semaphore,
            cache,
            metadata_cache,
            rate_limiter,
            config,
            http_client,
            watermark_cache,
        });

        Ok(Self { state })
    }

    /// Construct imgforge using environment-derived configuration.
    pub async fn from_env() -> Result<Self, InitError> {
        let config = Config::from_env().map_err(InitError::Configuration)?;
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

fn init_vips() -> Result<(), InitError> {
    Vips::init("imgforge").map_err(|err| InitError::Libvips(err.to_string()))
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
