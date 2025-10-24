use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{LocciKVError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub id: u64,
    pub bind_addr: String,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub backend: String,
    pub rocksdb: RocksDBConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocksDBConfig {
    pub max_open_files: i32,
    pub write_buffer_size: usize,
    pub max_write_buffer_number: i32,
    pub target_file_size_base: u64,
    pub enable_statistics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Config {
    /// Load configuration with priority: CLI > ENV > Config File > Defaults
    pub fn load(config_path: Option<String>) -> Result<Self> {
        // Start with defaults
        let mut config = Self::default();

        // Load from config file if provided
        if let Some(path) = config_path {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| LocciKVError::Config(format!("Failed to read config file: {}", e)))?;
            
            config = serde_yaml::from_str(&contents)
                .map_err(|e| LocciKVError::Config(format!("Failed to parse config file: {}", e)))?;
        }

        Ok(config)
    }

    /// Merge CLI overrides into the config
    pub fn merge_overrides(&mut self, id: Option<u64>, bind_addr: Option<String>, data_dir: Option<PathBuf>) {
        if let Some(id) = id {
            self.server.id = id;
        }
        if let Some(addr) = bind_addr {
            self.server.bind_addr = addr;
        }
        if let Some(dir) = data_dir {
            self.server.data_dir = dir;
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                id: 1,
                bind_addr: "127.0.0.1:8080".to_string(),
                data_dir: PathBuf::from("./data"),
            },
            storage: StorageConfig {
                backend: "rocksdb".to_string(),
                rocksdb: RocksDBConfig {
                    max_open_files: 1000,
                    write_buffer_size: 64 * 1024 * 1024, // 64MB
                    max_write_buffer_number: 3,
                    target_file_size_base: 64 * 1024 * 1024, // 64MB
                    enable_statistics: true,
                },
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
            },
        }
    }
}
