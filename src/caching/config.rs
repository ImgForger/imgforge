use crate::caching::error::CacheError;
use crate::constants::*;
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub enum CacheConfig {
    Memory {
        capacity: usize,
    },
    Disk {
        path: String,
        capacity: usize,
    },
    Hybrid {
        memory_capacity: usize,
        disk_path: String,
        disk_capacity: usize,
    },
}

impl CacheConfig {
    pub fn from_env() -> Result<Option<Self>, CacheError> {
        let cache_type = match env::var(CACHE_TYPE) {
            Ok(val) => val,
            Err(_) => return Ok(None),
        };

        match cache_type.to_lowercase().as_str() {
            "memory" => {
                let capacity = env::var(CACHE_MEMORY_CAPACITY)
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid memory capacity: {}", e)))?;
                Ok(Some(CacheConfig::Memory { capacity }))
            }
            "disk" => {
                let path = env::var(CACHE_DISK_PATH)
                    .map_err(|_| CacheError::InvalidConfiguration(format!("{} must be set", CACHE_DISK_PATH)))?;
                let capacity = env::var(CACHE_DISK_CAPACITY)
                    .unwrap_or_else(|_| "10000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid disk capacity: {}", e)))?;
                Ok(Some(CacheConfig::Disk { path, capacity }))
            }
            "hybrid" => {
                let dram_capacity = env::var(CACHE_MEMORY_CAPACITY)
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid hybrid DRAM capacity: {}", e)))?;
                let disk_path = env::var(CACHE_DISK_PATH)
                    .map_err(|_| CacheError::InvalidConfiguration(format!("{} must be set", CACHE_DISK_PATH)))?;
                let disk_capacity = env::var(CACHE_DISK_PATH)
                    .unwrap_or_else(|_| "10000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid hybrid disk capacity: {}", e)))?;
                Ok(Some(CacheConfig::Hybrid {
                    memory_capacity: dram_capacity,
                    disk_path,
                    disk_capacity,
                }))
            }
            _ => Err(CacheError::InvalidConfiguration("Invalid CACHE_TYPE".to_string())),
        }
    }
}