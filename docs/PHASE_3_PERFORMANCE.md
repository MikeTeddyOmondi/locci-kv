# Phase 3: Performance Implementation Guide

This document outlines the performance optimizations planned for Phase 3 to make Locci KV production-ready and competitive with industry KV stores.

---

## Current State (Phase 2)

### Benchmark Results

| Mode | Write RPS | Read RPS | Write Latency (p99) |
|------|-----------|----------|---------------------|
| Standalone | ~52,000 | ~56,000 | 3ms |
| Raft Single Node | ~500 | ~62,000 | 102ms |
| Raft 3-Node Cluster | ~250 | ~61,000 | 202ms |

### Root Cause Analysis

The ~100ms write latency per Raft operation is caused by the **tick-driven event loop**:

```rust
// Current implementation (simplified)
loop {
    tokio::select! {
        _ = tick_interval.tick() => {
            node.tick();
            handle_ready().await;  // Proposals processed here
        }
        // ...
    }
}
```

With `tick_interval = 100ms`, proposals must wait for the next tick cycle before being processed. This is the primary bottleneck.

### Industry Comparison

| System | Write RPS | Architecture |
|--------|-----------|--------------|
| Redis (standalone) | 100-200K+ | In-memory, single-threaded |
| etcd | 10-50K | Raft, gRPC, highly optimized |
| TiKV | 50-100K+ | Raft, distributed |
| **Locci KV (target)** | **50-100K** | Raft, RocksDB |

---

## Phase 3 Implementation Plan

### Priority 1: Event Loop Architecture (Critical)

**Goal**: Decouple proposal processing from tick timer
**Expected Impact**: 100x improvement (500 -> 50K writes/sec)

#### Current Architecture (Tick-Driven)

```
┌─────────────────────────────────────────┐
│              Event Loop                 │
├─────────────────────────────────────────┤
│  tick (100ms) ──► process proposals     │
│                   handle_ready()        │
│                   send messages         │
└─────────────────────────────────────────┘
```

#### Target Architecture (Event-Driven)

```
┌─────────────────────────────────────────┐
│              Event Loop                 │
├─────────────────────────────────────────┤
│  proposal ──────► immediate ready()     │
│  network msg ───► step() + ready()      │
│  tick (100ms) ──► heartbeat only        │
└─────────────────────────────────────────┘
```

#### Implementation

```rust
// New event loop structure
loop {
    tokio::select! {
        // Proposals trigger immediate processing
        Some(proposal) = proposal_rx.recv() => {
            node.propose(proposal.context, proposal.data)?;
            handle_ready().await?;  // Process immediately
        }

        // Network messages trigger immediate processing
        Some(msg) = network_rx.recv() => {
            node.step(msg)?;
            handle_ready().await?;  // Process immediately
        }

        // Tick only for heartbeats and leader election
        _ = tick_interval.tick() => {
            node.tick();
            handle_ready().await?;
        }
    }
}
```

#### Files to Modify

| File | Changes |
|------|---------|
| `src/raft/node.rs` | Refactor `run_event_loop()` |
| `src/server.rs` | Update event loop integration |

---

### Priority 2: Proposal Batching

**Goal**: Accumulate multiple proposals into single Raft entries
**Expected Impact**: 2-5x throughput improvement

#### Design

```rust
pub struct ProposalBatcher {
    pending: Vec<PendingProposal>,
    max_batch_size: usize,      // Default: 1000
    max_batch_bytes: usize,     // Default: 1MB
    flush_interval: Duration,   // Default: 1ms
    flush_tx: mpsc::Sender<Vec<PendingProposal>>,
}

impl ProposalBatcher {
    pub async fn add(&mut self, proposal: PendingProposal) {
        self.pending.push(proposal);

        if self.should_flush() {
            self.flush().await;
        }
    }

    fn should_flush(&self) -> bool {
        self.pending.len() >= self.max_batch_size ||
        self.pending_bytes() >= self.max_batch_bytes
    }

    async fn flush(&mut self) {
        let batch = std::mem::take(&mut self.pending);
        self.flush_tx.send(batch).await.ok();
    }
}
```

#### Batch Entry Format

```rust
#[derive(Serialize, Deserialize)]
pub struct BatchEntry {
    pub operations: Vec<KVOperation>,
}

#[derive(Serialize, Deserialize)]
pub enum KVOperation {
    Put { key: String, value: Vec<u8> },
    Delete { key: String },
}
```

#### Configuration

