use std::sync::Arc;
use tracing::info;
use crate::config::Config;
use crate::error::{LocciKVError, Result};
use crate::storage::{Storage, rocksdb_storage::RocksDBStorage};

pub struct Server {
    config: Config,
    storage: Arc<dyn Storage>,
}

impl Server {
    pub fn new(config: Config) -> Result<Self> {
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&config.server.data_dir)
            .map_err(|e| LocciKVError::Config(format!("Failed to create data directory: {}", e)))?;

        // Initialize storage
        let storage_path = config.server.data_dir.join("rocksdb");
        let storage = RocksDBStorage::new(storage_path, &config.storage.rocksdb)?;

        info!("Initialized RocksDB storage at {:?}", config.server.data_dir);

        Ok(Self {
            config,
            storage: Arc::new(storage),
        })
    }

    pub async fn start(self) -> Result<()> {
        let addr = self.config.server.bind_addr.clone();
        
        info!("Starting Locci KV server on {}", addr);
        info!("Server ID: {}", self.config.server.id);
        info!("Data directory: {:?}", self.config.server.data_dir);

        // Start HTTP API server
        crate::api::http::start_http_server(addr, self.storage).await?;

        Ok(())
    }

    pub fn storage(&self) -> Arc<dyn Storage> {
        self.storage.clone()
    }
}
