# Multi-stage build for smallest Debian image
FROM rust:1.83-slim AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
WORKDIR /app

# Install build dependencies for RocksDB and protobuf
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libclang-dev \
    clang \
    protobuf-compiler \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Build dependencies first (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release \
    && strip target/release/locci-kv

# Use distroless for smallest secure Debian-based runtime
FROM gcr.io/distroless/cc-debian12:latest AS release
WORKDIR /app
COPY --from=build /app/target/release/locci-kv ./locci-kv
EXPOSE 8080 9001
ENTRYPOINT ["./locci-kv"]
CMD ["start"]
