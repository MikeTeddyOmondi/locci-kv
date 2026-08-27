# Changelog

All notable changes to locci-kv are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

## [0.1.0] - 2026-08-27

### Added

- `GET /keys/:prefix` endpoint for prefix-scoped key listing. The prefix is a
  byte prefix rather than a path segment, so `proj_xyz` also matches
  `proj_xyzzy`; include the trailing separator to scope exactly. Prefixes
  containing `/` must be URL-encoded.

### Changed

- `RocksDBStorage::list_keys` now seeks directly to the requested prefix with
  `IteratorMode::From` and stops at the first non-matching key, instead of
  iterating the entire store and filtering. Prefix-scoped listing cost now
  scales with the number of matching keys rather than total store size. This
  also speeds up Raft log recovery, which reads via `RAFT_LOG_PREFIX`.

### Fixed

- Docker build failed at the `chef` stage: `cargo install cargo-chef` was
  unpinned and resolved transitive dependencies requiring rustc 1.91 against the
  pinned `rust:1.88-slim` base image. Now installed with `--locked`.

### Notes

- The server still defaults to binding `127.0.0.1:8080`, which is unreachable
  through a Docker port mapping or from a sibling Compose service. Set
  `LOCCI_KV_BIND_ADDR=0.0.0.0:8080` in containerized deployments.
