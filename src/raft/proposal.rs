use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Proposal {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

#[derive(Debug)]
pub struct PendingProposal {
    pub proposal: Proposal,
    pub response_tx: tokio::sync::oneshot::Sender<crate::error::Result<()>>,
}
