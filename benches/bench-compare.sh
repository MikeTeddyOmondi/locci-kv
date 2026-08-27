#!/bin/bash
# Compare Locci KV performance: Standalone vs Raft Single vs Raft Cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/locci-kv"
DURATION="${DURATION:-15s}"
CONNECTIONS="${CONNECTIONS:-50}"
THREADS="${THREADS:-4}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Locci KV Performance Comparison                    ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Duration: $DURATION, Connections: $CONNECTIONS, Threads: $THREADS"
echo ""

# Build if needed
if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    cd "$PROJECT_DIR" && cargo build --release
fi

# Temp files for results
STANDALONE_WRITE="/tmp/locci-bench-standalone-write.txt"
STANDALONE_READ="/tmp/locci-bench-standalone-read.txt"
RAFT_SINGLE_WRITE="/tmp/locci-bench-raft-single-write.txt"
RAFT_SINGLE_READ="/tmp/locci-bench-raft-single-read.txt"
RAFT_CLUSTER_WRITE="/tmp/locci-bench-raft-cluster-write.txt"
RAFT_CLUSTER_READ="/tmp/locci-bench-raft-cluster-read.txt"

cleanup_all() {
    echo "Cleaning up..."
    # Kill any remaining locci-kv processes from this benchmark
    kill $STANDALONE_PID 2>/dev/null || true
    kill $RAFT_SINGLE_PID 2>/dev/null || true
    pkill -f "locci-kv.*locci-bench-cluster" 2>/dev/null || true
    sleep 1
    rm -rf /tmp/locci-bench-standalone /tmp/locci-bench-raft-single /tmp/locci-bench-raft-cluster
    rm -f /tmp/locci-bench-*.yaml /tmp/locci-bench-*.log /tmp/locci-bench-*.txt
}
trap cleanup_all EXIT

# Initialize PIDs
STANDALONE_PID=""
RAFT_SINGLE_PID=""

