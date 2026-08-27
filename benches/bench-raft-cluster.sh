#!/bin/bash
# Benchmark Locci KV with 3-node Raft cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/locci-kv"
DATA_DIR="/tmp/locci-bench-raft-cluster"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-100}"
THREADS="${THREADS:-4}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}=== Locci KV 3-Node Raft Cluster Benchmark ===${NC}"
echo "Duration: $DURATION, Connections: $CONNECTIONS, Threads: $THREADS"
echo ""

# Cleanup
cleanup() {
    echo "Cleaning up..."
    pkill -f "locci-kv.*locci-bench-raft-cluster" 2>/dev/null || true
    rm -rf "$DATA_DIR"
    rm -f /tmp/locci-bench-node*.yaml
}
trap cleanup EXIT

# Build if needed
if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    cd "$PROJECT_DIR" && cargo build --release
fi

# Create configs for 3 nodes
rm -rf "$DATA_DIR"
mkdir -p "$DATA_DIR/node1" "$DATA_DIR/node2" "$DATA_DIR/node3"

for i in 1 2 3; do
    HTTP_PORT=$((8080 + $i))
    cat > /tmp/locci-bench-node$i.yaml << EOF
server:
  id: $i
  bind_addr: "127.0.0.1:$HTTP_PORT"
  data_dir: "$DATA_DIR/node$i"

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
      addr: "127.0.0.1:9001"
    - id: 2
      addr: "127.0.0.1:9002"
    - id: 3
      addr: "127.0.0.1:9003"

logging:
  level: "warn"
  format: "json"
EOF
done

# Start 3-node cluster
echo -e "${GREEN}Starting 3-node Raft cluster...${NC}"

# Node 1 (bootstrap)
$BINARY --enable-raft --config /tmp/locci-bench-node1.yaml start --bootstrap > /tmp/locci-bench-node1.log 2>&1 &
sleep 2

# Node 2 & 3
$BINARY --enable-raft --config /tmp/locci-bench-node2.yaml start > /tmp/locci-bench-node2.log 2>&1 &
$BINARY --enable-raft --config /tmp/locci-bench-node3.yaml start > /tmp/locci-bench-node3.log 2>&1 &
sleep 3

# Find leader
LEADER_PORT=""
for port in 8081 8082 8083; do
    STATUS=$(curl -s "http://127.0.0.1:$port/raft/status" 2>/dev/null || echo "{}")
    IS_LEADER=$(echo "$STATUS" | grep -o '"is_leader":true' || true)
    if [ -n "$IS_LEADER" ]; then
        LEADER_PORT=$port
        break
    fi
done

if [ -z "$LEADER_PORT" ]; then
    echo "ERROR: No leader found"
    echo "Node 1 status: $(curl -s http://127.0.0.1:8081/raft/status 2>/dev/null || echo 'unreachable')"
    echo "Node 2 status: $(curl -s http://127.0.0.1:8082/raft/status 2>/dev/null || echo 'unreachable')"
    echo "Node 3 status: $(curl -s http://127.0.0.1:8083/raft/status 2>/dev/null || echo 'unreachable')"
    exit 1
fi

LEADER_HOST="127.0.0.1:$LEADER_PORT"
echo -e "${YELLOW}Leader found at $LEADER_HOST${NC}"
echo "Node 1: $(curl -s http://127.0.0.1:8081/raft/status)"
echo "Node 2: $(curl -s http://127.0.0.1:8082/raft/status)"
echo "Node 3: $(curl -s http://127.0.0.1:8083/raft/status)"
echo ""

# Seed some data for read tests
echo -e "${GREEN}Seeding test data...${NC}"
for i in $(seq 1 100); do
    curl -s -X POST "http://$LEADER_HOST/kv/read-key-$i" \
        -H "Content-Type: application/json" \
        -d "{\"value\":\"read-value-$i\"}" > /dev/null
done
echo "Seeded 100 keys"
echo ""

# Write benchmark (to leader)
echo -e "${GREEN}[1/4] Write Benchmark - Leader (POST /kv/key via Raft consensus)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$LEADER_HOST/kv/bench-write-key" \
    --pct \
    -m POST \
    -H "Content-Type: application/json" \
    -b '{"value":"benchmark-value-raft-cluster"}'
echo ""

# Read benchmark (from leader)
echo -e "${GREEN}[2/4] Read Benchmark - Leader (GET /kv/key)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$LEADER_HOST/kv/read-key-1" \
    --pct
echo ""

# Read benchmark (from follower)
FOLLOWER_PORT=""
for port in 8081 8082 8083; do
    if [ "$port" != "$LEADER_PORT" ]; then
        FOLLOWER_PORT=$port
        break
    fi
done
FOLLOWER_HOST="127.0.0.1:$FOLLOWER_PORT"

echo -e "${GREEN}[3/4] Read Benchmark - Follower at $FOLLOWER_HOST (GET /kv/key)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$FOLLOWER_HOST/kv/read-key-1" \
    --pct
echo ""

# Raft status benchmark
echo -e "${GREEN}[4/4] Raft Status Benchmark (GET /raft/status)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$LEADER_HOST/raft/status" \
    --pct
echo ""

echo -e "${BLUE}=== 3-Node Raft Cluster Benchmark Complete ===${NC}"
