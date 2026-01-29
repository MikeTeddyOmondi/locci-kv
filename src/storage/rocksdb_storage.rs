use super::{Storage, StorageStats};
use crate::config::RocksDBConfig;
use crate::error::Result;
use async_trait::async_trait;
use rocksdb::{IteratorMode, Options, DB};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RocksDBStorage {
    db: Arc<RwLock<DB>>,
}

impl RocksDBStorage {
    pub fn new<P: AsRef<Path>>(path: P, config: &RocksDBConfig) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(config.max_open_files);
        opts.set_write_buffer_size(config.write_buffer_size);
        opts.set_max_write_buffer_number(config.max_write_buffer_number);
        opts.set_target_file_size_base(config.target_file_size_base);

        if config.enable_statistics {
            opts.enable_statistics();
        }

        // Use Snappy compression
        opts.set_compression_type(rocksdb::DBCompressionType::Snappy);

        let db = DB::open(&opts, path)?;

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
        })
    }
}

#[async_trait]
impl Storage for RocksDBStorage {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let db = self.db.read().await;
        Ok(db.get(key)?)
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let db = self.db.write().await;
        db.put(key, value)?;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let db = self.db.write().await;
        db.delete(key)?;
        Ok(())
    }

    async fn exists(&self, key: &[u8]) -> Result<bool> {
        let db = self.db.read().await;
        Ok(db.get(key)?.is_some())
    }

    async fn list_keys(&self, prefix: Option<&[u8]>) -> Result<Vec<Vec<u8>>> {
        let db = self.db.read().await;
        let mut keys = Vec::new();

        let iter = db.iterator(IteratorMode::Start);

        for item in iter {
            let (key, _) = item?;

            // Filter by prefix if provided
            if let Some(p) = prefix {
                if key.starts_with(p) {
                    keys.push(key.to_vec());
                }
            } else {
                keys.push(key.to_vec());
            }
        }

        Ok(keys)
    }

    async fn stats(&self) -> Result<StorageStats> {
        let db = self.db.read().await;

        // Count keys and estimate size
        let mut total_keys = 0u64;
        let mut total_size = 0u64;

        let iter = db.iterator(IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            total_keys += 1;
            total_size += key.len() as u64 + value.len() as u64;
        }

        Ok(StorageStats {
            total_keys,
            total_size_bytes: total_size,
            backend: "rocksdb".to_string(),
        })
    }
}
