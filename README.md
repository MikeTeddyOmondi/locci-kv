# Locci KV

A distributed key-value store built on Raft consensus with RocksDB as the storage backend.

## Features

### Phase 1 (Standalone)
- ✅ RocksDB storage backend
- ✅ HTTP REST API
- ✅ Configuration via CLI, ENV vars, and YAML
- ✅ Comprehensive logging
- ✅ CRUD operations (Create, Read, Update, Delete)
- ✅ Key listing
- ✅ Storage statistics

### Phase 2 (Raft Consensus)
- ✅ Raft consensus via raft-rs
- ✅ Leader election with pre-vote
- ✅ Log replication across nodes
- ✅ Leader failover
- ✅ TCP transport for Raft messages
- ✅ Proposal system with timeout handling
- ✅ `/raft/status` endpoint

### Phase 3 (Performance) - In Progress
- ⬜ Event loop optimization
- ⬜ Proposal batching
- ⬜ gRPC transport
- ⬜ Snapshots & log compaction

## Performance

Current benchmarks (3-node cluster, 50 connections):

| Mode | Write RPS | Read RPS | Write p99 |
|------|-----------|----------|-----------|
| Standalone | ~52K | ~56K | 3ms |
| Raft Single | ~500 | ~62K | 102ms |
| Raft Cluster (3) | ~250 | ~61K | 202ms |

**Phase 3 targets**: 50K+ writes/sec with <5ms p99 latency.

Run benchmarks:
```bash
cargo install rewrk
./benches/bench-compare.sh
```

See [docs/PHASE_3_PERFORMANCE.md](docs/PHASE_3_PERFORMANCE.md) for optimization details.

## Quick Start

### Build

```bash
cargo build --release
```

### Run standalone (no Raft)

```bash
./target/release/locci-kv start
```

### Run with Raft (3-node cluster)

```bash
# Terminal 1 - Bootstrap node 1
./target/release/locci-kv --enable-raft --config node1.yaml start --bootstrap

# Terminal 2 - Node 2
./target/release/locci-kv --enable-raft --config node2.yaml start

# Terminal 3 - Node 3
./target/release/locci-kv --enable-raft --config node3.yaml start
```

### Run with custom config

```bash
./target/release/locci-kv --config custom-config.yaml start
```

### Run with CLI overrides

```bash
./target/release/locci-kv \
  --id 1 \
  --bind-addr 0.0.0.0:8080 \
  --data-dir /var/lib/locci-kv \
  start
```

### Run with environment variables

```bash
export LOCCI_KV_CONFIG=config.yaml
export LOCCI_KV_SERVER_ID=1
export LOCCI_KV_BIND_ADDR=127.0.0.1:8080
export LOCCI_KV_DATA_DIR=./data

./target/release/locci-kv start
```

## API Usage

### Health Check

```bash
curl http://localhost:8080/health
```

### Put a key-value pair

```bash
curl -X POST http://localhost:8080/kv/mykey \
  -H "Content-Type: application/json" \
  -d '{"value": "myvalue"}'
```

### Get a value

```bash
curl http://localhost:8080/kv/mykey
```

### Delete a key

```bash
curl -X DELETE http://localhost:8080/kv/mykey
```

### List all keys

```bash
curl http://localhost:8080/keys
```

### Get storage statistics

```bash
curl http://localhost:8080/stats
```

### Get Raft status (Phase 2)

```bash
curl http://localhost:8081/raft/status
# {"enabled":true,"is_leader":true,"leader_id":1}
```

## Configuration

Configuration priority (highest to lowest):
1. CLI flags
2. Environment variables
3. Config file (YAML)
4. Default values

Example `config.yaml`:

```yaml
server:
  id: 1
  bind_addr: "127.0.0.1:8080"
  data_dir: "./data"

storage:
  backend: "rocksdb"
  rocksdb:
    max_open_files: 1000
    write_buffer_size: 67108864
    max_write_buffer_number: 3
    target_file_size_base: 67108864
    enable_statistics: true

logging:
  level: "info"
  format: "json"

# Phase 2: Raft configuration
raft:
  heartbeat_tick: 2
  election_tick: 10
  max_size_per_msg: 1048576
  max_inflight_msgs: 256
  check_quorum: true
  pre_vote: true

cluster:
  bootstrap: false
  peers:
    - id: 1
      addr: "127.0.0.1:9001"
    - id: 2
      addr: "127.0.0.1:9002"
    - id: 3
      addr: "127.0.0.1:9003"
```

## Development

### Run in development mode

```bash
cargo run -- --log-level debug start
```

### Run tests

```bash
cargo test
```

### Format code

```bash
cargo fmt
```

### Lint code

```bash
cargo clippy
```

## Architecture

### Phase 1 (Standalone)
```
┌─────────────────────────────────────────┐
│          HTTP API (Axum)                │
├─────────────────────────────────────────┤
│         Storage Interface               │
├─────────────────────────────────────────┤
│      RocksDB Storage Backend            │
└─────────────────────────────────────────┘
```

### Phase 2 (Raft Cluster)
```
┌─────────────────────────────────────────┐
│          HTTP API (Axum)                │
├─────────────────────────────────────────┤
│     Raft Layer (Leader Election,        │
│     Log Replication, Consensus)         │
├─────────────────────────────────────────┤
│         Storage Interface               │
├─────────────────────────────────────────┤
│      RocksDB Storage Backend            │
├─────────────────────────────────────────┤
│     TCP Transport (Raft Messages)       │
└─────────────────────────────────────────┘
```

## Roadmap

- [x] Phase 1: Single Node MVP
  - [x] CLI configuration with clap
  - [x] Config file (YAML) support
  - [x] RocksDB storage
  - [x] HTTP REST API
  - [x] Basic CRUD operations

- [x] Phase 2: Raft Integration
  - [x] Integrate raft-rs
  - [x] Consensus for writes
  - [x] Leader election with pre-vote
  - [x] TCP transport for Raft messages
  - [x] Log replication
  - [x] Leader failover
  - [x] Raft status endpoint

- [ ] Phase 3: Performance & Production
  - [ ] Event loop optimization (decouple ticks from proposals)
  - [ ] Proposal batching (accumulate writes)
  - [ ] Async disk pipeline (fsync batching)
  - [ ] gRPC transport with connection pooling
  - [ ] Snapshots & log compaction
  - [ ] Linearizable reads (lease-based)

- [ ] Phase 4: Operations & Scale
  - [ ] Prometheus metrics
  - [ ] Dynamic membership changes
  - [ ] TLS/mTLS support
  - [ ] Backup & restore
  - [ ] Multi-region support

## License

[Apache-2.0](./LICENSE)
