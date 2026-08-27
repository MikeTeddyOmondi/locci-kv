// use crate::error::{LocciKVError, Result};
// use crate::storage::Storage as KVStorage;
// use parking_lot::RwLock;
// use raft::eraftpb::{ConfState, Entry, HardState, Snapshot};
// use raft::{RaftState, Storage as RaftStorage, StorageError};
// use std::sync::Arc;

// const RAFT_HARD_STATE_KEY: &[u8] = b"__raft_hard_state__";
// const RAFT_CONF_STATE_KEY: &[u8] = b"__raft_conf_state__";
// const RAFT_SNAPSHOT_KEY: &[u8] = b"__raft_snapshot__";
// const RAFT_LOG_PREFIX: &[u8] = b"__raft_log_";

// /// Raft storage backed by RocksDB
// pub struct RaftStorageAdapter {
//     kv_storage: Arc<dyn KVStorage>,
//     // In-memory cache for Raft state
//     hard_state: RwLock<HardState>,
//     conf_state: RwLock<ConfState>,
//     entries: RwLock<Vec<Entry>>,
// }

// impl RaftStorageAdapter {
//     pub async fn new(kv_storage: Arc<dyn KVStorage>) -> Result<Self> {
//         let hard_state = Self::load_hard_state(&kv_storage).await?;
//         let conf_state = Self::load_conf_state(&kv_storage).await?;
//         let entries = Vec::new();

//         Ok(Self {
//             kv_storage,
//             hard_state: RwLock::new(hard_state),
//             conf_state: RwLock::new(conf_state),
//             entries: RwLock::new(entries),
//         })
//     }

//     async fn load_hard_state(storage: &Arc<dyn KVStorage>) -> Result<HardState> {
//         match storage.get(RAFT_HARD_STATE_KEY).await? {
//             Some(data) => {
//                 let hs: HardState = protobuf::Message::parse_from_bytes(&data)
//                     .map_err(|e| LocciKVError::Protobuf(e))?;
//                 Ok(hs)
//             }
//             None => Ok(HardState::default()),
//         }
//     }

//     async fn load_conf_state(storage: &Arc<dyn KVStorage>) -> Result<ConfState> {
//         match storage.get(RAFT_CONF_STATE_KEY).await? {
//             Some(data) => {
//                 let cs: ConfState = protobuf::Message::parse_from_bytes(&data)
//                     .map_err(|e| LocciKVError::Protobuf(e))?;
//                 Ok(cs)
//             }
//             None => Ok(ConfState::default()),
//         }
//     }

//     pub async fn save_hard_state(&self, hs: &HardState) -> Result<()> {
//         let data = protobuf::Message::write_to_bytes(hs)
//             .map_err(|e| LocciKVError::Protobuf(e))?;
//         self.kv_storage.put(RAFT_HARD_STATE_KEY, &data).await?;
//         *self.hard_state.write() = hs.clone();
//         Ok(())
//     }

//     pub async fn save_conf_state(&self, cs: &ConfState) -> Result<()> {
//         let data = protobuf::Message::write_to_bytes(cs)
//             .map_err(|e| LocciKVError::Protobuf(e))?;
//         self.kv_storage.put(RAFT_CONF_STATE_KEY, &data).await?;
//         *self.conf_state.write() = cs.clone();
//         Ok(())
//     }

//     pub async fn append_entries(&self, entries: &[Entry]) -> Result<()> {
//         let mut cached_entries = self.entries.write();

//         for entry in entries {
//             let key = format!("{}{}",
//                 String::from_utf8_lossy(RAFT_LOG_PREFIX),
//                 entry.index
//             );
//             let data = protobuf::Message::write_to_bytes(entry)
//                 .map_err(|e| LocciKVError::Protobuf(e))?;
//             self.kv_storage.put(key.as_bytes(), &data).await?;
//             cached_entries.push(entry.clone());
//         }

//         Ok(())
//     }

//     fn get_entry(&self, idx: u64) -> raft::Result<Entry> {
//         let entries = self.entries.read();

//         if let Some(entry) = entries.iter().find(|e| e.index == idx) {
//             return Ok(entry.clone());
//         }

//         Err(raft::Error::Store(StorageError::Unavailable))
//     }
// }

// impl RaftStorage for RaftStorageAdapter {
//     fn initial_state(&self) -> raft::Result<RaftState> {
//         let hard_state = self.hard_state.read().clone();
//         let conf_state = self.conf_state.read().clone();
//         Ok(RaftState {
//             hard_state,
//             conf_state,
//         })
//     }

