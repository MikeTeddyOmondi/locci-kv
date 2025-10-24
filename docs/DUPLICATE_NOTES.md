## Project Overview

Locci KV is a distributed key-value store built on top of Raft consensus using `raft-rs`, with RocksDB as the storage backend. It supports configuration via CLI arguments, YAML config files, and environment variables.

**Current Phase**: Phase 1 - Single Node MVP with full implementation

## Project Structure

```
locci-kv/
├── Cargo.toml
├── config.yaml
├── README.md
├── src/
│   ├── main.rs              # Entry point, CLI setup
│   ├── lib.rs               # Library exports
│   ├── config.rs            # Configuration management
│   ├── error.rs             # Error types
│   ├── storage/
│   │   ├── mod.rs           # Storage trait
│   │   └── rocksdb.rs       # RocksDB implementation
│   ├── server.rs            # Server/Node implementation
│   └── api/
│       ├── mod.rs           # API exports
│       └── http.rs          # HTTP API handlers
└── tests/
    └── integration_test.rs
```

## Complete Implementation Files

```
┌─────────────────────────────────────────────────────────────┐
│                        Locci KV Node                         │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   CLI/API   │  │  Raft Layer  │  │  Storage Engine  │  │
│  │   Handler   │─▶│  (raft-rs)   │─▶│    (RocksDB)     │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
│         │                 │                    │            │
│         ▼                 ▼                    ▼            │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Connection  │  │  Network/RPC │  │   Write-Ahead    │  │
│  │    Pool     │  │   Transport  │  │      Log         │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Configuration System

### Priority Order (Highest to Lowest)
1. CLI flags
2. Environment variables
3. Config file (YAML)
4. Default values

### Configuration Schema

```yaml
# config.yaml
server:
  id: 1                              # Unique node identifier
  bind_addr: "127.0.0.1:8080"       # Server bind address
  data_dir: "./data"                 # Data storage directory
  
raft:
  heartbeat_tick: 100                # Heartbeat interval (ms)
  election_tick: 300                 # Election timeout (ms)
  max_size_per_msg: 1048576          # Max message size (1MB)
  max_inflight_msgs: 256             # Max in-flight messages
  
peers:
  - id: 1
    addr: "127.0.0.1:9001"
  - id: 2
    addr: "127.0.0.1:9002"
  - id: 3
    addr: "127.0.0.1:9003"

storage:
  backend: "rocksdb"
  rocksdb:
    max_open_files: 1000
    write_buffer_size: 67108864      # 64MB
    max_write_buffer_number: 3
    target_file_size_base: 67108864  # 64MB
    enable_statistics: true
    compression: "snappy"

connection_pool:
  max_size: 100                      # Max connections
  min_idle: 10                       # Min idle connections
  connection_timeout: 5000           # Connection timeout (ms)
  idle_timeout: 300000               # Idle timeout (ms)

logging:
  level: "info"                      # debug, info, warn, error
  format: "json"                     # json or text
  file: "./logs/locci-kv.log"
```

## CLI Interface

### Using Clap

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "locci-kv")]
#[command(about = "A distributed key-value store built on Raft", long_about = None)]
struct Cli {
    /// Path to config file (can be set via LOCCI_CONFIG env var)
    #[arg(short, long, env = "LOCCI_CONFIG", default_value = "config.yaml")]
    config: String,

    /// Server ID
    #[arg(long, env = "LOCCI_SERVER_ID")]
    id: Option<u64>,

    /// Server bind address
    #[arg(long, env = "LOCCI_BIND_ADDR")]
    bind_addr: Option<String>,

    /// Data directory
    #[arg(long, env = "LOCCI_DATA_DIR")]
    data_dir: Option<String>,

    /// Log level (debug, info, warn, error)
    #[arg(long, env = "LOCCI_LOG_LEVEL", default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Locci KV server
    Start {
        /// Bootstrap a new cluster
        #[arg(long)]
        bootstrap: bool,
    },
    
    /// Join an existing cluster
    Join {
        /// Address of an existing cluster member
        #[arg(long)]
        peer: String,
    },
    
    /// Run in standalone mode (single node)
    Standalone,
}
```

