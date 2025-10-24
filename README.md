# Locci KV

A distributed key-value store built on Raft consensus with RocksDB as the storage backend.

## Features (Phase 1 MVP)

- ✅ RocksDB storage backend
- ✅ HTTP REST API
- ✅ Configuration via CLI, ENV vars, and YAML
- ✅ Comprehensive logging
- ✅ CRUD operations (Create, Read, Update, Delete)
- ✅ Key listing
- ✅ Storage statistics

## Quick Start

### Build

```bash
cargo build --release
```

### Run with defaults

```bash
./target/release/locci-kv start
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
export LOCCI_CONFIG=config.yaml
export LOCCI_SERVER_ID=1
export LOCCI_BIND_ADDR=127.0.0.1:8080
export LOCCI_DATA_DIR=./data

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

```
┌─────────────────────────────────────────┐
│          HTTP API (Axum)                │
├─────────────────────────────────────────┤
│         Storage Interface               │
├─────────────────────────────────────────┤
│      RocksDB Storage Backend            │
└─────────────────────────────────────────┘
```

## Roadmap

- [x] Phase 1: Single Node MVP
  - [x] CLI configuration with clap
  - [x] Config file (YAML) support
  - [x] RocksDB storage
  - [x] HTTP REST API
  - [x] Basic CRUD operations

- [ ] Phase 2: Raft Integration
  - [ ] Integrate raft-rs
  - [ ] Consensus for writes
  - [ ] Leader election

- [ ] Phase 3: Multi-Node Cluster
  - [ ] Cluster configuration
  - [ ] Node join/leave
  - [ ] Replication

- [ ] Phase 4: Production Features
  - [ ] Connection pooling
  - [ ] Metrics & monitoring
  - [ ] Backup & restore

## License

Apache-2.0
