#!/bin/bash
# Benchmark Locci KV in standalone mode (no Raft)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/locci-kv"
DATA_DIR="/tmp/locci-bench-standalone"
HOST="127.0.0.1:8080"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-100}"
THREADS="${THREADS:-4}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Locci KV Standalone Benchmark ===${NC}"
echo "Duration: $DURATION, Connections: $CONNECTIONS, Threads: $THREADS"
echo ""

# Cleanup
cleanup() {
    echo "Cleaning up..."
    pkill -f "locci-kv.*$DATA_DIR" 2>/dev/null || true
    rm -rf "$DATA_DIR"
}
trap cleanup EXIT

# Build if needed
if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    cd "$PROJECT_DIR" && cargo build --release
fi

# Start server
echo -e "${GREEN}Starting standalone server...${NC}"
rm -rf "$DATA_DIR"
mkdir -p "$DATA_DIR"
$BINARY --data-dir "$DATA_DIR" --bind-addr "$HOST" start > /tmp/locci-bench-standalone.log 2>&1 &
SERVER_PID=$!
sleep 2

# Check server is running
if ! curl -s "http://$HOST/health" > /dev/null; then
    echo "ERROR: Server failed to start"
    cat /tmp/locci-bench-standalone.log
    exit 1
fi
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
echo -e "${GREEN}[1/3] Write Benchmark (POST /kv/key)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$HOST/kv/bench-write-key" \
    --pct \
    -m POST \
    -H "Content-Type: application/json" \
    -b '{"value":"benchmark-value-standalone"}'
echo ""

# Read benchmark
echo -e "${GREEN}[2/3] Read Benchmark (GET /kv/key)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$HOST/kv/read-key-1" \
    --pct
echo ""

# Health check benchmark (baseline)
echo -e "${GREEN}[3/3] Health Check Benchmark (GET /health)${NC}"
rewrk -t "$THREADS" -c "$CONNECTIONS" -d "$DURATION" \
    -h "http://$HOST/health" \
    --pct
echo ""

echo -e "${BLUE}=== Standalone Benchmark Complete ===${NC}"