### Example Usage

```bash
# Start with config file
locci-kv --config /etc/locci/config.yaml start

# Start with env var
export LOCCI_CONFIG=/etc/locci/config.yaml
locci-kv start --bootstrap

# Override specific settings
locci-kv --id 1 --bind-addr 0.0.0.0:8080 start

# Join existing cluster
locci-kv --id 4 join --peer 127.0.0.1:9001

# Standalone mode for testing
locci-kv standalone
```

## Core Components

### 1. Primitive Node Structure

```rust
pub struct Node {
    // Core identification
    id: u64,
    
    // Raft consensus
    raft_node: RawNode<RaftStorage>,
    
    // Storage backend
    storage: Arc<RocksDBStorage>,
    
    // Network layer
    transport: NetworkTransport,
    
    // Connection pool for client connections
    conn_pool: ConnectionPool,
    
    // Configuration
    config: Config,
}

impl Node {
    pub fn new(config: Config) -> Result<Self> {
        // Initialize RocksDB storage
        let storage = Arc::new(RocksDBStorage::new(&config.storage)?);
        
        // Setup Raft configuration
        let raft_config = raft::Config {
            id: config.server.id,
            election_tick: config.raft.election_tick,
            heartbeat_tick: config.raft.heartbeat_tick,
            max_size_per_msg: config.raft.max_size_per_msg,
            max_inflight_msgs: config.raft.max_inflight_msgs,
            ..Default::default()
        };
        
        // Create Raft node
        let raft_storage = RaftStorage::new(storage.clone());
        let raft_node = RawNode::new(&raft_config, raft_storage)?;
        
        // Initialize network transport
        let transport = NetworkTransport::new(config.server.bind_addr.clone())?;
        
        // Setup connection pool
        let conn_pool = ConnectionPool::new(config.connection_pool.clone())?;
        
        Ok(Node {
            id: config.server.id,
            raft_node,
            storage,
            transport,
            conn_pool,
            config,
        })
    }
    
    pub async fn start(&mut self) -> Result<()> {
        // Start network listener
        self.transport.start().await?;
        
        // Main event loop
        loop {
            // Process Raft messages
            if self.raft_node.has_ready() {
                self.handle_ready().await?;
            }
            
            // Handle incoming network messages
            if let Some(msg) = self.transport.recv_message().await {
                self.handle_network_message(msg).await?;
            }
            
            // Tick the Raft node
            self.raft_node.tick();
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    
    async fn handle_ready(&mut self) -> Result<()> {
        let mut ready = self.raft_node.ready();
        
        // Persist entries to stable storage
        if !ready.entries().is_empty() {
            self.storage.append_entries(ready.entries())?;
        }
        
        // Apply committed entries to state machine
        if let Some(committed_entries) = ready.committed_entries.take() {
            for entry in committed_entries {
                self.apply_entry(&entry)?;
            }
        }
        
        // Send messages to peers
        for msg in ready.messages.drain(..) {
            self.transport.send_message(msg).await?;
        }
        
        // Advance the Raft node
        let mut light_rd = self.raft_node.advance(ready);
        
        // Handle light ready
        if let Some(commit) = light_rd.commit_index() {
            self.storage.set_hard_state_commit(commit)?;
        }
        
        self.transport.send_messages(light_rd.take_messages()).await?;
        
        Ok(())
    }
}
```

### 2. RocksDB Storage Backend

