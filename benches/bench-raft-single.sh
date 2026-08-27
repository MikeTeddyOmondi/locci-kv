#!/bin/bash
# Benchmark Locci KV with single-node Raft

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/locci-kv"
DATA_DIR="/tmp/locci-bench-raft-single"
HOST="127.0.0.1:8080"
RAFT_ADDR="127.0.0.1:9001"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-100}"
THREADS="${THREADS:-4}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Locci KV Single-Node Raft Benchmark ===${NC}"
echo "Duration: $DURATION, Connections: $CONNECTIONS, Threads: $THREADS"
echo ""

# Cleanup
cleanup() {
    echo "Cleaning up..."
    pkill -f "locci-kv.*$DATA_DIR" 2>/dev/null || true
    rm -rf "$DATA_DIR"
    rm -f /tmp/locci-bench-raft-single-config.yaml
}
trap cleanup EXIT

# Build if needed
if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    cd "$PROJECT_DIR" && cargo build --release
fi

# Create config
rm -rf "$DATA_DIR"
mkdir -p "$DATA_DIR"

cat > /tmp/locci-bench-raft-single-config.yaml << EOF
server:
  id: 1
  bind_addr: "$HOST"
  data_dir: "$DATA_DIR"

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
  bootstrap: true
  peers:
    - id: 1
      addr: "$RAFT_ADDR"

logging:
  level: "warn"
  format: "json"
EOF

# Start server
echo -e "${GREEN}Starting single-node Raft server...${NC}"
$BINARY --enable-raft --config /tmp/locci-bench-raft-single-config.yaml start --bootstrap > /tmp/locci-bench-raft-single.log 2>&1 &
SERVER_PID=$!
sleep 3

# Check server is running and is leader
if ! curl -s "http://$HOST/health" > /dev/null; then
    echo "ERROR: Server failed to start"
    cat /tmp/locci-bench-raft-single.log
    exit 1
fi

RAFT_STATUS=$(curl -s "http://$HOST/raft/status")
echo "Raft status: $RAFT_STATUS"
echo "Server running on $HOST (PID: $SERVER_PID)"
echo ""

# Seed some data for read tests
echo -e "${GREEN}Seeding test data...${NC}"
for i in $(seq 1 100); do
    curl -s -X POST "http://$HOST/kv/read-key-$i" \
        -H "Content-Type: application/json" \
        -d "{\"value\":\"read-value-$i\"}" > /dev/null
done
echo "Seeded 100 keys"
echo ""

# Write benchmark
echo -e "${GREEN}[1/3] Write Benchmark (POST /kv/key via Raft)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$HOST/kv/bench-write-key" \
    --pct \
    -m POST \
    -H "Content-Type: application/json" \
    -b '{"value":"benchmark-value-raft-single"}'
echo ""

# Read benchmark
echo -e "${GREEN}[2/3] Read Benchmark (GET /kv/key)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$HOST/kv/read-key-1" \
    --pct
echo ""

# Raft status benchmark
echo -e "${GREEN}[3/3] Raft Status Benchmark (GET /raft/status)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$HOST/raft/status" \
    --pct
echo ""

echo -e "${BLUE}=== Single-Node Raft Benchmark Complete ===${NC}"
