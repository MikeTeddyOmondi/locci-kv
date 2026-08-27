use thiserror::Error;

#[derive(Error, Debug)]
pub enum LocciKVError {
    #[error("Storage error: {0}")]
    Storage(#[from] rocksdb::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Server error: {0}")]
    Server(String),

    // Raft-specific errors
    #[error("Raft error: {0}")]
    Raft(#[from] raft::Error),

    #[error("Not leader, current leader: {0:?}")]
    NotLeader(Option<u64>),

    #[error("Proposal timeout")]
    ProposalTimeout,

    #[error("Network error: {0}")]
    Network(String),

    #[error("Prost decode error: {0}")]
    ProstDecode(#[from] prost::DecodeError),

    #[error("Prost encode error: {0}")]
    ProstEncode(#[from] prost::EncodeError),
}

pub type Result<T> = std::result::Result<T, LocciKVError>;