//     fn entries(
//         &self,
//         low: u64,
//         high: u64,
//         max_size: impl Into<Option<u64>>,
//     ) -> raft::Result<Vec<Entry>> {
//         let entries = self.entries.read();
//         let max_size = max_size.into();

//         let mut result = Vec::new();
//         let mut total_size = 0u64;

//         for entry in entries.iter() {
//             if entry.index >= low && entry.index < high {
//                 let entry_size = protobuf::Message::compute_size(entry) as u64;

//                 if let Some(max) = max_size {
//                     if total_size + entry_size > max && !result.is_empty() {
//                         break;
//                     }
//                 }

//                 total_size += entry_size;
//                 result.push(entry.clone());
//             }
//         }

//         if result.is_empty() {
//             return Err(raft::Error::Store(StorageError::Unavailable));
//         }

//         Ok(result)
//     }

//     fn term(&self, idx: u64) -> raft::Result<u64> {
//         let entry = self.get_entry(idx)?;
//         Ok(entry.term)
//     }

//     fn first_index(&self) -> raft::Result<u64> {
//         let entries = self.entries.read();
//         entries
//             .first()
//             .map(|e| e.index)
//             .ok_or(raft::Error::Store(StorageError::Unavailable))
//     }

//     fn last_index(&self) -> raft::Result<u64> {
//         let entries = self.entries.read();
//         entries
//             .last()
//             .map(|e| e.index)
//             .ok_or(raft::Error::Store(StorageError::Unavailable))
//     }

//     fn snapshot(&self, request_index: u64, to: u64) -> raft::Result<Snapshot> {
//         // Simplified snapshot implementation
//         let mut snapshot = Snapshot::default();
//         snapshot.mut_metadata().index = request_index;
//         snapshot.mut_metadata().term = 0;
//         Ok(snapshot)
//     }
// }

// use crate::error::{LocciKVError, Result};
// use crate::storage::Storage as KVStorage;
// use parking_lot::RwLock;
// use prost::Message; // CHANGED: Use prost instead of protobuf
// use raft::eraftpb::{ConfState, Entry, HardState, Snapshot};
// use raft::{GetEntriesContext, RaftState, Storage as RaftStorage, StorageError}; // ADDED: GetEntriesContext
// use std::sync::Arc;

// const RAFT_HARD_STATE_KEY: &[u8] = b"__raft_hard_state__";
// const RAFT_CONF_STATE_KEY: &[u8] = b"__raft_conf_state__";
// const RAFT_SNAPSHOT_KEY: &[u8] = b"__raft_snapshot__";
// const RAFT_LOG_PREFIX: &[u8] = b"__raft_log_";

// /// Raft storage backed by RocksDB
// #[derive(Clone)] // ADDED: Derive Clone
// pub struct RaftStorageAdapter {
//     kv_storage: Arc<dyn KVStorage>,
//     // In-memory cache for Raft state
//     hard_state: Arc<RwLock<HardState>>, // CHANGED: Wrap in Arc
//     conf_state: Arc<RwLock<ConfState>>, // CHANGED: Wrap in Arc
//     entries: Arc<RwLock<Vec<Entry>>>,   // CHANGED: Wrap in Arc
// }

// impl RaftStorageAdapter {
//     pub async fn new(kv_storage: Arc<dyn KVStorage>) -> Result<Self> {
//         let hard_state = Self::load_hard_state(&kv_storage).await?;
//         let conf_state = Self::load_conf_state(&kv_storage).await?;
//         let entries = Vec::new();

//         Ok(Self {
//             kv_storage,
//             hard_state: Arc::new(RwLock::new(hard_state)),
//             conf_state: Arc::new(RwLock::new(conf_state)),
//             entries: Arc::new(RwLock::new(entries)),
//         })
//     }

//     async fn load_hard_state(storage: &Arc<dyn KVStorage>) -> Result<HardState> {
//         match storage.get(RAFT_HARD_STATE_KEY).await? {
//             Some(data) => {
//                 // CHANGED: Use prost decode
//                 let hs = HardState::decode(&data[..])?;
//                 Ok(hs)
//             }
//             None => Ok(HardState::default()),
//         }
//     }

//     async fn load_conf_state(storage: &Arc<dyn KVStorage>) -> Result<ConfState> {
//         match storage.get(RAFT_CONF_STATE_KEY).await? {
//             Some(data) => {
//                 // CHANGED: Use prost decode
//                 let cs = ConfState::decode(&data[..])?;
//                 Ok(cs)
//             }
//             None => Ok(ConfState::default()),
//         }
//     }