```yaml
raft:
  batching:
    enabled: true
    max_batch_size: 1000
    max_batch_bytes: 1048576  # 1MB
    flush_interval_ms: 1
```

---

### Priority 3: Async Disk Pipeline

**Goal**: Reduce fsync overhead through batching
**Expected Impact**: 2-3x improvement for disk-bound workloads

#### Current Flow

```
proposal -> append entry -> fsync -> advance -> commit
                              ↑
                         Blocking!
```

#### Optimized Flow

```
proposal -> append entry ──┐
proposal -> append entry ──┼──► batch fsync -> advance all -> commit all
proposal -> append entry ──┘
```

#### Implementation

```rust
pub struct AsyncStorageWriter {
    pending_entries: Vec<Entry>,
    pending_hard_states: Vec<HardState>,
    flush_interval: Duration,
    sync_mode: SyncMode,
}

pub enum SyncMode {
    /// fsync after every entry (safest, slowest)
    Immediate,
    /// fsync after batch (balanced)
    Batched { interval_ms: u64 },
    /// fsync periodically (fastest, risk of data loss)
    Periodic { interval_ms: u64 },
}

impl AsyncStorageWriter {
    pub async fn append(&mut self, entries: Vec<Entry>) {
        self.pending_entries.extend(entries);

        if self.should_flush() {
            self.flush().await;
        }
    }

    async fn flush(&mut self) {
        // Write all pending entries
        for entry in &self.pending_entries {
            self.storage.append_entry(entry).await?;
        }

        // Single fsync for entire batch
        self.storage.sync().await?;

        self.pending_entries.clear();
    }
}
```

#### Configuration

```yaml
storage:
  sync_mode: "batched"  # immediate | batched | periodic
  sync_interval_ms: 5   # For batched/periodic modes
```

---

### Priority 4: gRPC Transport with Connection Pooling

**Goal**: Replace TCP with gRPC for better connection management
**Expected Impact**: Lower latency, better throughput under load

#### Protocol Definition

```protobuf
syntax = "proto3";
package locci.raft.v1;

service RaftService {
    // Streaming for high-throughput message passing
    rpc Stream(stream RaftMessage) returns (stream RaftMessage);

    // Unary for simple request/response
    rpc SendMessage(RaftMessage) returns (Empty);
}

message RaftMessage {
    uint64 from = 1;
    uint64 to = 2;
    bytes payload = 3;  // Serialized raft-rs Message
}
```

#### Connection Pool Design

```rust
pub struct PeerConnectionPool {
    peers: HashMap<u64, PeerConnection>,
    config: ConnectionPoolConfig,
}

pub struct ConnectionPoolConfig {
    pub max_connections_per_peer: usize,  // Default: 4
    pub connection_timeout: Duration,      // Default: 5s
    pub idle_timeout: Duration,            // Default: 60s
    pub max_message_size: usize,           // Default: 4MB
}

pub struct PeerConnection {
    client: RaftServiceClient<Channel>,
    stream: Option<Streaming<RaftMessage>>,
    last_used: Instant,
}
```

#### Dependencies

```toml
[dependencies]
tonic = "0.11"
prost = "0.12"

[build-dependencies]
tonic-build = "0.11"
```

---

### Priority 5: Linearizable Reads

**Goal**: Ensure reads see latest committed data
**Expected Impact**: Correctness improvement, slight read latency increase

#### Options

1. **Read Index** (Recommended)
   - Leader confirms it's still leader before read
   - Low overhead, maintains read performance

2. **Lease-Based Reads**
   - Leader holds lease, serves reads without confirmation
   - Fastest, requires clock synchronization

#### Read Index Implementation

```rust
impl RaftNode {
    pub async fn linearizable_read(&self, key: &str) -> Result<Option<Value>> {
        // 1. Get read index from Raft
        let read_index = self.get_read_index().await?;

        // 2. Wait for applied index to catch up
        self.wait_for_applied(read_index).await?;

        // 3. Read from storage
        self.storage.get(key).await
    }

    async fn get_read_index(&self) -> Result<u64> {
        let (tx, rx) = oneshot::channel();
        self.raw_node.write().read_index(tx);
        rx.await?
    }
}
```

---

### Priority 6: Snapshots & Log Compaction

**Goal**: Prevent unbounded log growth
**Expected Impact**: Stable memory usage, faster node recovery

#### Snapshot Trigger Conditions

