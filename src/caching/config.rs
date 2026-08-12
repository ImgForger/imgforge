use crate::caching::error::CacheError;
use crate::constants::*;
use serde::Deserialize;
use std::env::{self, VarError};

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
    pub fn startup_log_message(config: Option<&Self>) -> String {
        match config {
            None => "Caching disabled".to_string(),
            Some(Self::Memory { capacity }) => format!("Caching enabled: memory (capacity={capacity})"),
            Some(Self::Disk { path, capacity }) => {
                format!("Caching enabled: disk (path={path}, capacity={capacity})")
            }
            Some(Self::Hybrid {
                memory_capacity,
                disk_path,
                disk_capacity,
            }) => format!(
                "Caching enabled: hybrid (memory_capacity={memory_capacity}, disk_path={disk_path}, disk_capacity={disk_capacity})"
            ),
        }
    }

    pub fn from_env() -> Result<Option<Self>, CacheError> {
        let cache_type = match env::var(ENV_CACHE_TYPE) {
            Ok(val) => val,
            Err(VarError::NotPresent) => return Ok(None),
            Err(source) => {
                return Err(CacheError::Environment {
                    name: ENV_CACHE_TYPE,
                    source,
                });
            }
        };

        match cache_type.to_lowercase().as_str() {
            "memory" => {
                let capacity = capacity_from_env(ENV_CACHE_MEMORY_CAPACITY, "1000")?;
                Ok(Some(CacheConfig::Memory { capacity }))
            }
            "disk" => {
                let path = required_env(ENV_CACHE_DISK_PATH)?;
                let capacity = capacity_from_env(ENV_CACHE_DISK_CAPACITY, "10000")?;
                Ok(Some(CacheConfig::Disk { path, capacity }))
            }
            "hybrid" => {
                let memory_capacity = capacity_from_env(ENV_CACHE_MEMORY_CAPACITY, "1000")?;
                let disk_path = required_env(ENV_CACHE_DISK_PATH)?;
                let disk_capacity = capacity_from_env(ENV_CACHE_DISK_CAPACITY, "10000")?;
                Ok(Some(CacheConfig::Hybrid {
                    memory_capacity,
                    disk_path,
                    disk_capacity,
                }))
            }
            _ => Err(CacheError::UnsupportedType { value: cache_type }),
        }
    }
}

fn required_env(name: &'static str) -> Result<String, CacheError> {
    env::var(name).map_err(|source| CacheError::Environment { name, source })
}

fn capacity_from_env(name: &'static str, default: &str) -> Result<usize, CacheError> {
    let value = match env::var(name) {
        Ok(value) => value,
        Err(VarError::NotPresent) => default.to_owned(),
        Err(source) => return Err(CacheError::Environment { name, source }),
    };

    value
        .parse()
        .map_err(|source| CacheError::InvalidCapacity { name, value, source })
}

#[cfg(test)]
mod tests {
    use super::CacheConfig;

    #[test]
    fn startup_log_message_describes_memory_cache() {
        let config = CacheConfig::Memory { capacity: 1000 };
        assert_eq!(
            CacheConfig::startup_log_message(Some(&config)),
            "Caching enabled: memory (capacity=1000)"
        );
    }

    #[test]
    fn startup_log_message_describes_disabled_cache() {
        assert_eq!(CacheConfig::startup_log_message(None), "Caching disabled");
    }
}
