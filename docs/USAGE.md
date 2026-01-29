# Locci KV Usage Guide

Comprehensive usage documentation for Locci KV, a distributed key-value store built on Raft consensus.

---

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Running Modes](#running-modes)
4. [Configuration](#configuration)
5. [API Reference](#api-reference)
6. [Raft Cluster Setup](#raft-cluster-setup)
7. [Operations](#operations)
8. [Troubleshooting](#troubleshooting)

---

## Installation

### Prerequisites

- Rust 1.70+ (for building from source)
- RocksDB dependencies (automatically handled by cargo)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/your-org/locci-kv.git
cd locci-kv

# Build release binary
cargo build --release

# Binary located at
./target/release/locci-kv
```

### Verify Installation

```bash
./target/release/locci-kv --version
./target/release/locci-kv --help
```

---

## Quick Start

### Standalone Mode (Single Node)

```bash
# Start with defaults
./target/release/locci-kv start

# Or explicitly standalone
./target/release/locci-kv standalone
```

### Test the API

```bash
# Health check
curl http://localhost:8080/health

# Store a value
curl -X POST http://localhost:8080/kv/greeting \
  -H "Content-Type: application/json" \
  -d '{"value": "Hello, Locci!"}'

# Retrieve the value
curl http://localhost:8080/kv/greeting

# List all keys
curl http://localhost:8080/keys

# Delete the key
curl -X DELETE http://localhost:8080/kv/greeting
```

---

## Running Modes

### Standalone Mode

Single-node operation without Raft consensus. Best for development and testing.

```bash
./target/release/locci-kv start
# or
./target/release/locci-kv standalone
```

**Characteristics:**
- Direct writes to RocksDB
- No replication
- Highest performance (~50K writes/sec)
- No fault tolerance

### Raft Mode (Distributed)

Multi-node operation with Raft consensus for fault tolerance.

```bash
./target/release/locci-kv --enable-raft --config node1.yaml start --bootstrap
```

**Characteristics:**
- Writes replicated across nodes
- Automatic leader election
- Survives minority node failures
- Lower write throughput (consensus overhead)

---

## Configuration

### Priority Order

Configuration is loaded with the following priority (highest first):

1. CLI flags
2. Environment variables
3. Config file (YAML)
4. Default values

### CLI Options

```
locci-kv [OPTIONS] <COMMAND>

Options:
  -c, --config <PATH>        Path to config file
      --id <ID>              Server ID
      --bind-addr <ADDR>     HTTP bind address (e.g., 127.0.0.1:8080)
      --data-dir <PATH>      Data directory for storage
      --log-level <LEVEL>    Log level: trace, debug, info, warn, error
      --enable-raft          Enable Raft consensus mode

Commands:
  start       Start the server
  standalone  Run in standalone mode (no Raft)
```

### Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `LOCCI_KV_CONFIG` | Config file path | `/etc/locci-kv/config.yaml` |
| `LOCCI_KV_SERVER_ID` | Server ID | `1` |
| `LOCCI_KV_BIND_ADDR` | HTTP bind address | `0.0.0.0:8080` |
| `LOCCI_KV_DATA_DIR` | Data directory | `/var/lib/locci-kv` |
| `LOCCI_LOG_LEVEL` | Log level | `info` |
| `LOCCI_ENABLE_RAFT` | Enable Raft mode | `true` |

### Configuration File

Full configuration file reference:

```yaml
# Server configuration
server:
  id: 1                           # Unique server ID (required for Raft)
  bind_addr: "127.0.0.1:8080"     # HTTP API bind address
  data_dir: "./data"              # Data storage directory

# Storage backend configuration
storage:
  backend: "rocksdb"              # Storage engine (only rocksdb supported)
  rocksdb:
    max_open_files: 1000          # Maximum open file handles
    write_buffer_size: 67108864   # Write buffer size (64MB)
    max_write_buffer_number: 3    # Number of write buffers
    target_file_size_base: 67108864  # SST file size (64MB)
    enable_statistics: true       # Enable RocksDB statistics

# Logging configuration
logging:
  level: "info"                   # Log level
  format: "json"                  # Output format: json or plain

# Raft consensus configuration (only used with --enable-raft)
raft:
  heartbeat_tick: 2               # Heartbeat interval in ticks
  election_tick: 10               # Election timeout in ticks
  max_size_per_msg: 1048576       # Max message size (1MB)
  max_inflight_msgs: 256          # Max in-flight messages
  check_quorum: true              # Enable quorum checking
  pre_vote: true                  # Enable pre-vote protocol

# Cluster configuration (only used with --enable-raft)
cluster:
  bootstrap: false                # Bootstrap new cluster (first node only)
  peers:                          # List of cluster peers
    - id: 1
      addr: "127.0.0.1:9001"      # Raft communication address
    - id: 2
      addr: "127.0.0.1:9002"
    - id: 3
      addr: "127.0.0.1:9003"
```

### Minimal Configuration Examples

**Standalone:**
```yaml
server:
  bind_addr: "0.0.0.0:8080"
  data_dir: "/var/lib/locci-kv"

storage:
  backend: "rocksdb"
  rocksdb:
    max_open_files: 1000
    write_buffer_size: 67108864
    max_write_buffer_number: 3
    target_file_size_base: 67108864
    enable_statistics: false

logging:
  level: "info"
  format: "json"
```

**3-Node Cluster (Node 1):**
```yaml
server:
  id: 1
  bind_addr: "192.168.1.10:8080"
  data_dir: "/var/lib/locci-kv"

storage:
  backend: "rocksdb"
  rocksdb:
    max_open_files: 1000
    write_buffer_size: 67108864
    max_write_buffer_number: 3
    target_file_size_base: 67108864
    enable_statistics: false

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
      addr: "192.168.1.10:9001"
    - id: 2
      addr: "192.168.1.11:9001"
    - id: 3
      addr: "192.168.1.12:9001"

logging:
  level: "info"
  format: "json"
```

---

## API Reference

### Base URL

```
http://<bind_addr>/
```

### Endpoints

#### Health Check

```http
GET /health
GET /
```

**Response:**
```json
{
  "message": "Locci KV is running"
}
```

---

#### Store a Key-Value Pair

```http
POST /kv/:key
Content-Type: application/json

{
  "value": "your-value-here"
}
```

**Parameters:**
- `:key` - The key name (URL path parameter)
- `value` - The value to store (JSON body)

**Response (Success - 200):**
```json
{
  "message": "Key 'mykey' stored successfully"
}
```

**Response (Not Leader - 503):**
```json
{
  "error": "Not leader. Current leader: 2"
}
```

**Response (Timeout - 408):**
```json
{
  "error": "Proposal timeout"
}
```

---

#### Retrieve a Value

```http
GET /kv/:key
```

**Response (Success - 200):**
```json
{
  "key": "mykey",
  "value": "myvalue"
}
```

**Response (Not Found - 404):**
```json
{
  "error": "Key not found: mykey"
}
```

---

#### Delete a Key

```http
DELETE /kv/:key
```

**Response (Success - 200):**
```json
{
  "message": "Key 'mykey' deleted successfully"
}
```

**Response (Not Found - 404):**
```json
{
  "error": "Key not found: mykey"
}
```

---

#### List All Keys

```http
GET /keys
```

**Response:**
```json
{
  "keys": ["key1", "key2", "key3"],
  "count": 3
}
```

---

#### Storage Statistics

```http
GET /stats
```

**Response:**
```json
{
  "num_keys": 1000,
  "disk_usage": 52428800,
  "mem_table_size": 16777216
}
```

---

#### Raft Status

```http
GET /raft/status
```

**Response (Raft Enabled):**
```json
{
  "enabled": true,
  "is_leader": true,
  "leader_id": 1
}
```

**Response (Standalone Mode):**
```json
{
  "enabled": false,
  "is_leader": false,
  "leader_id": null
}
```

---

### HTTP Status Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 400 | Bad Request (invalid input) |
| 404 | Key Not Found |
| 408 | Request Timeout (proposal timeout) |
| 500 | Internal Server Error |
| 503 | Service Unavailable (not leader) |

---

## Raft Cluster Setup

### 3-Node Cluster Example

#### Create Configuration Files

**node1.yaml:**
```yaml
server:
  id: 1
  bind_addr: "127.0.0.1:8081"
  data_dir: "./data/node1"

storage:
  backend: "rocksdb"
  rocksdb:
    max_open_files: 1000
    write_buffer_size: 67108864
    max_write_buffer_number: 3
    target_file_size_base: 67108864
    enable_statistics: false

raft:
  heartbeat_tick: 2
  election_tick: 10
  max_size_per_msg: 1048576
  max_inflight_msgs: 256
  check_quorum: true
  pre_vote: true

cluster:
  peers:
    - id: 1
      addr: "127.0.0.1:9001"
    - id: 2
      addr: "127.0.0.1:9002"
    - id: 3
      addr: "127.0.0.1:9003"

logging:
  level: "info"
  format: "json"
```

**node2.yaml:** (Change `id`, `bind_addr`, `data_dir`)
```yaml
server:
  id: 2
  bind_addr: "127.0.0.1:8082"
  data_dir: "./data/node2"
# ... rest same as node1
```

**node3.yaml:** (Change `id`, `bind_addr`, `data_dir`)
```yaml
server:
  id: 3
  bind_addr: "127.0.0.1:8083"
  data_dir: "./data/node3"
# ... rest same as node1
```

#### Start the Cluster

```bash
# Terminal 1: Bootstrap node 1 (first node initializes the cluster)
./target/release/locci-kv --enable-raft --config node1.yaml start --bootstrap

# Terminal 2: Start node 2 (joins existing cluster)
./target/release/locci-kv --enable-raft --config node2.yaml start

# Terminal 3: Start node 3 (joins existing cluster)
./target/release/locci-kv --enable-raft --config node3.yaml start
```

#### Verify Cluster Status

```bash
# Check each node's Raft status
curl http://127.0.0.1:8081/raft/status
curl http://127.0.0.1:8082/raft/status
curl http://127.0.0.1:8083/raft/status

# One node should show is_leader: true
```

#### Test Writes

```bash
# Find the leader (check /raft/status on each node)
# Write to the leader
curl -X POST http://127.0.0.1:8081/kv/test \
  -H "Content-Type: application/json" \
  -d '{"value": "replicated!"}'

# Read from any node (reads are local)
curl http://127.0.0.1:8082/kv/test
curl http://127.0.0.1:8083/kv/test
```

### Leader Failover

1. Kill the current leader node (Ctrl+C)
2. Wait 1-2 seconds for election timeout
3. Check `/raft/status` on remaining nodes
4. A new leader will be elected automatically
5. Writes continue on the new leader

```bash
# After killing leader, check remaining nodes
curl http://127.0.0.1:8082/raft/status
# {"enabled":true,"is_leader":true,"leader_id":2}
```

---

## Operations

### Backup

Data is stored in the configured `data_dir`. To backup:

```bash
# Stop the server gracefully
# Copy the data directory
cp -r ./data ./data-backup-$(date +%Y%m%d)
```

### Monitoring

Check server health:
```bash
curl http://localhost:8080/health
```

Check storage statistics:
```bash
curl http://localhost:8080/stats
```

Check Raft cluster status:
```bash
curl http://localhost:8080/raft/status
```

### Log Levels

Set log level via CLI or environment:

```bash
# Via CLI
./target/release/locci-kv --log-level debug start

# Via environment
export LOCCI_LOG_LEVEL=debug
./target/release/locci-kv start
```

Available levels: `trace`, `debug`, `info`, `warn`, `error`

---

## Troubleshooting

### Common Issues

#### "Not leader" Error

```json
{"error": "Not leader. Current leader: 2"}
```

**Solution:** Send write requests to the leader node. Check `/raft/status` to find the current leader.

```bash
# Find leader
for port in 8081 8082 8083; do
  echo "Node $port:"
  curl -s http://127.0.0.1:$port/raft/status | jq .
done
```

---

#### Proposal Timeout

```json
{"error": "Proposal timeout"}
```

**Causes:**
- Network issues between Raft nodes
- Cluster doesn't have quorum (majority of nodes down)
- Disk I/O slow

**Solutions:**
- Check network connectivity between nodes
- Ensure majority of nodes are running
- Check disk performance

---

#### Node Won't Join Cluster

**Causes:**
- Wrong peer addresses in config
- Firewall blocking Raft ports
- Node already has data from different cluster

**Solutions:**
- Verify peer addresses match across all configs
- Open Raft ports (default 9001-9003)
- Clear data directory for fresh start

```bash
rm -rf ./data/node1
```

---

#### RocksDB Errors

```
Storage error: Corruption: ...
```

**Solution:**
1. Stop the server
2. Backup corrupted data directory
3. Try RocksDB repair or start fresh

---

### Debug Mode

Run with debug logging for more information:

```bash
./target/release/locci-kv --log-level debug --enable-raft --config node1.yaml start
```

### Performance Tuning

See [PHASE_3_PERFORMANCE.md](PHASE_3_PERFORMANCE.md) for detailed performance optimization guidance.

---

## Examples

### Shell Script: Bulk Insert

```bash
#!/bin/bash
HOST="${1:-localhost:8080}"

for i in $(seq 1 1000); do
  curl -s -X POST "http://$HOST/kv/key-$i" \
    -H "Content-Type: application/json" \
    -d "{\"value\": \"value-$i\"}" > /dev/null
done

echo "Inserted 1000 keys"
curl "http://$HOST/keys" | jq '.count'
```

### Python Client Example

```python
import requests

class LocciKVClient:
    def __init__(self, host="localhost", port=8080):
        self.base_url = f"http://{host}:{port}"

    def put(self, key, value):
        resp = requests.post(
            f"{self.base_url}/kv/{key}",
            json={"value": value}
        )
        resp.raise_for_status()
        return resp.json()

    def get(self, key):
        resp = requests.get(f"{self.base_url}/kv/{key}")
        resp.raise_for_status()
        return resp.json()["value"]

    def delete(self, key):
        resp = requests.delete(f"{self.base_url}/kv/{key}")
        resp.raise_for_status()
        return resp.json()

    def keys(self):
        resp = requests.get(f"{self.base_url}/keys")
        resp.raise_for_status()
        return resp.json()["keys"]

    def raft_status(self):
        resp = requests.get(f"{self.base_url}/raft/status")
        resp.raise_for_status()
        return resp.json()

# Usage
client = LocciKVClient()
client.put("name", "Locci")
print(client.get("name"))  # "Locci"
print(client.keys())       # ["name"]
```

---

## Next Steps

- [Phase 3 Performance Guide](PHASE_3_PERFORMANCE.md) - Performance optimization details
- [Phase 2 Implementation](PHASE_2.md) - Raft implementation details
- [README](../README.md) - Project overview and roadmap
