use crate::error::Result;
use async_trait::async_trait;

pub mod rocksdb_storage;

/// Storage trait for key-value operations
#[async_trait]
pub trait Storage: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Put a key-value pair
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Delete a key
    async fn delete(&self, key: &[u8]) -> Result<()>;

    /// Check if a key exists
    async fn exists(&self, key: &[u8]) -> Result<bool>;

    /// List all keys with optional prefix
    async fn list_keys(&self, prefix: Option<&[u8]>) -> Result<Vec<Vec<u8>>>;

    /// Get storage statistics
    async fn stats(&self) -> Result<StorageStats>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageStats {
    pub total_keys: u64,
    pub total_size_bytes: u64,
    pub backend: String,
}