//     pub async fn save_hard_state(&self, hs: &HardState) -> Result<()> {
//         // CHANGED: Use prost encode
//         let mut buf = Vec::new();
//         hs.encode(&mut buf)?;
//         self.kv_storage.put(RAFT_HARD_STATE_KEY, &buf).await?;
//         *self.hard_state.write() = hs.clone();
//         Ok(())
//     }

//     pub async fn save_conf_state(&self, cs: &ConfState) -> Result<()> {
//         // CHANGED: Use prost encode
//         let mut buf = Vec::new();
//         cs.encode(&mut buf)?;
//         self.kv_storage.put(RAFT_CONF_STATE_KEY, &buf).await?;
//         *self.conf_state.write() = cs.clone();
//         Ok(())
//     }

//     pub async fn append_entries(&self, entries: &[Entry]) -> Result<()> {
//         let mut cached_entries = self.entries.write();

//         for entry in entries {
//             let key = format!("{}{}",
//                 String::from_utf8_lossy(RAFT_LOG_PREFIX),
//                 entry.index
//             );
//             // CHANGED: Use prost encode
//             let mut buf = Vec::new();
//             entry.encode(&mut buf)?;
//             self.kv_storage.put(key.as_bytes(), &buf).await?;
//             cached_entries.push(entry.clone());
//         }

//         Ok(())
//     }

//     fn get_entry(&self, idx: u64) -> raft::Result<Entry> {
//         let entries = self.entries.read();

//         if let Some(entry) = entries.iter().find(|e| e.index == idx) {
//             return Ok(entry.clone());
//         }

//         Err(raft::Error::Store(StorageError::Unavailable))
//     }
// }

// impl RaftStorage for RaftStorageAdapter {
//     fn initial_state(&self) -> raft::Result<RaftState> {
//         let hard_state = self.hard_state.read().clone();
//         let conf_state = self.conf_state.read().clone();
//         Ok(RaftState {
//             hard_state,
//             conf_state,
//         })
//     }

//     fn entries(
//         &self,
//         low: u64,
//         high: u64,
//         max_size: impl Into<Option<u64>>,
//         _context: GetEntriesContext, // ADDED: Missing parameter
//     ) -> raft::Result<Vec<Entry>> {
//         let entries = self.entries.read();
//         let max_size = max_size.into();

//         let mut result = Vec::new();
//         let mut total_size = 0u64;

//         for entry in entries.iter() {
//             if entry.index >= low && entry.index < high {
//                 // CHANGED: Use prost encoded_len
//                 let entry_size = entry.encoded_len() as u64;

//                 if let Some(max) = max_size {
//                     if total_size + entry_size > max && !result.is_empty() {
//                         break;
//                     }
//                 }

//                 total_size += entry_size;
//                 result.push(entry.clone());
//             }
//         }

//         if result.is_empty() {
//             return Err(raft::Error::Store(StorageError::Unavailable));
//         }

//         Ok(result)
//     }

//     fn term(&self, idx: u64) -> raft::Result<u64> {
//         let entry = self.get_entry(idx)?;
//         Ok(entry.term)
//     }

//     fn first_index(&self) -> raft::Result<u64> {
//         let entries = self.entries.read();
//         entries
//             .first()
//             .map(|e| e.index)
//             .ok_or(raft::Error::Store(StorageError::Unavailable))
//     }

//     fn last_index(&self) -> raft::Result<u64> {
//         let entries = self.entries.read();
//         entries
//             .last()
//             .map(|e| e.index)
//             .ok_or(raft::Error::Store(StorageError::Unavailable))
//     }

//     fn snapshot(&self, request_index: u64, _to: u64) -> raft::Result<Snapshot> {
//         // Simplified snapshot implementation
//         let mut snapshot = Snapshot::default();
//         snapshot.mut_metadata().index = request_index;
//         snapshot.mut_metadata().term = 0;
//         Ok(snapshot)
//     }
// }

use crate::error::Result;
use crate::storage::Storage as KVStorage;
use bytes::Bytes;
use parking_lot::RwLock;
use prost::Message; // Use prost 0.11
use raft::eraftpb::{ConfState, Entry, HardState, Snapshot};
use raft::{GetEntriesContext, RaftState, Storage as RaftStorage, StorageError};
use std::sync::Arc;

const RAFT_HARD_STATE_KEY: &[u8] = b"__raft_hard_state__";
const RAFT_CONF_STATE_KEY: &[u8] = b"__raft_conf_state__";
#[allow(dead_code)] // Used in Phase 3 for snapshots
const RAFT_SNAPSHOT_KEY: &[u8] = b"__raft_snapshot__";
const RAFT_LOG_PREFIX: &[u8] = b"__raft_log_";

