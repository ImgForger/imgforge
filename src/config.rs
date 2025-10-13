use crate::constants::*;
use std::env;

pub struct Config {
    pub workers: usize,
    pub bind_address: String,
    pub prometheus_bind_address: Option<String>,
    pub timeout: u64,
    pub key: Vec<u8>,
    pub salt: Vec<u8>,
    pub allow_unsigned: bool,
    pub allow_security_options: bool,
    pub max_src_file_size: Option<usize>,
    pub max_src_resolution: Option<f32>,
    pub allowed_mime_types: Option<Vec<String>>,
    pub download_timeout: u64,
    pub secret: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let workers = env::var(ENV_WORKERS)
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .unwrap_or(0);
        let workers = if workers == 0 { num_cpus::get() * 2 } else { workers };

        let bind_address = env::var(ENV_BIND).unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let prometheus_bind_address = env::var(ENV_PROMETHEUS_BIND).ok();
        let timeout = env::var(ENV_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);

        let key_str = env::var(ENV_KEY).unwrap_or_default();
        let salt_str = env::var(ENV_SALT).unwrap_or_default();
        let key = hex::decode(key_str).map_err(|_| "Invalid IMGFORGE_KEY")?;
        let salt = hex::decode(salt_str).map_err(|_| "Invalid IMGFORGE_SALT")?;

        let allow_unsigned = env::var(ENV_ALLOW_UNSIGNED).unwrap_or_default().to_lowercase() == "true";
        let allow_security_options = env::var(ENV_ALLOW_SECURITY_OPTIONS).unwrap_or_default().to_lowercase() == "true";

        let max_src_file_size = env::var(ENV_MAX_SRC_FILE_SIZE).ok().and_then(|s| s.parse().ok());
        let max_src_resolution = env::var(ENV_MAX_SRC_RESOLUTION).ok().and_then(|s| s.parse().ok());
        let allowed_mime_types = env::var(ENV_ALLOWED_MIME_TYPES)
            .ok()
            .map(|s| s.split(',').map(|s| s.to_string()).collect());
        let download_timeout = env::var(ENV_DOWNLOAD_TIMEOUT)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10);
        let secret = env::var(ENV_SECRET).ok();

        Ok(Self {
            workers,
            bind_address,
            prometheus_bind_address,
            timeout,
            key,
            salt,
            allow_unsigned,
            allow_security_options,
            max_src_file_size,
            max_src_resolution,
            allowed_mime_types,
            download_timeout,
            secret,
        })
    }
}
