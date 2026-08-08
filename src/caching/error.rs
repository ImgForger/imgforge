use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CacheError {
    #[error("failed to initialize cache device")]
    DeviceInitialization {
        #[source]
        source: foyer::Error,
    },

    #[error("failed to initialize cache storage")]
    StorageInitialization {
        #[source]
        source: foyer::Error,
    },

    #[error("failed to read {name}")]
    Environment {
        name: &'static str,
        #[source]
        source: std::env::VarError,
    },

    #[error("invalid value for {name} ({value:?})")]
    InvalidCapacity {
        name: &'static str,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },

    #[error("unsupported cache type {value:?}")]
    UnsupportedType { value: String },
}
