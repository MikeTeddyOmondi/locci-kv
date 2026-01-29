use crate::config::Config;
use crate::error::{LocciKVError, Result};
use crate::network::NetworkTransport;
use crate::raft::RaftNode;
use crate::storage::{rocksdb_storage::RocksDBStorage, Storage};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

pub struct Server {
    config: Config,
    storage: Arc<dyn Storage>,
    raft_node: Option<Arc<RaftNode>>,
    network: Option<Arc<NetworkTransport>>,
    raft_enabled: bool,
}

impl Server {
    pub fn new(config: Config) -> Result<Self> {
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&config.server.data_dir)
            .map_err(|e| LocciKVError::Config(format!("Failed to create data directory: {}", e)))?;

        // Initialize storage
        let storage_path = config.server.data_dir.join("rocksdb");
        let storage = RocksDBStorage::new(storage_path, &config.storage.rocksdb)?;

        info!(
            "Initialized RocksDB storage at {:?}",
            config.server.data_dir
        );

        Ok(Self {
            config,
            storage: Arc::new(storage),
            raft_node: None,
            network: None,
            raft_enabled: false,
        })
    }

    /// Enable Raft consensus
    pub async fn with_raft(mut self) -> Result<Self> {
        info!("Initializing Raft consensus...");

        // Create network transport
        let network = Arc::new(NetworkTransport::new(
            self.config.server.id,
            self.config.cluster.peers.clone(),
        ));

        // Create Raft node
        let raft_node = Arc::new(
            RaftNode::new(
                self.config.server.id,
                self.storage.clone(),
                &self.config.raft,
                &self.config.cluster,
            )
            .await?,
        );

        self.raft_node = Some(raft_node);
        self.network = Some(network);
        self.raft_enabled = true;

        info!(
            "Raft consensus initialized for node {}",
            self.config.server.id
        );
        Ok(self)
    }

    pub async fn start(self) -> Result<()> {
        let addr = self.config.server.bind_addr.clone();

        info!("Starting Locci KV server on {}", addr);
        info!("Server ID: {}", self.config.server.id);
        info!("Data directory: {:?}", self.config.server.data_dir);
        info!("Raft enabled: {}", self.raft_enabled);

        let storage = self.storage.clone();
        let raft_node = self.raft_node.clone();
        let network = self.network.clone();

        // Start Raft event loop if enabled
        if self.raft_enabled {
            let raft_node_clone = raft_node.clone().unwrap();
            let network_clone = network.clone().unwrap();

            // Start the TCP server for receiving Raft messages
            network_clone.start_server().await?;

            tokio::spawn(async move {
                Self::raft_event_loop(raft_node_clone, network_clone).await;
            });
        }

        // Start HTTP API server
        crate::api::http::start_http_server(addr, storage, raft_node).await?;

        Ok(())
    }

    /// Raft event loop - processes Raft state machine
    async fn raft_event_loop(raft_node: Arc<RaftNode>, network: Arc<NetworkTransport>) {
        info!("Starting Raft event loop");

        let mut tick_interval = tokio::time::interval(Duration::from_millis(100));
        let mut tick_count = 0u64;

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    tick_count += 1;

                    // Tick the Raft node
                    raft_node.tick();

                    // Log every 10 ticks (1 second)
                    if tick_count.is_multiple_of(10) {
                        debug!("Raft tick #{}, {}", tick_count, raft_node.raft_state());
                    }

                    // Process ready state
                    match raft_node.handle_ready().await {
                        Ok(messages) => {
                            if !messages.is_empty() {
                                info!("Sending {} Raft messages", messages.len());
                                if let Err(e) = network.send_messages(messages).await {
                                    error!("Failed to send Raft messages: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error handling Raft ready: {}", e);
                        }
                    }
                }

                Some(msg) = network.recv_message() => {
                    // Process incoming Raft message
                    info!("Received Raft message: {:?} from node {}", msg.msg_type(), msg.from);
                    if let Err(e) = raft_node.step(msg) {
                        error!("Error stepping Raft: {}", e);
                    }
                }
            }
        }
    }

    pub fn storage(&self) -> Arc<dyn Storage> {
        self.storage.clone()
    }

    pub fn raft_node(&self) -> Option<Arc<RaftNode>> {
        self.raft_node.clone()
    }
}
