use crate::caching::config::CacheConfig;
use crate::caching::error::CacheError;
use crate::monitoring::{increment_cache_hit, increment_cache_miss};
use crate::utils::format_to_content_type;
use bytes::Bytes;
use foyer::{
    BlockEngineConfig, Cache, CacheBuilder, Code, Error as FoyerError, ErrorKind, FsDeviceBuilder, HybridCache,
    HybridCacheBuilder,
};
use foyer::{DeviceBuilder, RecoverMode};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

const DEFAULT_BLOCK_SIZE: usize = 16 * 1024 * 1024;
const MIN_BLOCK_SIZE: usize = 4 * 1024;

fn block_size_for_capacity(capacity: usize) -> usize {
    let target = capacity.min(DEFAULT_BLOCK_SIZE);
    let aligned = target - (target % MIN_BLOCK_SIZE);
    if aligned == 0 {
        MIN_BLOCK_SIZE
    } else {
        aligned
    }
}

#[derive(Clone)]
pub struct CachedImage {
    pub bytes: Bytes,
    pub content_type: &'static str,
}

impl Code for CachedImage {
    fn encode(&self, writer: &mut impl Write) -> Result<(), FoyerError> {
        let data = self.bytes.as_ref();
        data.len().encode(writer)?;
        writer.write_all(data).map_err(FoyerError::io_error)?;

        let content_type_bytes = self.content_type.as_bytes();
        content_type_bytes.len().encode(writer)?;
        writer.write_all(content_type_bytes).map_err(FoyerError::io_error)?;
        Ok(())
    }

    fn decode(reader: &mut impl Read) -> Result<Self, FoyerError> {
        let len = usize::decode(reader)?;
        let mut data = vec![0u8; len];
        reader.read_exact(&mut data).map_err(FoyerError::io_error)?;

        let content_len = usize::decode(reader)?;
        let mut content_buf = vec![0u8; content_len];
        reader.read_exact(&mut content_buf).map_err(FoyerError::io_error)?;
        let content_str = std::str::from_utf8(&content_buf)
            .map_err(|_| FoyerError::new(ErrorKind::Parse, "invalid utf8 in content type"))?;
        let content_type = format_to_content_type(content_str);

        Ok(CachedImage {
            bytes: Bytes::from(data),
            content_type,
        })
    }

    fn estimated_size(&self) -> usize {
        self.bytes.len() + self.content_type.len() + std::mem::size_of::<usize>() * 2
    }
}

#[derive(Clone)]
pub struct CachedMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
}

impl Code for CachedMetadata {
    fn encode(&self, writer: &mut impl Write) -> Result<(), FoyerError> {
        self.width.encode(writer)?;
        self.height.encode(writer)?;

        let format_bytes = self.format.as_bytes();
        format_bytes.len().encode(writer)?;
        writer.write_all(format_bytes).map_err(FoyerError::io_error)?;
        Ok(())
    }

    fn decode(reader: &mut impl Read) -> Result<Self, FoyerError> {
        let width = u32::decode(reader)?;
        let height = u32::decode(reader)?;

        let len = usize::decode(reader)?;
        let mut format_buf = vec![0u8; len];
        reader.read_exact(&mut format_buf).map_err(FoyerError::io_error)?;
        let format = std::str::from_utf8(&format_buf)
            .map_err(|_| FoyerError::new(ErrorKind::Parse, "invalid utf8 in cached format"))?
            .to_string();

        Ok(CachedMetadata { width, height, format })
    }

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<u32>() * 2 + self.format.len() + std::mem::size_of::<usize>()
    }
}

/// Represents the different cache backends for Imgforge.
pub enum ImgforgeCache {
    None,
    Memory(Arc<Cache<String, CachedImage>>),
    Disk(Arc<HybridCache<String, CachedImage>>),
    Hybrid(Arc<HybridCache<String, CachedImage>>),
}