```rust
use rocksdb::{DB, Options, ColumnFamily, WriteBatch};

pub struct RocksDBStorage {
    db: Arc<DB>,
}

impl RocksDBStorage {
    pub fn new(config: &StorageConfig) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(config.rocksdb.max_open_files);
        opts.set_write_buffer_size(config.rocksdb.write_buffer_size);
        opts.set_max_write_buffer_number(config.rocksdb.max_write_buffer_number);
        opts.set_target_file_size_base(config.rocksdb.target_file_size_base);
        opts.set_compression_type(rocksdb::DBCompressionType::Snappy);
        
        // Column families for different data types
        let cf_names = vec!["data", "metadata", "raft_log"];
        let db = DB::open_cf(&opts, &config.data_dir, cf_names)?;
        
        Ok(Self {
            db: Arc::new(db),
        })
    }
    
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf = self.db.cf_handle("data").unwrap();
        Ok(self.db.get_cf(cf, key)?)
    }
    
    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle("data").unwrap();
        self.db.put_cf(cf, key, value)?;
        Ok(())
    }
    
    pub fn delete(&self, key: &[u8]) -> Result<()> {
        let cf = self.db.cf_handle("data").unwrap();
        self.db.delete_cf(cf, key)?;
        Ok(())
    }
    
    pub fn batch_write(&self, operations: Vec<Operation>) -> Result<()> {
        let mut batch = WriteBatch::default();
        let cf = self.db.cf_handle("data").unwrap();
        
        for op in operations {
            match op {
                Operation::Put(key, value) => batch.put_cf(cf, key, value),
                Operation::Delete(key) => batch.delete_cf(cf, key),
            }
        }
        
        self.db.write(batch)?;
        Ok(())
    }
}

// Implement raft::Storage trait for RocksDB
pub struct RaftStorage {
    store: Arc<RocksDBStorage>,
}

impl raft::Storage for RaftStorage {
    fn initial_state(&self) -> Result<RaftState> {
        // Read hard state and conf state from RocksDB
        // ...
    }
    
    fn entries(&self, low: u64, high: u64, max_size: impl Into<Option<u64>>) 
        -> Result<Vec<Entry>> {
        // Read log entries from RocksDB
        // ...
    }
    
    fn term(&self, idx: u64) -> Result<u64> {
        // Get term for a specific log index
        // ...
    }
    
    fn first_index(&self) -> Result<u64> {
        // Get first log index
        // ...
    }
    
    fn last_index(&self) -> Result<u64> {
        // Get last log index
        // ...
    }
    
    fn snapshot(&self, request_index: u64, to: u64) -> Result<Snapshot> {
        // Create snapshot
        // ...
    }
}
```

### 3. Connection Pooling

```rust
use deadpool::managed::{Manager, Pool, RecycleResult};

pub struct ConnectionPool {
    pool: Pool<ConnectionManager>,
}

struct ConnectionManager {
    config: ConnectionPoolConfig,
}

#[async_trait]
impl Manager for ConnectionManager {
    type Type = Connection;
    type Error = Error;
    
    async fn create(&self) -> Result<Connection> {
        // Create new connection
        Connection::new(&self.config).await
    }
    
    async fn recycle(&self, conn: &mut Connection) -> RecycleResult<Self::Error> {
        // Check if connection is still valid
        if conn.is_alive().await {
            Ok(())
        } else {
            Err(RecycleResult::Closed)
        }
    }
}

impl ConnectionPool {
    pub fn new(config: ConnectionPoolConfig) -> Result<Self> {
        let manager = ConnectionManager { config: config.clone() };
        let pool = Pool::builder(manager)
            .max_size(config.max_size)
            .build()?;
        
        Ok(Self { pool })
    }
    
    pub async fn get_connection(&self) -> Result<PooledConnection> {
        Ok(self.pool.get().await?)
    }
}
```

### 4. API Layer

```rust
// Simple KV API
pub struct KVApi {
    node: Arc<RwLock<Node>>,
}

impl KVApi {
    pub async fn get(&self, key: String) -> Result<Option<Vec<u8>>> {
        let node = self.node.read().await;
        node.storage.get(key.as_bytes())
    }
    
    pub async fn put(&self, key: String, value: Vec<u8>) -> Result<()> {
        // Create Raft proposal
        let proposal = Proposal::Put { key, value };
        let data = bincode::serialize(&proposal)?;
        
        // Submit to Raft
        let mut node = self.node.write().await;
        node.raft_node.propose(vec![], data)?;
        
        // Wait for commit
        // ... (implement wait mechanism)
        
        Ok(())
    }
    
    pub async fn delete(&self, key: String) -> Result<()> {
        let proposal = Proposal::Delete { key };
        let data = bincode::serialize(&proposal)?;
        
        let mut node = self.node.write().await;
        node.raft_node.propose(vec![], data)?;
        
        Ok(())
    }
}
```

