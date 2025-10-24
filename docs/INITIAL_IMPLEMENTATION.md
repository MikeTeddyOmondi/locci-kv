# Initial Implementation

Dependencies

```toml
[package]
name = "locci-kv"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dashmap = "5.5"

```

Source

```rust
#![allow(unused)]

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct LogEntry {
    term: u64,
    index: u64,
    key: String,
    value: String,
    op_type: String, // "set" or "delete"
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct RaftState {
    current_term: u64,
    voted_for: Option<u64>,
    log: Vec<LogEntry>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
enum Message {
    Set { key: String, value: String },
    Get { key: String },
    Delete { key: String },
    AppendEntries {
        term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    },
    RequestVote {
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
}

#[derive(Serialize, Deserialize, Debug)]
enum Response {
    Ok(String),
    Error(String),
    Value(Option<String>),
}

struct Node {
    id: u64,
    is_leader: Arc<RwLock<bool>>,
    data: Arc<DashMap<String, String>>,
    raft_state: Arc<RwLock<RaftState>>,
    commit_index: Arc<RwLock<u64>>,
    peers: Vec<(u64, String)>, // (peer_id, addr)
}

impl Node {
    async fn new(
        id: u64,
        is_leader: bool,
        peers: Vec<(u64, String)>,
    ) -> Self {
        Node {
            id,
            is_leader: Arc::new(RwLock::new(is_leader)),
            data: Arc::new(DashMap::new()),
            raft_state: Arc::new(RwLock::new(RaftState {
                current_term: 0,
                voted_for: None,
                log: Vec::new(),
            })),
            commit_index: Arc::new(RwLock::new(0)),
            peers,
        }
    }

    async fn handle_set(&self, key: String, value: String) -> Response {
        if !*self.is_leader.read().await {
            return Response::Error("Not leader".to_string());
        }

        let mut state = self.raft_state.write().await;
        let entry = LogEntry {
            term: state.current_term,
            index: state.log.len() as u64,
            key: key.clone(),
            value: value.clone(),
            op_type: "set".to_string(),
        };

        state.log.push(entry);
        drop(state);

        // Apply to state machine
        self.data.insert(key.clone(), value);
        let mut commit = self.commit_index.write().await;
        *commit = (*commit + 1).max(self.raft_state.read().await.log.len() as u64);

        Response::Ok("Set".to_string())
    }

    async fn handle_get(&self, key: String) -> Response {
        match self.data.get(&key) {
            Some(val) => Response::Value(Some(val.clone())),
            None => Response::Value(None),
        }
    }

    async fn handle_delete(&self, key: String) -> Response {
        if !*self.is_leader.read().await {
            return Response::Error("Not leader".to_string());
        }

        let mut state = self.raft_state.write().await;
        let entry = LogEntry {
            term: state.current_term,
            index: state.log.len() as u64,
            key: key.clone(),
            value: String::new(),
            op_type: "delete".to_string(),
        };

        state.log.push(entry);
        drop(state);

        self.data.remove(&key);
        let mut commit = self.commit_index.write().await;
        *commit = (*commit + 1).max(self.raft_state.read().await.log.len() as u64);

        Response::Ok("Deleted".to_string())
    }

    async fn process_message(&self, msg: Message) -> Response {
        match msg {
            Message::Set { key, value } => self.handle_set(key, value).await,
            Message::Get { key } => self.handle_get(key).await,
            Message::Delete { key } => self.handle_delete(key).await,
            Message::AppendEntries { term, entries, leader_commit } => {
                let mut state = self.raft_state.write().await;
                if term > state.current_term {
                    state.current_term = term;
                    *self.is_leader.write().await = false;
                }

                for entry in entries {
                    match entry.op_type.as_str() {
                        "set" => {
                            self.data.insert(entry.key, entry.value);
                        }
                        "delete" => {
                            self.data.remove(&entry.key);
                        }
                        _ => {}
                    }
                }

                let mut commit = self.commit_index.write().await;
                *commit = (*commit).max(leader_commit);

                Response::Ok("Entries appended".to_string())
            }
            Message::RequestVote { term, candidate_id, last_log_index, last_log_term } => {
                let mut state = self.raft_state.write().await;
                if term > state.current_term {
                    state.current_term = term;
                    state.voted_for = Some(candidate_id);
                    Response::Ok("Vote granted".to_string())
                } else {
                    Response::Error("Vote denied".to_string())
                }
            }
        }
    }

    async fn handle_client(&self, mut socket: TcpStream) {
        let mut buffer = vec![0; 1024];

        loop {
            match socket.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    let msg_str = String::from_utf8_lossy(&buffer[..n]);

                    if let Ok(msg) = serde_json::from_str::<Message>(&msg_str) {
                        let response = self.process_message(msg).await;
                        let response_json = serde_json::to_string(&response).unwrap();
                        let _ = socket.write_all(response_json.as_bytes()).await;
                    }
                }
                Err(_) => break,
            }
        }
    }

    async fn start_server(&self, addr: String) {
        let listener = TcpListener::bind(&addr).await.unwrap();
        println!("Node {} listening on {}", self.id, addr);

        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    let node_clone = Node {
                        id: self.id,
                        is_leader: Arc::clone(&self.is_leader),
                        data: Arc::clone(&self.data),
                        raft_state: Arc::clone(&self.raft_state),
                        commit_index: Arc::clone(&self.commit_index),
                        peers: self.peers.clone(),
                    };

                    tokio::spawn(async move {
                        node_clone.handle_client(socket).await;
                    });
                }
                Err(_) => {}
            }
        }
    }

    async fn replicate_to_peers(&self, entry: LogEntry) {
        let msg = Message::AppendEntries {
            term: self.raft_state.read().await.current_term,
            entries: vec![entry],
            leader_commit: *self.commit_index.read().await,
        };

        for (_, peer_addr) in &self.peers {
            let msg_clone = msg.clone();
            let addr = peer_addr.clone();

            tokio::spawn(async move {
                if let Ok(mut stream) = TcpStream::connect(&addr).await {
                    let msg_json = serde_json::to_string(&msg_clone).unwrap();
                    let _ = stream.write_all(msg_json.as_bytes()).await;
                }
            });
        }
    }
}

#[tokio::main]
async fn main() {
    // Start 3 nodes: 1 leader, 2 followers
    let peers = vec![
        (1, "127.0.0.1:5001".to_string()),
        (2, "127.0.0.1:5002".to_string()),
        (3, "127.0.0.1:5003".to_string()),
    ];

    let node1 = Arc::new(Node::new(1, true, peers.clone()).await);
    let node2 = Arc::new(Node::new(2, false, peers.clone()).await);
    let node3 = Arc::new(Node::new(3, false, peers.clone()).await);

    // Start servers
    let node1_clone = Arc::clone(&node1);
    tokio::spawn(async move {
        node1_clone.start_server("127.0.0.1:5001".to_string()).await;
    });

    let node2_clone = Arc::clone(&node2);
    tokio::spawn(async move {
        node2_clone.start_server("127.0.0.1:5002".to_string()).await;
    });

    let node3_clone = Arc::clone(&node3);
    tokio::spawn(async move {
        node3_clone.start_server("127.0.0.1:5003".to_string()).await;
    });

    // Demo: Simulate concurrent operations
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    println!("\n=== Locci KV Demo ===\n");

    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        let msg = Message::Set {
            key: key.clone(),
            value: value.clone()
        };
        let _ = node1.process_message(msg).await;

        if i % 10 == 0 {
            println!("Inserted {} entries", i + 1);
        }
    }

    // Verify all keys are stored
    let count = node1.data.len();
    println!("\nTotal keys stored: {}", count);
    println!("Leader status: {}", *node1.is_leader.read().await);
    println!("Log entries: {}", node1.raft_state.read().await.log.len());

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}

```
