use crate::config::PeerConfig;
use crate::error::{LocciKVError, Result};
use parking_lot::RwLock;
use raft::eraftpb::Message as RaftMessage;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::debug;

/// Simplified network transport for Raft messages
pub struct NetworkTransport {
    node_id: u64,
    peers: Arc<RwLock<HashMap<u64, PeerConfig>>>,
    message_tx: mpsc::UnboundedSender<RaftMessage>,
    message_rx: Arc<RwLock<mpsc::UnboundedReceiver<RaftMessage>>>,
}

impl NetworkTransport {
    pub fn new(node_id: u64, peers: Vec<PeerConfig>) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        
        let peer_map: HashMap<u64, PeerConfig> = peers
            .into_iter()
            .map(|p| (p.id, p))
            .collect();

        Self {
            node_id,
            peers: Arc::new(RwLock::new(peer_map)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
        }
    }

    /// Send a Raft message to a peer
    pub async fn send_message(&self, msg: RaftMessage) -> Result<()> {
        let to = msg.to;
        
        if to == self.node_id {
            // Message to self, just queue it
            self.message_tx.send(msg)
                .map_err(|_| LocciKVError::Network("Failed to send message".to_string()))?;
            return Ok(());
        }

        // In a real implementation, this would send over network
        // For now, we'll just log it
        debug!("Would send message to peer {}: {:?}", to, msg.msg_type());
        
        Ok(())
    }

    /// Send multiple messages
    pub async fn send_messages(&self, messages: Vec<RaftMessage>) -> Result<()> {
        for msg in messages {
            self.send_message(msg).await?;
        }
        Ok(())
    }

    /// Receive a message (non-blocking)
    pub async fn recv_message(&self) -> Option<RaftMessage> {
        let mut rx = self.message_rx.write();
        rx.try_recv().ok()
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
