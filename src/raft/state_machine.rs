use crate::error::Result;
use crate::raft::proposal::Proposal;
use crate::storage::Storage;
use std::sync::Arc;
use tracing::{debug, info};

/// State machine that applies committed Raft entries to storage
pub struct StateMachine {
    storage: Arc<dyn Storage>,
}

impl StateMachine {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Apply a committed proposal to the state machine
    pub async fn apply(&self, proposal: Proposal) -> Result<()> {
        match proposal {
            Proposal::Put { key, value } => {
                debug!("Applying PUT: key={:?}", String::from_utf8_lossy(&key));
                self.storage.put(&key, &value).await?;
                info!("Applied PUT successfully");
            }
            Proposal::Delete { key } => {
                debug!("Applying DELETE: key={:?}", String::from_utf8_lossy(&key));
                self.storage.delete(&key).await?;
                info!("Applied DELETE successfully");
            }
        }
        Ok(())
    }
}