/// Raft storage backed by RocksDB
#[derive(Clone)]
pub struct RaftStorageAdapter {
    kv_storage: Arc<dyn KVStorage>,
    hard_state: Arc<RwLock<HardState>>,
    conf_state: Arc<RwLock<ConfState>>,
    entries: Arc<RwLock<Vec<Entry>>>,
    /// Snapshot metadata index - used for index calculations when entries are empty
    snapshot_index: Arc<RwLock<u64>>,
    /// Snapshot metadata term - used for term lookups at snapshot index
    snapshot_term: Arc<RwLock<u64>>,
}

impl RaftStorageAdapter {
    pub async fn new(kv_storage: Arc<dyn KVStorage>) -> Result<Self> {
        Self::new_inner(kv_storage, None).await
    }

    /// Create storage with bootstrap - initializes ConfState with voters if not already set
    pub async fn new_with_bootstrap(
        kv_storage: Arc<dyn KVStorage>,
        voter_ids: Vec<u64>,
    ) -> Result<Self> {
        Self::new_inner(kv_storage, Some(voter_ids)).await
    }

    async fn new_inner(
        kv_storage: Arc<dyn KVStorage>,
        bootstrap_voters: Option<Vec<u64>>,
    ) -> Result<Self> {
        let hard_state = Self::load_hard_state(&kv_storage).await?;
        let mut conf_state = Self::load_conf_state(&kv_storage).await?;
        let entries = Self::load_entries(&kv_storage).await?;

        // If bootstrapping and conf_state is empty, initialize voters
        if let Some(voters) = bootstrap_voters {
            if conf_state.voters.is_empty() {
                conf_state.voters = voters;
                // Persist the initial conf_state
                let adapter = Self {
                    kv_storage: kv_storage.clone(),
                    hard_state: Arc::new(RwLock::new(hard_state.clone())),
                    conf_state: Arc::new(RwLock::new(conf_state.clone())),
                    entries: Arc::new(RwLock::new(entries.clone())),
                    snapshot_index: Arc::new(RwLock::new(0)),
                    snapshot_term: Arc::new(RwLock::new(0)),
                };
                adapter.save_conf_state(&conf_state).await?;
                return Ok(adapter);
            }
        }

        Ok(Self {
            kv_storage,
            hard_state: Arc::new(RwLock::new(hard_state)),
            conf_state: Arc::new(RwLock::new(conf_state)),
            entries: Arc::new(RwLock::new(entries)),
            // Initialize snapshot metadata to 0 - this means first_index=1, last_index=0 for empty log
            snapshot_index: Arc::new(RwLock::new(0)),
            snapshot_term: Arc::new(RwLock::new(0)),
        })
    }

    /// Load all Raft log entries from storage on startup
    async fn load_entries(storage: &Arc<dyn KVStorage>) -> Result<Vec<Entry>> {
        let keys = storage.list_keys(Some(RAFT_LOG_PREFIX)).await?;
        let mut entries = Vec::new();

        for key in keys {
            if let Some(data) = storage.get(&key).await? {
                let entry = Entry::decode(Bytes::from(data))?;
                entries.push(entry);
            }
        }

        // Sort by index to ensure correct order
        entries.sort_by_key(|e| e.index);

        Ok(entries)
    }

    async fn load_hard_state(storage: &Arc<dyn KVStorage>) -> Result<HardState> {
        match storage.get(RAFT_HARD_STATE_KEY).await? {
            Some(data) => {
                let hs = HardState::decode(Bytes::from(data))?;
                Ok(hs)
            }
            None => Ok(HardState::default()),
        }
    }

    async fn load_conf_state(storage: &Arc<dyn KVStorage>) -> Result<ConfState> {
        match storage.get(RAFT_CONF_STATE_KEY).await? {
            Some(data) => {
                let cs = ConfState::decode(Bytes::from(data))?;
                Ok(cs)
            }
            None => Ok(ConfState::default()),
        }
    }

    pub async fn save_hard_state(&self, hs: &HardState) -> Result<()> {
        let mut buf = Vec::with_capacity(hs.encoded_len());
        hs.encode(&mut buf)?;
        self.kv_storage.put(RAFT_HARD_STATE_KEY, &buf).await?;
        *self.hard_state.write() = hs.clone();
        Ok(())
    }

