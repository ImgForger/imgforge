use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Cache is not configured")]
    NotConfigured,

    #[error("Failed to insert value into cache: {0}")]
    Insert(String),

    #[error("Failed to retrieve value from cache: {0}")]
    Retrieve(String),

    #[error("Invalid cache configuration: {0}")]
    InvalidConfiguration(String),
}
