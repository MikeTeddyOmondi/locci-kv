# Locci KV Docker

Docker configuration for Locci KV distributed key-value store.

## Quick Start

### Build Image

```bash
# From project root
docker build -t locci/kv:latest -f docker/Dockerfile .

# Or using just
just docker-build
```

### Run Standalone

```bash
# Direct docker run
docker run -d --name locci-kv \
  -p 8080:8080 \
  -v locci-data:/app/data \
  locci/kv:latest

# Or using docker-compose
docker-compose -f docker/docker-compose.yml --profile standalone up -d

# Or using just
just docker-standalone-up
```

### Run 3-Node Cluster

```bash
# Using docker-compose
docker-compose -f docker/docker-compose.yml --profile cluster up -d

# Or using just
just docker-cluster-up

# View logs
just docker-cluster-logs
```

## Docker Compose Profiles

| Profile | Description |
|---------|-------------|
| `standalone` | Single node, no Raft |
| `cluster` | 3-node Raft cluster |

```bash
# Start specific profile
docker-compose -f docker/docker-compose.yml --profile standalone up -d
docker-compose -f docker/docker-compose.yml --profile cluster up -d
```

## Ports

| Service | HTTP Port | Raft Port |
|---------|-----------|-----------|
| Standalone | 8080 | - |
| Node 1 | 8081 | 9001 |
| Node 2 | 8082 | 9002 |
| Node 3 | 8083 | 9003 |

## Volumes

Data is persisted in Docker volumes:

- `locci-kv-standalone-data` - Standalone mode data
- `locci-kv-node1-data` - Node 1 cluster data
- `locci-kv-node2-data` - Node 2 cluster data
- `locci-kv-node3-data` - Node 3 cluster data

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `LOCCI_KV_BIND_ADDR` | `0.0.0.0:8080` | HTTP bind address |
| `LOCCI_KV_DATA_DIR` | `/app/data` | Data directory |
| `LOCCI_LOG_LEVEL` | `info` | Log level |

## Testing the Cluster

```bash
# Check health
curl http://localhost:8081/health
curl http://localhost:8082/health
curl http://localhost:8083/health

# Check Raft status (find leader)
curl http://localhost:8081/raft/status
curl http://localhost:8082/raft/status
curl http://localhost:8083/raft/status

# Write to leader
curl -X POST http://localhost:8081/kv/test \
  -H "Content-Type: application/json" \
  -d '{"value": "hello from docker!"}'

# Read from any node
curl http://localhost:8082/kv/test
curl http://localhost:8083/kv/test
```

## Cleanup

```bash
# Stop and remove containers
docker-compose -f docker/docker-compose.yml down

# Also remove volumes
docker-compose -f docker/docker-compose.yml down -v

# Or using just
just docker-clean
```

## Building for Multiple Platforms

```bash
# Build for linux/amd64 and linux/arm64
docker buildx build --platform linux/amd64,linux/arm64 \
  -t locci/kv:latest -f docker/Dockerfile .
```

## Docker Hub

Images are automatically published to Docker Hub on:
- Push to `main` branch (tagged as `latest`)
- Version tags (e.g., `v0.1.0`)

Pull the latest image:
```bash
docker pull locci/kv:latest
```
