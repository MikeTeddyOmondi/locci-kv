//! Storage-level tests for prefix-scoped key listing.

use locci_kv::config::{Config, RocksDBConfig};
use locci_kv::storage::rocksdb_storage::RocksDBStorage;
use locci_kv::storage::Storage;

fn test_config() -> RocksDBConfig {
    Config::default().storage.rocksdb
}

async fn seeded_store(dir: &tempfile::TempDir) -> RocksDBStorage {
    let storage = RocksDBStorage::new(dir.path(), &test_config()).unwrap();

    for key in [
        "locci-functions:proj_abc:one",
        "locci-functions:proj_xyz:one",
        "locci-functions:proj_xyz:two",
        "locci-functions:proj_xyzzy:three",
        "other:proj_xyz:four",
        "zzz-after-everything",
    ] {
        storage.put(key.as_bytes(), b"v").await.unwrap();
    }

    storage
}

fn as_strings(keys: Vec<Vec<u8>>) -> Vec<String> {
    keys.into_iter()
        .map(|k| String::from_utf8(k).unwrap())
        .collect()
}

#[tokio::test]
async fn prefix_list_returns_only_matching_keys() {
    let dir = tempfile::tempdir().unwrap();
    let storage = seeded_store(&dir).await;

    let keys = as_strings(
        storage
            .list_keys(Some(b"locci-functions:proj_xyz"))
            .await
            .unwrap(),
    );

    // Note: proj_xyzzy is a legitimate match — the prefix is a byte prefix,
    // not a path segment.
    assert_eq!(
        keys,
        vec![
            "locci-functions:proj_xyz:one",
            "locci-functions:proj_xyz:two",
            "locci-functions:proj_xyzzy:three",
        ]
    );
}

#[tokio::test]
async fn prefix_list_stops_before_unrelated_keys() {
    let dir = tempfile::tempdir().unwrap();
    let storage = seeded_store(&dir).await;

    let keys = as_strings(storage.list_keys(Some(b"other:")).await.unwrap());
    assert_eq!(keys, vec!["other:proj_xyz:four"]);
}

#[tokio::test]
async fn unmatched_prefix_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let storage = seeded_store(&dir).await;

    assert!(storage
        .list_keys(Some(b"no-such-prefix"))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn no_prefix_returns_all_keys() {
    let dir = tempfile::tempdir().unwrap();
    let storage = seeded_store(&dir).await;

    assert_eq!(storage.list_keys(None).await.unwrap().len(), 6);
}
