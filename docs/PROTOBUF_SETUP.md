The build is failing because **`protoc` (Protocol Buffers compiler) is not installed** on your system. The `raft-rs` crate requires it to compile protobuf definitions.

## Quick Fix Options:

### Option 1: Install protoc (Recommended)

**On macOS:**

```bash
brew install protobuf
```

**On Ubuntu/Debian:**

```bash
sudo apt-get install protobuf-compiler
```

**On Fedora/RHEL:**

```bash
sudo dnf install protobuf-compiler
```

Then rebuild:

```bash
cargo build --release
```

---

### Option 2: Use vendored protoc (No system install needed)

Update `Cargo.toml` to use the bundled protoc:

```toml
[dependencies]
# ... existing dependencies ...

# Change this:
# raft = "0.7"
# raft-proto = "0.7"

# To this (with vendored feature):
raft = { version = "0.7", features = ["default-protobuf-codec"] }
raft-proto = { version = "0.7", features = ["bundled"] }
```

This will use a bundled version of protoc, so you don't need to install it system-wide.

---

### Option 3: Simplify for MVP (Skip protoc entirely)

If you want to test Phase 1 first without Raft, temporarily comment out the Raft dependencies in `Cargo.toml`:

```toml
[dependencies]
# ... keep all other dependencies ...

# Temporarily comment these out:
# raft = "0.7"
# raft-proto = "0.7"
# protobuf = "3.3"
```

Then you can build and test Phase 1:

```bash
cargo build --release
./target/release/locci-kv standalone
```

---

## Recommended Approach:

**Use Option 2** (vendored protoc) - add the `bundled` feature:

```toml
raft-proto = { version = "0.7", features = ["bundled"] }
```

This is the easiest solution that doesn't require system dependencies.

---

## Error

```bash
❯ cargo build --release
   Compiling serde v1.0.228
   Compiling autocfg v1.5.0
   Compiling aho-corasick v1.1.3
   Compiling regex-syntax v0.8.8
   Compiling crossbeam-utils v0.8.21
   Compiling zerocopy v0.8.27
   Compiling itertools v0.12.1
   Compiling slog v2.8.2
   Compiling protobuf v2.28.0
   Compiling getrandom v0.2.16
   Compiling http v0.2.12
   Compiling futures-sink v0.3.31
   Compiling bytes v1.10.1
   Compiling tokio-util v0.7.16
   Compiling num-traits v0.2.19
   Compiling rand_core v0.6.4
   Compiling slab v0.4.11
   Compiling regex-automata v0.4.13
   Compiling http-body v0.4.6
   Compiling indexmap v1.9.3
   Compiling try-lock v0.2.5
   Compiling num-conv v0.1.0
   Compiling bitflags v1.3.2
   Compiling erased-serde v0.3.31
   Compiling core-foundation-sys v0.8.7
   Compiling regex v1.12.2
   Compiling powerfmt v0.2.0
   Compiling time-core v0.1.6
   Compiling bindgen v0.69.5
   Compiling time-macros v0.2.24
   Compiling deranged v0.5.4
   Compiling iana-time-zone v0.1.64
   Compiling want v0.3.1
   Compiling crossbeam-channel v0.5.15
   Compiling h2 v0.3.27
   Compiling anyhow v1.0.100
   Compiling pin-project-internal v1.1.10
   Compiling protobuf-codegen v2.28.0
   Compiling ppv-lite86 v0.2.21
   Compiling rand_chacha v0.3.1
   Compiling rand v0.8.5
   Compiling socket2 v0.5.10
   Compiling protobuf-build v0.14.1
   Compiling axum-core v0.3.4
   Compiling arc-swap v1.7.1
   Compiling slog-async v2.8.0
   Compiling hashbrown v0.12.3
   Compiling librocksdb-sys v0.16.0+8.10.0
   Compiling time v0.3.44
   Compiling slog-scope v4.4.0
   Compiling raft-proto v0.7.0
   Compiling pin-project v1.1.10
   Compiling chrono v0.4.42
   Compiling proc-macro-error-attr2 v2.0.0
   Compiling is-terminal v0.4.17
   Compiling axum v0.6.20
   Compiling hyper v0.14.32
   Compiling take_mut v0.2.2
   Compiling term v1.2.0
   Compiling slog-term v2.9.2
   Compiling proc-macro-error2 v2.0.1
   Compiling tower v0.4.13
   Compiling prost-derive v0.12.6
   Compiling slog-stdlog v4.1.1
   Compiling crossbeam-epoch v0.9.18
   Compiling tokio-io-timeout v1.2.1
   Compiling async-stream-impl v0.3.6
   Compiling sync_wrapper v0.1.2
   Compiling protobuf v3.7.2
   Compiling byteorder v1.5.0
   Compiling async-stream v0.3.6
error: failed to run custom build command for `raft-proto v0.7.0`

Caused by:
  process didn't exit successfully: `/Users/mt0/src/mt0/locci-suite/locci-kv/target/release/build/raft-proto-954aeb8c8e8e241a/build-script-build` (exit status: 101)
  --- stdout
  `protoc` not in PATH, try using the bundled protoc

  --- stderr

  thread 'main' panicked at /Users/mt0/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/protobuf-build-0.14.1/src/protobuf_impl.rs:35:14:
  No suitable `protoc` (>= 3.1.0) found in PATH
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
warning: build failed, waiting for other jobs to finish...

```
