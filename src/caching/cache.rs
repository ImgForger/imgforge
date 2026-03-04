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
// Foyer's hybrid builder always constructs a memory phase before storage. In disk mode,
// keeping that tier to a single entry on one shard avoids startup warnings while leaving
// the in-memory footprint effectively negligible.
const DISK_MODE_MEMORY_CAPACITY: usize = 1;
const DISK_MODE_MEMORY_SHARDS: usize = 1;

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
    pub content_type: String,
    pub size_bytes: usize,
    pub channels: u32,
    pub has_alpha: bool,
    pub orientation: u32,
}

impl Code for CachedMetadata {
    fn encode(&self, writer: &mut impl Write) -> Result<(), FoyerError> {
        self.width.encode(writer)?;
        self.height.encode(writer)?;

        let format_bytes = self.format.as_bytes();
        format_bytes.len().encode(writer)?;
        writer.write_all(format_bytes).map_err(FoyerError::io_error)?;

        let content_type_bytes = self.content_type.as_bytes();
        content_type_bytes.len().encode(writer)?;
        writer.write_all(content_type_bytes).map_err(FoyerError::io_error)?;

        self.size_bytes.encode(writer)?;
        self.channels.encode(writer)?;
        self.has_alpha.encode(writer)?;
        self.orientation.encode(writer)?;
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

        let content_type_len = usize::decode(reader)?;
        let mut content_type_buf = vec![0u8; content_type_len];
        reader.read_exact(&mut content_type_buf).map_err(FoyerError::io_error)?;
        let content_type = std::str::from_utf8(&content_type_buf)
            .map_err(|_| FoyerError::new(ErrorKind::Parse, "invalid utf8 in cached content type"))?
            .to_string();

        let size_bytes = usize::decode(reader)?;
        let channels = u32::decode(reader)?;
        let has_alpha = bool::decode(reader)?;
        let orientation = u32::decode(reader)?;

        Ok(CachedMetadata {
            width,
            height,
            format,
            content_type,
            size_bytes,
            channels,
            has_alpha,
            orientation,
        })
    }

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<u32>() * 4
            + std::mem::size_of::<usize>() * 2
            + std::mem::size_of::<bool>()
            + self.format.len()
            + self.content_type.len()
    }
}

/// Represents the different cache backends for imgforge value types.
pub enum TypedCache<T>
where
    T: Clone + Code + Send + Sync + 'static,
{
    None,
    Memory(Arc<Cache<String, T>>),
    Disk(Arc<HybridCache<String, T>>),
    Hybrid(Arc<HybridCache<String, T>>),
}

pub type ImgforgeCache = TypedCache<CachedImage>;

/// Metadata cache for lightweight info requests.
pub type MetadataCache = TypedCache<CachedMetadata>;

impl<T> TypedCache<T>
where
    T: Clone + Code + Send + Sync + 'static,
{
    async fn get_with_metric_labels(
        &self,
        key: &str,
        memory_label: &str,
        disk_label: &str,
        hybrid_label: &str,
    ) -> Option<T> {
        match self {
            Self::None => None,
            Self::Memory(cache) => {
                let res = cache.get(key).map(|e| e.value().clone());
                record_cache_metric(res.is_some(), memory_label);
                res
            }
            Self::Disk(cache) => {
                let res = cache
                    .get(&key.to_string())
                    .await
                    .ok()
                    .flatten()
                    .map(|e| e.value().clone());
                record_cache_metric(res.is_some(), disk_label);
                res
            }
            Self::Hybrid(cache) => {
                let res = cache
                    .get(&key.to_string())
                    .await
                    .ok()
                    .flatten()
                    .map(|e| e.value().clone());
                record_cache_metric(res.is_some(), hybrid_label);
                res
            }
        }
    }

    async fn insert_value(&self, key: String, value: T) -> Result<(), CacheError> {
        match self {
            Self::None => Ok(()),
            Self::Memory(cache) => {
                cache.insert(key, value);
                Ok(())
            }
            Self::Disk(cache) | Self::Hybrid(cache) => {
                cache.insert(key, value);
                Ok(())
            }
        }
    }
}

impl ImgforgeCache {
    /// Create a new metadata cache instance based on the provided configuration.
    pub async fn new(config: Option<CacheConfig>) -> Result<Self, CacheError> {
        build_typed_cache(config, "image").await
    }

    /// Retrieve a value from the cache by key.
    pub async fn get(&self, key: &str) -> Option<CachedImage> {
        self.get_with_metric_labels(key, "memory", "disk", "hybrid").await
    }

    /// Insert a value into the cache.
    pub async fn insert(&self, key: String, value: CachedImage) -> Result<(), CacheError> {
        self.insert_value(key, value).await
    }
}

impl MetadataCache {
    /// Create a new metadata cache instance based on the provided configuration.
    pub async fn new(config: Option<CacheConfig>) -> Result<Self, CacheError> {
        build_typed_cache(config, "metadata").await
    }

    /// Retrieve metadata from the cache by key.
    pub async fn get(&self, key: &str) -> Option<CachedMetadata> {
        self.get_with_metric_labels(key, "metadata-memory", "metadata-disk", "metadata-hybrid")
            .await
    }

    /// Insert metadata into the cache.
    pub async fn insert(&self, key: String, value: CachedMetadata) -> Result<(), CacheError> {
        self.insert_value(key, value).await
    }
}

async fn build_typed_cache<T>(config: Option<CacheConfig>, cache_name: &str) -> Result<TypedCache<T>, CacheError>
where
    T: Clone + Code + Send + Sync + 'static,
{
    match config {
        None => Ok(TypedCache::None),
        Some(CacheConfig::Memory { capacity, .. }) => {
            let cache = CacheBuilder::new(capacity).build();
            Ok(TypedCache::Memory(Arc::new(cache)))
        }
        Some(CacheConfig::Disk { path, capacity, .. }) => {
            let cache = build_storage_cache(
                cache_name,
                "disk",
                &path,
                capacity,
                DISK_MODE_MEMORY_CAPACITY,
                Some(DISK_MODE_MEMORY_SHARDS),
            )
            .await?;
            Ok(TypedCache::Disk(Arc::new(cache)))
        }
        Some(CacheConfig::Hybrid {
            memory_capacity,
            disk_path,
            disk_capacity,
            ..
        }) => {
            let cache =
                build_storage_cache(cache_name, "hybrid", &disk_path, disk_capacity, memory_capacity, None).await?;
            Ok(TypedCache::Hybrid(Arc::new(cache)))
        }
    }
}

async fn build_storage_cache<T>(
    cache_name: &str,
    mode: &str,
    path: &str,
    capacity: usize,
    memory_capacity: usize,
    memory_shards: Option<usize>,
) -> Result<HybridCache<String, T>, CacheError>
where
    T: Clone + Code + Send + Sync + 'static,
{
    let device = FsDeviceBuilder::new(Path::new(path))
        .with_capacity(capacity)
        .build()
        .map_err(|e| CacheError::Initialization(e.to_string()))?;
    let block_size = block_size_for_capacity(capacity);
    info!(
        cache = cache_name,
        mode, capacity, block_size, "Using disk cache block size"
    );
    let engine = BlockEngineConfig::new(device).with_block_size(block_size);
    let builder = HybridCacheBuilder::new().memory(memory_capacity);
    let builder = match memory_shards {
        Some(shards) => builder.with_shards(shards),
        None => builder,
    };

    builder
        .storage()
        .with_engine_config(engine)
        .with_recover_mode(RecoverMode::Quiet)
        .build()
        .await
        .map_err(|e| CacheError::Initialization(e.to_string()))
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
