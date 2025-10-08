use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Cache initialization error: {0}")]
    Initialization(String),

    #[error("Invalid cache configuration: {0}")]
    InvalidConfiguration(String),
}