```rust
pub struct SnapshotPolicy {
    /// Trigger snapshot after N entries
    pub entries_threshold: u64,  // Default: 10000

    /// Trigger snapshot after N bytes
    pub bytes_threshold: u64,    // Default: 64MB

    /// Minimum interval between snapshots
    pub min_interval: Duration,  // Default: 5 minutes
}
```

#### Snapshot Format

```rust
pub struct Snapshot {
    pub metadata: SnapshotMetadata,
    pub data: SnapshotData,
}

pub struct SnapshotMetadata {
    pub index: u64,
    pub term: u64,
    pub conf_state: ConfState,
}

pub struct SnapshotData {
    /// Serialized RocksDB checkpoint or key-value pairs
    pub payload: Vec<u8>,
    pub checksum: u32,
}
```

---

## Implementation Timeline

| Priority | Feature | Complexity | Impact |
|----------|---------|------------|--------|
| P1 | Event Loop Optimization | Medium | 100x |
| P2 | Proposal Batching | Medium | 2-5x |
| P3 | Async Disk Pipeline | High | 2-3x |
| P4 | gRPC Transport | High | 1.5-2x |
| P5 | Linearizable Reads | Medium | Correctness |
| P6 | Snapshots | High | Stability |

### Suggested Order

1. **Week 1-2**: Event Loop Optimization (P1)
   - Biggest impact, unblocks other optimizations

2. **Week 3-4**: Proposal Batching (P2)
   - Multiplies the benefit of P1

3. **Week 5-6**: gRPC Transport (P4)
   - Better foundation for remaining work

4. **Week 7-8**: Async Disk Pipeline (P3)
   - Final throughput optimization

5. **Week 9-10**: Snapshots & Compaction (P6)
   - Required for production stability

6. **Week 11-12**: Linearizable Reads (P5)
   - Correctness feature

---

## Benchmarking

### Running Benchmarks

```bash
# Install benchmark tool
cargo install rewrk

# Build release binary
cargo build --release

# Run comparison benchmark
./benches/bench-compare.sh

# Custom benchmark settings
DURATION=60s CONNECTIONS=200 ./benches/bench-compare.sh
```

### Target Metrics

| Metric | Current | Phase 3 Target |
|--------|---------|----------------|
| Write RPS (3-node) | 250 | 50,000+ |
| Write p99 (3-node) | 200ms | <5ms |
| Read RPS | 60,000 | 100,000+ |
| Read p99 | 2ms | <1ms |

### Benchmark Scripts

| Script | Description |
|--------|-------------|
| `bench-standalone.sh` | Baseline without Raft |
| `bench-raft-single.sh` | Single-node Raft |
| `bench-raft-cluster.sh` | 3-node Raft cluster |
| `bench-compare.sh` | Compare all modes |

---

## Configuration Reference (Phase 3)

```yaml
server:
  id: 1
  bind_addr: "127.0.0.1:8080"
  data_dir: "./data"

storage:
  backend: "rocksdb"
  sync_mode: "batched"        # immediate | batched | periodic
  sync_interval_ms: 5
  rocksdb:
    max_open_files: 10000
    write_buffer_size: 134217728  # 128MB
    max_write_buffer_number: 4
    target_file_size_base: 134217728

raft:
  tick_interval_ms: 50        # Reduced from 100ms
  heartbeat_tick: 2
  election_tick: 10
  max_size_per_msg: 4194304   # 4MB
  max_inflight_msgs: 256
  check_quorum: true
  pre_vote: true

  batching:
    enabled: true
    max_batch_size: 1000
    max_batch_bytes: 1048576
    flush_interval_ms: 1

  snapshots:
    enabled: true
    entries_threshold: 10000
    bytes_threshold: 67108864   # 64MB
    min_interval_secs: 300

cluster:
  transport: "grpc"           # tcp | grpc
  peers:
    - id: 1
      addr: "127.0.0.1:9001"
    - id: 2
      addr: "127.0.0.1:9002"
    - id: 3
      addr: "127.0.0.1:9003"

  connection_pool:
    max_connections_per_peer: 4
    connection_timeout_ms: 5000
    idle_timeout_ms: 60000

logging:
  level: "info"
  format: "json"
```

---

## References

- [raft-rs documentation](https://docs.rs/raft/latest/raft/)
- [etcd performance tuning](https://etcd.io/docs/v3.5/tuning/)
- [TiKV architecture](https://tikv.org/deep-dive/introduction/)
- [RocksDB tuning guide](https://github.com/facebook/rocksdb/wiki/RocksDB-Tuning-Guide)
