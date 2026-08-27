# Locci KV Benchmarks

Benchmark scripts for measuring Locci KV performance.

## Prerequisites

Install `rewrk` (Rust rewrite of wrk):

```bash
cargo install rewrk
```

Or build from source:
```bash
git clone https://github.com/lnx-search/rewrk
cd rewrk
cargo install --path .
```

## Scripts

| Script | Description |
|--------|-------------|
| `bench-standalone.sh` | Benchmark standalone mode (no Raft) |
| `bench-raft-single.sh` | Benchmark single-node Raft |
| `bench-raft-cluster.sh` | Benchmark 3-node Raft cluster |
| `bench-compare.sh` | Compare standalone vs Raft performance |

## Quick Start

```bash
# Build release binary first
cargo build --release

# Run standalone benchmark
./benches/bench-standalone.sh

# Run Raft cluster benchmark
./benches/bench-raft-cluster.sh

# Compare all modes
./benches/bench-compare.sh

# Custom settings
DURATION=60s CONNECTIONS=200 ./benches/bench-raft-cluster.sh
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DURATION` | `30s` | Test duration |
| `CONNECTIONS` | `100` | Concurrent connections |
| `THREADS` | `4` | Number of threads |

## Metrics

The benchmarks measure:
- **Requests/sec** - Throughput
- **Latency** - p50, p90, p99 percentiles
- **Transfer/sec** - Data throughput

## Test Scenarios

1. **Write-heavy**: 100% PUT requests
2. **Read-heavy**: 100% GET requests (after seeding)
3. **Mixed**: 80% reads, 20% writes
