# locci-kv — Backlog

Deferred / future work. Completed items graduate to [README.md](./README.md).

---

## Performance

### ~~Prefix-Based Key Listing (`feat/kv-prefix-list`)~~ — DONE

Shipped: `GET /keys/:prefix` plus real RocksDB prefix iteration in
`RocksDBStorage::list_keys`. See [README.md](./README.md#list-keys-by-prefix).
Remaining follow-up: benchmark prefix vs full scan at production data sizes.

<details>
<summary>Original entry</summary>

**Problem:** `GET /keys` scans the entire RocksDB store, causing 42-192 ms latency spikes per invocation.
Client SDKs calling this endpoint twice per request for namespace validation amplifies the bottleneck.

Current latency:
- Full scan: 42-192 ms (varies with data size)
- No filtering: returns all keys in store

**Solution:**

Add `/keys/:prefix` endpoint for efficient prefix-scoped listing:

```rust
.route("/keys/:prefix", get(list_keys_with_prefix))

async fn list_keys_with_prefix(
    State(state): State<AppState>,
    Path(prefix): Path<String>,
) -> Result<Json<ListResponse>> {
    let prefix_bytes = prefix.as_bytes();
    let keys_bytes = state.storage.list_keys(Some(prefix_bytes)).await?;
    // Decode and return only keys matching prefix
    let keys: Vec<String> = keys_bytes
        .into_iter()
        .filter_map(|k| String::from_utf8(k).ok())
        .collect();
    
    Ok(Json(ListResponse { keys, count: keys.len() }))
}
```

**Implementation checklist:**

- [x] Add `/keys/:prefix` route to `src/api/http.rs`
- [x] Verify `storage.list_keys(Some(prefix))` uses RocksDB prefix iteration (not full scan + filter)
- [x] Test: `GET /keys/locci-functions:proj_xyz` returns only keys with that prefix
- [ ] Benchmark: prefix scan vs full scan latency at various data sizes

**Expected outcome:**
- Prefix scan latency: 0-2 ms (vs 42-192 ms for full scan)
- Clients can query project-scoped keys efficiently
- Safe to merge Raft Phase 2 afterward (prefix queries won't bottleneck leader)

</details>

---

## Phase 2: Raft Consensus

**Status:** In progress on branch `feat/raft-phase-2`

Leader-follower replication for data availability and durability. No longer
blocked — prefix listing has shipped.

**Note:** Once Raft is live, all writes serialize through leader.
