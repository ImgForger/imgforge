use crate::caching::config::CacheConfig;
use crate::caching::error::CacheError;
use foyer::DeviceBuilder;
use foyer::{BlockEngineBuilder, Cache, CacheBuilder, FsDeviceBuilder, HybridCache, HybridCacheBuilder};
use std::path::Path;
use std::sync::Arc;

/// Represents the different cache backends for Imgforge.
pub enum ImgforgeCache {
    None,
    Memory(Arc<Cache<String, Vec<u8>>>),
    Disk(Arc<HybridCache<String, Vec<u8>>>),
    Hybrid(Arc<HybridCache<String, Vec<u8>>>),
}

impl ImgforgeCache {
    /// Create a new cache instance based on the provided configuration.
    pub async fn new(config: Option<CacheConfig>) -> Result<Self, CacheError> {
        match config {
            None => Ok(ImgforgeCache::None),
            Some(CacheConfig::Memory { capacity, .. }) => {
                let cache = CacheBuilder::new(capacity).build();
                Ok(ImgforgeCache::Memory(Arc::new(cache)))
            }
            Some(CacheConfig::Disk { path, capacity, .. }) => {
                let device = FsDeviceBuilder::new(Path::new(&path))
                    .with_capacity(capacity)
                    .build()
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                let engine = BlockEngineBuilder::new(device);
                let cache = HybridCacheBuilder::new()
                    .memory(0) // No memory, pure disk
                    .storage()
                    .with_engine_config(engine)
                    .build()
                    .await
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                Ok(ImgforgeCache::Disk(Arc::new(cache)))
            }
            Some(CacheConfig::Hybrid {
                memory_capacity,
                disk_path,
                disk_capacity,
                ..
            }) => {
                let device = FsDeviceBuilder::new(Path::new(&disk_path))
                    .with_capacity(disk_capacity)
                    .build()
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                let engine = BlockEngineBuilder::new(device);
                let cache = HybridCacheBuilder::new()
                    .memory(memory_capacity)
                    .storage()
                    .with_engine_config(engine)
                    .build()
                    .await
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                Ok(ImgforgeCache::Hybrid(Arc::new(cache)))
            }
        }
    }

    /// Retrieve a value from the cache by key.
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        match self {
            ImgforgeCache::None => None,
            ImgforgeCache::Memory(cache) => cache.get(key).map(|e| e.value().clone()),
            ImgforgeCache::Disk(cache) | ImgforgeCache::Hybrid(cache) => cache
                .get(&key.to_string())
                .await
                .ok()
                .flatten()
                .map(|e| e.value().clone()),
        }
    }

    /// Insert a value into the cache.
    pub async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), CacheError> {
        match self {
            ImgforgeCache::None => Ok(()),
            ImgforgeCache::Memory(cache) => {
                cache.insert(key, value);
                Ok(())
            }
            ImgforgeCache::Disk(cache) | ImgforgeCache::Hybrid(cache) => {
                cache.insert(key, value);
                Ok(())
            }
        }
    }
}