#######################################
# 1. STANDALONE MODE
#######################################
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[1/3] STANDALONE MODE (No Raft)${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

DATA_DIR="/tmp/locci-bench-standalone"
rm -rf "$DATA_DIR" && mkdir -p "$DATA_DIR"

$BINARY --bind-addr 127.0.0.1:8090 --data-dir "$DATA_DIR" start > /tmp/locci-bench-standalone.log 2>&1 &
STANDALONE_PID=$!
disown $STANDALONE_PID
sleep 2

# Seed data
curl -s -X POST "http://127.0.0.1:8090/kv/read-key" \
    -H "Content-Type: application/json" \
    -d '{"value":"benchmark-value"}' > /dev/null

echo -e "${GREEN}Write Benchmark:${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://127.0.0.1:8090/kv/bench-key" \
    --pct \
    -m POST \
    -H "Content-Type: application/json" \
    -b '{"value":"benchmark-value"}' 2>&1 | tee "$STANDALONE_WRITE"

echo ""
echo -e "${GREEN}Read Benchmark:${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://127.0.0.1:8090/kv/read-key" \
    --pct 2>&1 | tee "$STANDALONE_READ"

kill $STANDALONE_PID 2>/dev/null || true
sleep 1
echo ""

#######################################
# 2. RAFT SINGLE NODE
#######################################
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[2/3] RAFT SINGLE NODE${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

DATA_DIR="/tmp/locci-bench-raft-single"
rm -rf "$DATA_DIR" && mkdir -p "$DATA_DIR"

cat > /tmp/locci-bench-raft-single.yaml << EOF
server:
  id: 1
  bind_addr: "127.0.0.1:8091"
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
  bootstrap: false
  peers:
    - id: 1
      addr: "127.0.0.1:9091"

logging:
  level: "warn"
  format: "json"
EOF

$BINARY --enable-raft --config /tmp/locci-bench-raft-single.yaml start --bootstrap > /tmp/locci-bench-raft-single.log 2>&1 &
RAFT_SINGLE_PID=$!
disown $RAFT_SINGLE_PID
sleep 3

# Wait for leader
for i in $(seq 1 10); do
    STATUS=$(curl -s "http://127.0.0.1:8091/raft/status" 2>/dev/null || echo "{}")
    IS_LEADER=$(echo "$STATUS" | grep -o '"is_leader":true' || true)
    if [ -n "$IS_LEADER" ]; then
        break
    fi
    sleep 1
done

# Seed data
curl -s -X POST "http://127.0.0.1:8091/kv/read-key" \
    -H "Content-Type: application/json" \
    -d '{"value":"benchmark-value"}' > /dev/null

echo -e "${GREEN}Write Benchmark:${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://127.0.0.1:8091/kv/bench-key" \
    --pct \
    -m POST \
    -H "Content-Type: application/json" \
    -b '{"value":"benchmark-value"}' 2>&1 | tee "$RAFT_SINGLE_WRITE"

echo ""
echo -e "${GREEN}Read Benchmark:${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://127.0.0.1:8091/kv/read-key" \
    --pct 2>&1 | tee "$RAFT_SINGLE_READ"

kill $RAFT_SINGLE_PID 2>/dev/null || true
sleep 1
echo ""

#######################################
# 3. RAFT 3-NODE CLUSTER
#######################################
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[3/3] RAFT 3-NODE CLUSTER${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

DATA_DIR="/tmp/locci-bench-raft-cluster"
rm -rf "$DATA_DIR" && mkdir -p "$DATA_DIR/node1" "$DATA_DIR/node2" "$DATA_DIR/node3"

for i in 1 2 3; do
    HTTP_PORT=$((8091 + $i))
    RAFT_PORT=$((9091 + $i))
    cat > /tmp/locci-bench-cluster-node$i.yaml << EOF
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
      addr: "127.0.0.1:9092"
    - id: 2
      addr: "127.0.0.1:9093"
    - id: 3
      addr: "127.0.0.1:9094"

logging:
  level: "warn"
  format: "json"
EOF
done

# Start cluster
$BINARY --enable-raft --config /tmp/locci-bench-cluster-node1.yaml start --bootstrap > /tmp/locci-bench-cluster-node1.log 2>&1 &
disown
sleep 2
$BINARY --enable-raft --config /tmp/locci-bench-cluster-node2.yaml start > /tmp/locci-bench-cluster-node2.log 2>&1 &
disown
$BINARY --enable-raft --config /tmp/locci-bench-cluster-node3.yaml start > /tmp/locci-bench-cluster-node3.log 2>&1 &
disown
sleep 4

# Find leader
LEADER_PORT=""
for port in 8092 8093 8094; do
    STATUS=$(curl -s "http://127.0.0.1:$port/raft/status" 2>/dev/null || echo "{}")
    IS_LEADER=$(echo "$STATUS" | grep -o '"is_leader":true' || true)
    if [ -n "$IS_LEADER" ]; then
        LEADER_PORT=$port
        break
    fi
done

if [ -z "$LEADER_PORT" ]; then
    echo "ERROR: No leader found"
    exit 1
fi

LEADER_HOST="127.0.0.1:$LEADER_PORT"
echo -e "Leader found at ${YELLOW}$LEADER_HOST${NC}"

# Seed data
curl -s -X POST "http://$LEADER_HOST/kv/read-key" \
    -H "Content-Type: application/json" \
    -d '{"value":"benchmark-value"}' > /dev/null

echo -e "${GREEN}Write Benchmark (to Leader):${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$LEADER_HOST/kv/bench-key" \
    --pct \
    -m POST \
    -H "Content-Type: application/json" \
    -b '{"value":"benchmark-value"}' 2>&1 | tee "$RAFT_CLUSTER_WRITE"

echo ""
echo -e "${GREEN}Read Benchmark (from Leader):${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$LEADER_HOST/kv/read-key" \
    --pct 2>&1 | tee "$RAFT_CLUSTER_READ"

pkill -f "locci-kv.*locci-bench-cluster" 2>/dev/null || true
sleep 1
echo ""

#######################################
# SUMMARY
#######################################
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                      SUMMARY                                 ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

extract_rps() {
    grep -o 'Req/Sec:[ ]*[0-9.]*' "$1" 2>/dev/null | head -1 | awk '{print $2}' || echo "N/A"
}

extract_p99() {
    grep '99%' "$1" 2>/dev/null | head -1 | awk '{print $3}' || echo "N/A"
}

echo "┌─────────────────────┬──────────────────┬──────────────────┐"
echo "│ Mode                │ Write RPS        │ Read RPS         │"
echo "├─────────────────────┼──────────────────┼──────────────────┤"
printf "│ %-19s │ %16s │ %16s │\n" "Standalone" "$(extract_rps $STANDALONE_WRITE)" "$(extract_rps $STANDALONE_READ)"
printf "│ %-19s │ %16s │ %16s │\n" "Raft Single" "$(extract_rps $RAFT_SINGLE_WRITE)" "$(extract_rps $RAFT_SINGLE_READ)"
printf "│ %-19s │ %16s │ %16s │\n" "Raft Cluster (3)" "$(extract_rps $RAFT_CLUSTER_WRITE)" "$(extract_rps $RAFT_CLUSTER_READ)"
echo "└─────────────────────┴──────────────────┴──────────────────┘"
echo ""

echo -e "${BLUE}=== Comparison Complete ===${NC}"