/// Metadata cache for lightweight info requests.
pub enum MetadataCache {
    None,
    Memory(Arc<Cache<String, CachedMetadata>>),
    Disk(Arc<HybridCache<String, CachedMetadata>>),
    Hybrid(Arc<HybridCache<String, CachedMetadata>>),
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
                let block_size = block_size_for_capacity(capacity);
                info!(
                    cache = "image",
                    mode = "disk",
                    capacity,
                    block_size,
                    "Using disk cache block size"
                );
                let engine = BlockEngineConfig::new(device).with_block_size(block_size);
                let cache = HybridCacheBuilder::new()
                    .memory(0) // No memory, pure disk
                    .storage()
                    .with_engine_config(engine)
                    .with_recover_mode(RecoverMode::Quiet)
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
                let block_size = block_size_for_capacity(disk_capacity);
                info!(
                    cache = "image",
                    mode = "hybrid",
                    capacity = disk_capacity,
                    block_size,
                    "Using disk cache block size"
                );
                let engine = BlockEngineConfig::new(device).with_block_size(block_size);
                let cache = HybridCacheBuilder::new()
                    .memory(memory_capacity)
                    .storage()
                    .with_engine_config(engine)
                    .with_recover_mode(RecoverMode::Quiet)
                    .build()
                    .await
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                Ok(ImgforgeCache::Hybrid(Arc::new(cache)))
            }
        }
    }

    /// Retrieve a value from the cache by key.
    pub async fn get(&self, key: &str) -> Option<CachedImage> {
        let result = match self {
            ImgforgeCache::None => None,
            ImgforgeCache::Memory(cache) => {
                let res = cache.get(key).map(|e| e.value().clone());
                record_cache_metric(res.is_some(), "memory");
                res
            }
            ImgforgeCache::Disk(cache) => {
                let res = cache
                    .get(&key.to_string())
                    .await
                    .ok()
                    .flatten()
                    .map(|e| e.value().clone());
                record_cache_metric(res.is_some(), "disk");
                res
            }
            ImgforgeCache::Hybrid(cache) => {
                let res = cache
                    .get(&key.to_string())
                    .await
                    .ok()
                    .flatten()
                    .map(|e| e.value().clone());
                record_cache_metric(res.is_some(), "hybrid");
                res
            }
        };
        result
    }

    /// Insert a value into the cache.
    pub async fn insert(&self, key: String, value: CachedImage) -> Result<(), CacheError> {
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

impl MetadataCache {
    /// Create a new metadata cache instance based on the provided configuration.
    pub async fn new(config: Option<CacheConfig>) -> Result<Self, CacheError> {
        match config {
            None => Ok(MetadataCache::None),
            Some(CacheConfig::Memory { capacity, .. }) => {
                let cache = CacheBuilder::new(capacity).build();
                Ok(MetadataCache::Memory(Arc::new(cache)))
            }
            Some(CacheConfig::Disk { path, capacity, .. }) => {
                let device = FsDeviceBuilder::new(Path::new(&path))
                    .with_capacity(capacity)
                    .build()
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                let block_size = block_size_for_capacity(capacity);
                info!(
                    cache = "metadata",
                    mode = "disk",
                    capacity,
                    block_size,
                    "Using disk cache block size"
                );
                let engine = BlockEngineConfig::new(device).with_block_size(block_size);
                let cache = HybridCacheBuilder::new()
                    .memory(0)
                    .storage()
                    .with_engine_config(engine)
                    .with_recover_mode(RecoverMode::Quiet)
                    .build()
                    .await
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                Ok(MetadataCache::Disk(Arc::new(cache)))
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
                let block_size = block_size_for_capacity(disk_capacity);
                info!(
                    cache = "metadata",
                    mode = "hybrid",
                    capacity = disk_capacity,
                    block_size,
                    "Using disk cache block size"
                );
                let engine = BlockEngineConfig::new(device).with_block_size(block_size);
                let cache = HybridCacheBuilder::new()
                    .memory(memory_capacity)
                    .storage()
                    .with_engine_config(engine)
                    .with_recover_mode(RecoverMode::Quiet)
                    .build()
                    .await
                    .map_err(|e| CacheError::Initialization(e.to_string()))?;
                Ok(MetadataCache::Hybrid(Arc::new(cache)))
            }
        }
    }

    /// Retrieve metadata from the cache by key.
    pub async fn get(&self, key: &str) -> Option<CachedMetadata> {
        let result = match self {
            MetadataCache::None => None,
            MetadataCache::Memory(cache) => {
                let res = cache.get(key).map(|e| e.value().clone());
                record_cache_metric(res.is_some(), "metadata-memory");
                res
            }
            MetadataCache::Disk(cache) => {
                let res = cache
                    .get(&key.to_string())
                    .await
                    .ok()
                    .flatten()
                    .map(|e| e.value().clone());
                record_cache_metric(res.is_some(), "metadata-disk");
                res
            }
            MetadataCache::Hybrid(cache) => {
                let res = cache
                    .get(&key.to_string())
                    .await
                    .ok()
                    .flatten()
                    .map(|e| e.value().clone());
                record_cache_metric(res.is_some(), "metadata-hybrid");
                res
            }
        };
        result
    }

    /// Insert metadata into the cache.
    pub async fn insert(&self, key: String, value: CachedMetadata) -> Result<(), CacheError> {
        match self {
            MetadataCache::None => Ok(()),
            MetadataCache::Memory(cache) => {
                cache.insert(key, value);
                Ok(())
            }
            MetadataCache::Disk(cache) | MetadataCache::Hybrid(cache) => {
                cache.insert(key, value);
                Ok(())
            }
        }
    }
}

fn record_cache_metric(hit: bool, cache_type: &str) {
    if hit {
        increment_cache_hit(cache_type);
    } else {
        increment_cache_miss(cache_type);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caching::config::CacheConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_new_none_cache() {
        let cache = ImgforgeCache::new(None).await.unwrap();
        assert!(matches!(cache, ImgforgeCache::None));
    }

    #[tokio::test]
    async fn test_new_memory_cache() {
        let config = Some(CacheConfig::Memory { capacity: 1000 });
        let cache = ImgforgeCache::new(config).await.unwrap();
        assert!(matches!(cache, ImgforgeCache::Memory(_)));
    }

    #[tokio::test]
    async fn test_new_disk_cache() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let config = Some(CacheConfig::Disk { path, capacity: 10000 });
        let cache = ImgforgeCache::new(config).await.unwrap();
        assert!(matches!(cache, ImgforgeCache::Disk(_)));
    }

    #[tokio::test]
    async fn test_new_hybrid_cache() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let config = Some(CacheConfig::Hybrid {
            memory_capacity: 1000,
            disk_path: path,
            disk_capacity: 10000,
        });
        let cache = ImgforgeCache::new(config).await.unwrap();
        assert!(matches!(cache, ImgforgeCache::Hybrid(_)));
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let config = Some(CacheConfig::Memory { capacity: 1000 });
        let cache = ImgforgeCache::new(config).await.unwrap();

        let key = "test_key".to_string();
        let value = CachedImage {
            bytes: Bytes::from(vec![1, 2, 3]),
            content_type: "image/jpeg",
        };

        cache.insert(key.clone(), value.clone()).await.unwrap();
        let retrieved = cache.get(&key).await.unwrap();
        assert_eq!(retrieved.bytes, value.bytes);
        assert_eq!(retrieved.content_type, value.content_type);
    }

    #[tokio::test]
    async fn test_cache_operations_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let config = Some(CacheConfig::Disk { path, capacity: 10000 });
        let cache = ImgforgeCache::new(config).await.unwrap();
        let key = "test_key".to_string();
        let value = CachedImage {
            bytes: Bytes::from(vec![1, 2, 3]),
            content_type: "image/jpeg",
        };
        cache.insert(key.clone(), value.clone()).await.unwrap();
        let retrieved = cache.get(&key).await.unwrap();
        assert_eq!(retrieved.bytes, value.bytes);
        assert_eq!(retrieved.content_type, value.content_type);
    }
}
