use crate::caching::error::CacheError;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub enum CacheConfig {
    Memory {
        capacity: usize,
        ttl: Option<u64>,
    },
    Disk {
        path: String,
        capacity: usize,
        ttl: Option<u64>,
    },
    Hybrid {
        dram_capacity: usize,
        dram_ttl: Option<u64>,
        disk_path: String,
        disk_capacity: usize,
        disk_ttl: Option<u64>,
    },
}

impl CacheConfig {
    pub fn from_env() -> Result<Option<Self>, CacheError> {
        let cache_type = match std::env::var("CACHE_TYPE") {
            Ok(val) => val,
            Err(_) => return Ok(None),
        };

        match cache_type.to_lowercase().as_str() {
            "memory" => {
                let capacity = std::env::var("CACHE_MEMORY_CAPACITY")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid memory capacity: {}", e)))?;
                let ttl = std::env::var("CACHE_MEMORY_TTL").ok().and_then(|v| v.parse().ok());
                Ok(Some(CacheConfig::Memory { capacity, ttl }))
            }
            "disk" => {
                let path = std::env::var("CACHE_DISK_PATH")
                    .map_err(|_| CacheError::InvalidConfiguration("CACHE_DISK_PATH must be set".to_string()))?;
                let capacity = std::env::var("CACHE_DISK_CAPACITY")
                    .unwrap_or_else(|_| "10000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid disk capacity: {}", e)))?;
                let ttl = std::env::var("CACHE_DISK_TTL").ok().and_then(|v| v.parse().ok());
                Ok(Some(CacheConfig::Disk { path, capacity, ttl }))
            }
            "hybrid" => {
                let dram_capacity = std::env::var("CACHE_HYBRID_DRAM_CAPACITY")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid hybrid DRAM capacity: {}", e)))?;
                let dram_ttl = std::env::var("CACHE_HYBRID_DRAM_TTL").ok().and_then(|v| v.parse().ok());
                let disk_path = std::env::var("CACHE_HYBRID_DISK_PATH")
                    .map_err(|_| CacheError::InvalidConfiguration("CACHE_HYBRID_DISK_PATH must be set".to_string()))?;
                let disk_capacity = std::env::var("CACHE_HYBRID_DISK_CAPACITY")
                    .unwrap_or_else(|_| "10000".to_string())
                    .parse()
                    .map_err(|e| CacheError::InvalidConfiguration(format!("Invalid hybrid disk capacity: {}", e)))?;
                let disk_ttl = std::env::var("CACHE_HYBRID_DISK_TTL").ok().and_then(|v| v.parse().ok());
                Ok(Some(CacheConfig::Hybrid {
                    dram_capacity,
                    dram_ttl,
                    disk_path,
                    disk_capacity,
                    disk_ttl,
                }))
            }
            _ => Err(CacheError::InvalidConfiguration("Invalid CACHE_TYPE".to_string())),
        }
    }
}