## Building & Running

### Dependencies (Cargo.toml)

```toml
[package]
name = "locci-kv"
version = "0.1.0"
edition = "2021"

[dependencies]
# CLI and configuration
clap = { version = "4.4", features = ["derive", "env"] }
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
config = "0.14"

# Raft consensus
raft = "0.7"
raft-proto = "0.7"

# Storage
rocksdb = "0.22"

# Async runtime
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"

# Connection pooling
deadpool = "0.12"

# Serialization
bincode = "1.3"
prost = "0.12"

# Networking
tonic = "0.11"
bytes = "1.5"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }

# Error handling
anyhow = "1.0"
thiserror = "1.0"
```

### Build Commands

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Run with config file
./target/release/locci-kv --config config.yaml start

# Run in development
cargo run -- --id 1 --bind-addr 127.0.0.1:8080 start --bootstrap
```

## Development Roadmap

### Phase 1: Single Node (Primitive)
- ✅ CLI configuration with clap
- ✅ Config file (YAML) support
- ✅ Basic RocksDB storage
- ✅ Simple KV API (get, put, delete)
- ✅ Logging infrastructure

### Phase 2: Raft Integration
- 🔄 Integrate raft-rs
- 🔄 Implement RaftStorage trait
- 🔄 Network transport layer
- 🔄 Consensus for writes
- 🔄 Leader election

### Phase 3: Multi-Node Cluster
- ⬜ Peer discovery
- ⬜ Cluster configuration
- ⬜ Node join/leave
- ⬜ Snapshot support
- ⬜ Log compaction

### Phase 4: Production Features
- ⬜ Connection pooling
- ⬜ gRPC API
- ⬜ HTTP REST API
- ⬜ Metrics & monitoring
- ⬜ Admin operations
- ⬜ Backup & restore

### Phase 5: Advanced Features
- ⬜ Read replicas
- ⬜ Range queries
- ⬜ TTL support
- ⬜ Watch/subscribe
- ⬜ Transactions

## Testing Strategy

```bash
# Start 3-node cluster locally
./locci-kv --id 1 --bind-addr 127.0.0.1:8081 --data-dir ./data1 start --bootstrap
./locci-kv --id 2 --bind-addr 127.0.0.1:8082 --data-dir ./data2 join --peer 127.0.0.1:9001
./locci-kv --id 3 --bind-addr 127.0.0.1:8083 --data-dir ./data3 join --peer 127.0.0.1:9001

# Test operations
curl -X PUT http://127.0.0.1:8081/kv/mykey -d '{"value": "myvalue"}'
curl -X GET http://127.0.0.1:8081/kv/mykey
curl -X DELETE http://127.0.0.1:8081/kv/mykey
```

## Key Design Decisions

1. **Configuration Priority**: CLI > ENV > Config File > Defaults
2. **Storage**: RocksDB with column families for data separation
3. **Consensus**: raft-rs for distributed consensus
4. **Async Runtime**: Tokio for async I/O
5. **Serialization**: Bincode for internal, JSON for external API
6. **Connection Management**: Deadpool for connection pooling
7. **Modularity**: Start simple, build incrementally

## Next Steps

1. Implement the basic Node structure with RocksDB
2. Add CLI configuration parsing with clap
3. Create simple in-memory KV store for testing
4. Integrate raft-rs for consensus
5. Build network layer for inter-node communication
6. Add connection pooling
7. Implement gRPC/HTTP API
8. Add comprehensive testing
9. Performance benchmarking
10. Production hardening

---

**Project Status**: 🟡 Design Phase  
**Target**: Production-ready distributed KV store  
**License**: MIT / Apache 2.0

---