    pub async fn save_conf_state(&self, cs: &ConfState) -> Result<()> {
        let mut buf = Vec::with_capacity(cs.encoded_len());
        cs.encode(&mut buf)?;
        self.kv_storage.put(RAFT_CONF_STATE_KEY, &buf).await?;
        *self.conf_state.write() = cs.clone();
        Ok(())
    }

    pub async fn append_entries(&self, entries: &[Entry]) -> Result<()> {
        // Prepare all the data BEFORE acquiring the lock
        let mut entries_to_save = Vec::new();

        for entry in entries {
            let key = format!(
                "{}{}",
                String::from_utf8_lossy(RAFT_LOG_PREFIX),
                entry.index
            );
            let mut buf = Vec::with_capacity(entry.encoded_len());
            entry.encode(&mut buf)?;
            entries_to_save.push((key, buf, entry.clone()));
        }

        // Now do all the async storage operations WITHOUT holding the lock
        for (key, buf, _) in &entries_to_save {
            self.kv_storage.put(key.as_bytes(), buf).await?;
        }

        // ONLY NOW acquire the lock and update the in-memory cache
        {
            let mut cached_entries = self.entries.write();
            for (_, _, entry) in entries_to_save {
                cached_entries.push(entry);
            }
        } // Lock is dropped here

        Ok(())
    }
    // pub async fn append_entries(&self, entries: &[Entry]) -> Result<()> {
    //     let mut cached_entries = self.entries.write();

    //     for entry in entries {
    //         let key = format!("{}{}",
    //             String::from_utf8_lossy(RAFT_LOG_PREFIX),
    //             entry.index
    //         );
    //         let mut buf = Vec::with_capacity(entry.encoded_len());
    //         entry.encode(&mut buf)?;
    //         self.kv_storage.put(key.as_bytes(), &buf).await?;
    //         cached_entries.push(entry.clone());
    //     }

    //     Ok(())
    // }

    fn get_entry(&self, idx: u64) -> raft::Result<Entry> {
        let entries = self.entries.read();

        if let Some(entry) = entries.iter().find(|e| e.index == idx) {
            return Ok(entry.clone());
        }

        Err(raft::Error::Store(StorageError::Unavailable))
    }
}

impl RaftStorage for RaftStorageAdapter {
    fn initial_state(&self) -> raft::Result<RaftState> {
        let hard_state = self.hard_state.read().clone();
        let conf_state = self.conf_state.read().clone();
        Ok(RaftState {
            hard_state,
            conf_state,
        })
    }

    fn entries(
        &self,
        low: u64,
        high: u64,
        max_size: impl Into<Option<u64>>,
        _context: GetEntriesContext,
    ) -> raft::Result<Vec<Entry>> {
        let entries = self.entries.read();
        let max_size = max_size.into();

        let mut result = Vec::new();
        let mut total_size = 0u64;

        for entry in entries.iter() {
            if entry.index >= low && entry.index < high {
                let entry_size = entry.encoded_len() as u64;

                if let Some(max) = max_size {
                    if total_size + entry_size > max && !result.is_empty() {
                        break;
                    }
                }

                total_size += entry_size;
                result.push(entry.clone());
            }
        }

        if result.is_empty() {
            return Err(raft::Error::Store(StorageError::Unavailable));
        }

        Ok(result)
    }

    fn term(&self, idx: u64) -> raft::Result<u64> {
        let snapshot_index = *self.snapshot_index.read();
        let snapshot_term = *self.snapshot_term.read();

        // If requesting term at snapshot index, return snapshot term
        if idx == snapshot_index {
            return Ok(snapshot_term);
        }

        // If index is before snapshot, it's been compacted
        if idx < snapshot_index {
            return Err(raft::Error::Store(StorageError::Compacted));
        }

        // Otherwise look up in entries
        let entry = self.get_entry(idx)?;
        Ok(entry.term)
    }

    fn first_index(&self) -> raft::Result<u64> {
        let entries = self.entries.read();
        match entries.first() {
            Some(e) => Ok(e.index),
            // When entries are empty, first available index is snapshot_index + 1
            None => Ok(*self.snapshot_index.read() + 1),
        }
    }

    fn last_index(&self) -> raft::Result<u64> {
        let entries = self.entries.read();
        match entries.last() {
            Some(e) => Ok(e.index),
            // When entries are empty, last index is the snapshot index
            None => Ok(*self.snapshot_index.read()),
        }
    }

    fn snapshot(&self, request_index: u64, _to: u64) -> raft::Result<Snapshot> {
        let mut snapshot = Snapshot::default();
        snapshot.mut_metadata().index = request_index;
        snapshot.mut_metadata().term = 0;
        Ok(snapshot)
    }
}
