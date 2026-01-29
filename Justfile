# Locci KV - Project Commands
# Usage: just <recipe>

# Default recipe - show available commands
default:
    @just --list

# ─────────────────────────────────────────────────────────────────────────────
# Build Commands
# ─────────────────────────────────────────────────────────────────────────────

# Build debug binary
build:
    cargo build

# Build release binary
build-release:
    cargo build --release

# Build and strip release binary
build-prod:
    cargo build --release
    strip target/release/locci-kv

# Clean build artifacts
clean:
    cargo clean

# ─────────────────────────────────────────────────────────────────────────────
# Development Commands
# ─────────────────────────────────────────────────────────────────────────────

# Run tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Format code
fmt:
    cargo fmt

# Check formatting
fmt-check:
    cargo fmt -- --check

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Run all checks (format, lint, test)
check: fmt-check lint test

# ─────────────────────────────────────────────────────────────────────────────
# Run Commands
# ─────────────────────────────────────────────────────────────────────────────

# Run in standalone mode (development)
run:
    cargo run -- start

# Run in standalone mode with debug logging
run-debug:
    cargo run -- --log-level debug start

# Run release binary in standalone mode
run-release:
    ./target/release/locci-kv start

# ─────────────────────────────────────────────────────────────────────────────
# Raft Cluster Commands (Local Development)
# ─────────────────────────────────────────────────────────────────────────────

# Start node 1 (bootstrap)
node1:
    ./target/release/locci-kv --enable-raft --config configs/node1.yaml start --bootstrap

# Start node 2
node2:
    ./target/release/locci-kv --enable-raft --config configs/node2.yaml start

# Start node 3
node3:
    ./target/release/locci-kv --enable-raft --config configs/node3.yaml start

# Create local config files for 3-node cluster
cluster-init:
    mkdir -p configs data/node1 data/node2 data/node3
    @echo "Creating node configs..."
    @just _create-node-config 1 8081 9001
    @just _create-node-config 2 8082 9002
    @just _create-node-config 3 8083 9003
    @echo "Configs created in ./configs/"

# Clean cluster data
cluster-clean:
    rm -rf data/node1 data/node2 data/node3

# Helper to create node config
_create-node-config id http_port raft_port:
    #!/usr/bin/env bash
    cat > configs/node{{id}}.yaml << 'EOF'
    server:
      id: {{id}}
      bind_addr: "127.0.0.1:{{http_port}}"
      data_dir: "./data/node{{id}}"
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
      level: "info"
      format: "json"
    EOF

# ─────────────────────────────────────────────────────────────────────────────
# Docker Commands
# ─────────────────────────────────────────────────────────────────────────────

# Build Docker image
docker-build:
    docker build -t locci-kv:latest -f docker/Dockerfile .

# Build Docker image with specific tag
docker-build-tag tag:
    docker build -t locci-kv:{{tag}} -f docker/Dockerfile .

# Run standalone container
docker-run:
    docker run -d --name locci-kv -p 8080:8080 locci-kv:latest

# Stop and remove standalone container
docker-stop:
    docker stop locci-kv && docker rm locci-kv

# Start 3-node cluster with docker-compose
docker-cluster-up:
    docker compose -f docker/docker-compose.yml --profile cluster up -d

# Stop cluster
docker-cluster-down:
    docker compose -f docker/docker-compose.yml --profile cluster down

# View cluster logs
docker-cluster-logs:
    docker compose -f docker/docker-compose.yml --profile cluster logs -f

# Start standalone with docker-compose
docker-standalone-up:
    docker compose -f docker/docker-compose.yml --profile standalone up -d

# Stop standalone
docker-standalone-down:
    docker compose -f docker/docker-compose.yml --profile standalone down

# Clean all docker resources
docker-clean:
    docker compose -f docker/docker-compose.yml down -v --remove-orphans
    docker rmi locci-kv:latest 2>/dev/null || true

# ─────────────────────────────────────────────────────────────────────────────
# Benchmark Commands
# ─────────────────────────────────────────────────────────────────────────────

# Run standalone benchmark
bench-standalone:
    ./benches/bench-standalone.sh

# Run single-node Raft benchmark
bench-raft-single:
    ./benches/bench-raft-single.sh

# Run 3-node cluster benchmark
bench-raft-cluster:
    ./benches/bench-raft-cluster.sh

# Run comparison benchmark (all modes)
bench-compare:
    ./benches/bench-compare.sh

# Run quick benchmark comparison
bench-quick:
    DURATION=10s CONNECTIONS=50 ./benches/bench-compare.sh

# ─────────────────────────────────────────────────────────────────────────────
# Documentation Commands
# ─────────────────────────────────────────────────────────────────────────────

# Generate and open docs
docs:
    cargo doc --open

# Generate docs without opening
docs-build:
    cargo doc --no-deps

# ─────────────────────────────────────────────────────────────────────────────
# Release Commands
# ─────────────────────────────────────────────────────────────────────────────

# Create release build for current platform
release:
    just build-prod

# Show version
version:
    @cargo pkgid | cut -d# -f2

# Tag a new version (usage: just tag v0.1.0)
tag version:
    git tag -a {{version}} -m "Release {{version}}"
    @echo "Tagged {{version}}. Push with: git push origin {{version}}"
