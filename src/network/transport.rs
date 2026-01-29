use crate::config::PeerConfig;
use crate::error::{LocciKVError, Result};
use parking_lot::RwLock;
use prost::Message;
use raft::eraftpb::Message as RaftMessage;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// TCP-based network transport for Raft messages
pub struct NetworkTransport {
    node_id: u64,
    local_addr: String,
    peers: Arc<RwLock<HashMap<u64, PeerConfig>>>,
    message_tx: mpsc::UnboundedSender<RaftMessage>,
    message_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<RaftMessage>>>,
    /// Cached connections to peers
    connections: Arc<tokio::sync::Mutex<HashMap<u64, TcpStream>>>,
}

impl NetworkTransport {
    pub fn new(node_id: u64, peers: Vec<PeerConfig>) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        // Find this node's address from peers
        let local_addr = peers
            .iter()
            .find(|p| p.id == node_id)
            .map(|p| p.addr.clone())
            .unwrap_or_else(|| "127.0.0.1:9001".to_string());

        let peer_map: HashMap<u64, PeerConfig> = peers.into_iter().map(|p| (p.id, p)).collect();

        Self {
            node_id,
            local_addr,
            peers: Arc::new(RwLock::new(peer_map)),
            message_tx,
            message_rx: Arc::new(tokio::sync::Mutex::new(message_rx)),
            connections: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Start the TCP server to receive Raft messages
    pub async fn start_server(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.local_addr).await.map_err(|e| {
            LocciKVError::Network(format!("Failed to bind to {}: {}", self.local_addr, e))
        })?;

        info!("Raft transport listening on {}", self.local_addr);

        let message_tx = self.message_tx.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        debug!("Accepted Raft connection from {}", addr);
                        let tx = message_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(stream, tx).await {
                                warn!("Connection handler error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle an incoming connection - read messages and forward to channel
    async fn handle_connection(
        mut stream: TcpStream,
        tx: mpsc::UnboundedSender<RaftMessage>,
    ) -> Result<()> {
        loop {
            // Read length prefix (4 bytes, big-endian)
            let mut len_buf = [0u8; 4];
            match stream.read_exact(&mut len_buf).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    debug!("Connection closed");
                    return Ok(());
                }
                Err(e) => {
                    return Err(LocciKVError::Network(format!("Read error: {}", e)));
                }
            }

            let len = u32::from_be_bytes(len_buf) as usize;
            if len == 0 || len > 10 * 1024 * 1024 {
                // Sanity check: max 10MB
                return Err(LocciKVError::Network(format!("Invalid message length: {}", len)));
            }

            // Read message body
            let mut buf = vec![0u8; len];
            stream.read_exact(&mut buf).await.map_err(|e| {
                LocciKVError::Network(format!("Failed to read message body: {}", e))
            })?;

            // Decode RaftMessage
            let msg = RaftMessage::decode(&buf[..])
                .map_err(|e| LocciKVError::Network(format!("Failed to decode message: {}", e)))?;

            debug!(
                "Received Raft message: {:?} from node {}",
                msg.msg_type(),
                msg.from
            );

            // Forward to channel
            if tx.send(msg).is_err() {
                return Err(LocciKVError::Network("Channel closed".to_string()));
            }
        }
    }

    /// Send a Raft message to a peer
    pub async fn send_message(&self, msg: RaftMessage) -> Result<()> {
        let to = msg.to;

        if to == self.node_id {
            // Message to self, just queue it
            self.message_tx
                .send(msg)
                .map_err(|_| LocciKVError::Network("Failed to send message to self".to_string()))?;
            return Ok(());
        }

        // Get peer address
        let peer_addr = {
            let peers = self.peers.read();
            peers.get(&to).map(|p| p.addr.clone())
        };

        let peer_addr = match peer_addr {
            Some(addr) => addr,
            None => {
                warn!("Unknown peer {}, dropping message", to);
                return Ok(());
            }
        };

        // Encode message
        let mut buf = Vec::with_capacity(msg.encoded_len());
        msg.encode(&mut buf)
            .map_err(|e| LocciKVError::Network(format!("Failed to encode message: {}", e)))?;

        // Try to get cached connection or create new one
        let send_result = self.send_to_peer(to, &peer_addr, &buf).await;

        if let Err(e) = send_result {
            // Remove failed connection from cache
            self.connections.lock().await.remove(&to);
            debug!("Failed to send to peer {}: {}", to, e);
            // Don't propagate error - message loss is expected in network partitions
        }

        Ok(())
    }

    /// Send data to a peer, with connection caching
    async fn send_to_peer(&self, peer_id: u64, addr: &str, data: &[u8]) -> Result<()> {
        let mut connections = self.connections.lock().await;

        // Try to use cached connection
        if let Some(stream) = connections.get_mut(&peer_id) {
            if Self::write_message(stream, data).await.is_ok() {
                return Ok(());
            }
            // Connection failed, remove it
            connections.remove(&peer_id);
        }

        // Create new connection
        let mut stream = TcpStream::connect(addr)
            .await
            .map_err(|e| LocciKVError::Network(format!("Failed to connect to {}: {}", addr, e)))?;

        Self::write_message(&mut stream, data).await?;

        // Cache the connection
        connections.insert(peer_id, stream);

        Ok(())
    }

    /// Write a length-prefixed message to a stream
    async fn write_message(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
        let len = data.len() as u32;
        stream.write_all(&len.to_be_bytes()).await.map_err(|e| {
            LocciKVError::Network(format!("Failed to write length: {}", e))
        })?;
        stream.write_all(data).await.map_err(|e| {
            LocciKVError::Network(format!("Failed to write data: {}", e))
        })?;
        stream.flush().await.map_err(|e| {
            LocciKVError::Network(format!("Failed to flush: {}", e))
        })?;
        Ok(())
    }

    /// Send multiple messages
    pub async fn send_messages(&self, messages: Vec<RaftMessage>) -> Result<()> {
        for msg in messages {
            self.send_message(msg).await?;
        }
        Ok(())
    }

    /// Receive a message (blocking - waits for next message)
    pub async fn recv_message(&self) -> Option<RaftMessage> {
        let mut rx = self.message_rx.lock().await;
        rx.recv().await
    }

    /// Add a peer
    pub fn add_peer(&self, peer: PeerConfig) {
        self.peers.write().insert(peer.id, peer);
    }

    /// Remove a peer
    pub fn remove_peer(&self, peer_id: u64) {
        self.peers.write().remove(&peer_id);
    }
}
