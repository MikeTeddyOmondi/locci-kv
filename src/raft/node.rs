// use crate::config::{ClusterConfig, RaftConfig as RaftCfg};
// use crate::error::{LocciKVError, Result};
// use crate::raft::proposal::{PendingProposal, Proposal};
// use crate::raft::state_machine::StateMachine;
// use crate::raft::storage::RaftStorageAdapter;
// use crate::storage::Storage;
// use parking_lot::RwLock;
// use raft::eraftpb::{ConfState, Message as RaftMessage};
// use raft::prelude::*;
// use std::collections::HashMap;
// use std::sync::Arc;
// use std::time::Duration;
// use tokio::sync::mpsc;
// use tracing::{debug, error, info, warn};

// pub struct RaftNode {
//     raw_node: RwLock<RawNode<RaftStorageAdapter>>,
//     state_machine: Arc<StateMachine>,
//     pending_proposals: RwLock<HashMap<u64, PendingProposal>>,
//     proposal_index: RwLock<u64>,
// }

// impl RaftNode {
//     pub async fn new(
//         node_id: u64,
//         storage: Arc<dyn Storage>,
//         raft_config: &RaftCfg,
//         cluster_config: &ClusterConfig,
//     ) -> Result<Self> {
//         // Create Raft storage adapter
//         let raft_storage = RaftStorageAdapter::new(storage.clone()).await?;

//         // Configure Raft
//         let config = Config {
//             id: node_id,
//             election_tick: raft_config.election_tick,
//             heartbeat_tick: raft_config.heartbeat_tick,
//             max_size_per_msg: raft_config.max_size_per_msg,
//             max_inflight_msgs: raft_config.max_inflight_msgs,
//             check_quorum: raft_config.check_quorum,
//             pre_vote: raft_config.pre_vote,
//             ..Default::default()
//         };

//         // Initialize Raft node
//         let raw_node = if cluster_config.bootstrap {
//             // Bootstrap a new cluster
//             let peers: Vec<u64> = cluster_config.peers.iter().map(|p| p.id).collect();
//             info!("Bootstrapping new cluster with peers: {:?}", peers);
//             RawNode::new(&config, raft_storage, &Default::default())?
//         } else {
//             // Join existing cluster
//             info!("Starting node to join existing cluster");
//             RawNode::new(&config, raft_storage, &Default::default())?
//         };

//         let state_machine = Arc::new(StateMachine::new(storage));

//         Ok(Self {
//             raw_node: RwLock::new(raw_node),
//             state_machine,
//             pending_proposals: RwLock::new(HashMap::new()),
//             proposal_index: RwLock::new(0),
//         })
//     }

//     /// Propose a change to the Raft cluster
//     pub async fn propose(&self, proposal: Proposal) -> Result<()> {
//         let data = bincode::serialize(&proposal)?;

//         // Create a channel for the response
//         let (tx, rx) = tokio::sync::oneshot::channel();

//         let proposal_id = {
//             let mut idx = self.proposal_index.write();
//             *idx += 1;
//             *idx
//         };

//         // Store pending proposal
//         self.pending_proposals.write().insert(
//             proposal_id,
//             PendingProposal {
//                 proposal: proposal.clone(),
//                 response_tx: tx,
//             },
//         );

//         // Propose to Raft
//         {
//             let mut node = self.raw_node.write();
//             node.propose(vec![], data)?;
//         }

//         // Wait for proposal to be committed (with timeout)
//         tokio::select! {
//             result = rx => {
//                 result.map_err(|_| LocciKVError::ProposalTimeout)?
//             }
//             _ = tokio::time::sleep(Duration::from_secs(5)) => {
//                 self.pending_proposals.write().remove(&proposal_id);
//                 Err(LocciKVError::ProposalTimeout)
//             }
//         }
//     }

//     /// Check if this node is the leader
//     pub fn is_leader(&self) -> bool {
//         let node = self.raw_node.read();
//         node.raft.state == StateRole::Leader
//     }

//     /// Get the current leader ID
//     pub fn leader_id(&self) -> Option<u64> {
//         let node = self.raw_node.read();
//         let leader = node.raft.leader_id;
//         if leader == 0 {
//             None
//         } else {
//             Some(leader)
//         }
//     }

//     /// Process a Raft message from another node
//     pub fn step(&self, msg: RaftMessage) -> Result<()> {
//         let mut node = self.raw_node.write();
//         node.step(msg)?;
//         Ok(())
//     }

//     /// Tick the Raft node (call periodically)
//     pub fn tick(&self) {
//         let mut node = self.raw_node.write();
//         node.tick();
//     }

//     /// Check if there's a ready state and process it
//     pub async fn handle_ready(&self) -> Result<Vec<RaftMessage>> {
//         let mut node = self.raw_node.write();

//         if !node.has_ready() {
//             return Ok(Vec::new());
//         }

//         let mut ready = node.ready();
//         let messages = ready.messages.drain(..).collect::<Vec<_>>();

//         // Persist entries
//         if !ready.entries().is_empty() {
//             let storage = node.raft.raft_log.store.clone();
//             drop(node); // Release lock before async operation

//             storage.append_entries(ready.entries()).await?;

//             node = self.raw_node.write();
//         }

//         // Apply committed entries
//         if let Some(committed_entries) = ready.committed_entries.take() {
//             for entry in committed_entries {
//                 if entry.data.is_empty() {
//                     // Configuration change or empty entry
//                     continue;
//                 }

//                 // Deserialize and apply proposal
//                 if let Ok(proposal) = bincode::deserialize::<Proposal>(&entry.data) {
//                     drop(node); // Release lock before async operation

//                     let result = self.state_machine.apply(proposal).await;

//                     // Notify pending proposals
//                     if let Some(pending) = self.pending_proposals.write().remove(&entry.index) {
//                         let _ = pending.response_tx.send(result);
//                     }

//                     node = self.raw_node.write();
//                 }
//             }
//         }

//         // Persist hard state
//         if let Some(hs) = ready.hs() {
//             let storage = node.raft.raft_log.store.clone();
//             let hs_clone = hs.clone();
//             drop(node);

//             storage.save_hard_state(&hs_clone).await?;

//             node = self.raw_node.write();
//         }

//         // Advance the Raft state machine
//         let mut light_rd = node.advance(ready);

//         // Handle light ready (messages to send)
//         let light_messages = light_rd.take_messages();

//         Ok([messages, light_messages].concat())
//     }
// }

use crate::config::{ClusterConfig, RaftConfig as RaftCfg};
use crate::error::{LocciKVError, Result};
use crate::raft::proposal::{PendingProposal, Proposal};
use crate::raft::state_machine::StateMachine;
use crate::raft::storage::RaftStorageAdapter;
use crate::storage::Storage;
use parking_lot::RwLock;
use raft::eraftpb::Message as RaftMessage;
use raft::prelude::*;
use raft::StateRole; // ADDED: Import StateRole
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

pub struct RaftNode {
    raw_node: RwLock<RawNode<RaftStorageAdapter>>,
    state_machine: Arc<StateMachine>,
    pending_proposals: RwLock<HashMap<u64, PendingProposal>>,
    proposal_index: RwLock<u64>,
}

impl RaftNode {
    pub async fn new(
        node_id: u64,
        storage: Arc<dyn Storage>,
        raft_config: &RaftCfg,
        cluster_config: &ClusterConfig,
    ) -> Result<Self> {
        // Create Raft storage adapter
        // All nodes need the initial ConfState with all voters for proper election
        let peers: Vec<u64> = cluster_config.peers.iter().map(|p| p.id).collect();
        if cluster_config.bootstrap {
            info!("Bootstrapping new cluster with peers: {:?}", peers);
        } else {
            info!("Joining cluster with peers: {:?}", peers);
        }
        // Both bootstrap and non-bootstrap nodes get the same initial voter list
        let raft_storage = RaftStorageAdapter::new_with_bootstrap(storage.clone(), peers).await?;

        // Configure Raft
        let config = Config {
            id: node_id,
            election_tick: raft_config.election_tick,
            heartbeat_tick: raft_config.heartbeat_tick,
            max_size_per_msg: raft_config.max_size_per_msg,
            max_inflight_msgs: raft_config.max_inflight_msgs,
            check_quorum: raft_config.check_quorum,
            pre_vote: raft_config.pre_vote,
            ..Default::default()
        };

        // Create a proper logger for raft
        let logger = slog::Logger::root(slog::Discard, slog::o!());

        // Initialize Raft node
        let raw_node = RawNode::new(&config, raft_storage, &logger)?;

        let state_machine = Arc::new(StateMachine::new(storage));

        Ok(Self {
            raw_node: RwLock::new(raw_node),
            state_machine,
            pending_proposals: RwLock::new(HashMap::new()),
            proposal_index: RwLock::new(0),
        })
    }

    /// Propose a change to the Raft cluster
    pub async fn propose(&self, proposal: Proposal) -> Result<()> {
        let data = bincode::serialize(&proposal)?;

        // Create a channel for the response
        let (tx, rx) = tokio::sync::oneshot::channel();

        let proposal_id = {
            let mut idx = self.proposal_index.write();
            *idx += 1;
            *idx
        };

        // Store pending proposal
        self.pending_proposals.write().insert(
            proposal_id,
            PendingProposal {
                proposal: proposal.clone(),
                response_tx: tx,
            },
        );

        // Propose to Raft (pass proposal_id as context for tracking)
        {
            let mut node = self.raw_node.write();
            node.propose(proposal_id.to_le_bytes().to_vec(), data)?;
        }

        // Wait for proposal to be committed (with timeout)
        tokio::select! {
            result = rx => {
                result.map_err(|_| LocciKVError::ProposalTimeout)?
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                self.pending_proposals.write().remove(&proposal_id);
                Err(LocciKVError::ProposalTimeout)
            }
        }
    }

    /// Check if this node is the leader
    pub fn is_leader(&self) -> bool {
        let node = self.raw_node.read();
        node.raft.state == StateRole::Leader
    }

    /// Get the current leader ID
    pub fn leader_id(&self) -> Option<u64> {
        let node = self.raw_node.read();
        let leader = node.raft.leader_id;
        if leader == 0 {
            None
        } else {
            Some(leader)
        }
    }

    /// Process a Raft message from another node
    pub fn step(&self, msg: RaftMessage) -> Result<()> {
        let mut node = self.raw_node.write();
        node.step(msg)?;
        Ok(())
    }

    /// Tick the Raft node (call periodically)
    pub fn tick(&self) {
        let mut node = self.raw_node.write();
        node.tick();
    }

    // /// Check if there's a ready state and process it
    // pub async fn handle_ready(&self) -> Result<Vec<RaftMessage>> {
    //     let mut node = self.raw_node.write();

    //     if !node.has_ready() {
    //         return Ok(Vec::new());
    //     }

    //     let mut ready = node.ready();
    //     let messages = ready.take_messages(); // CHANGED: Use take_messages()

    //     // Persist entries
    //     if !ready.entries().is_empty() {
    //         let storage = node.mut_store().clone(); // CHANGED: Use mut_store()
    //         drop(node); // Release lock before async operation

    //         storage.append_entries(ready.entries()).await?;

    //         node = self.raw_node.write();
    //     }

    //     // Apply committed entries
    //     let committed_entries = ready.take_committed_entries(); // CHANGED: Use take_committed_entries()
    //     if !committed_entries.is_empty() {
    //         for entry in committed_entries {
    //             if entry.data.is_empty() {
    //                 // Configuration change or empty entry
    //                 continue;
    //             }

    //             // Deserialize and apply proposal
    //             if let Ok(proposal) = bincode::deserialize::<Proposal>(&entry.data) {
    //                 drop(node); // Release lock before async operation

    //                 let result = self.state_machine.apply(proposal).await;

    //                 // Notify pending proposals
    //                 if let Some(pending) = self.pending_proposals.write().remove(&entry.index) {
    //                     let _ = pending.response_tx.send(result);
    //                 }

    //                 node = self.raw_node.write();
    //             }
    //         }
    //     }

    //     // Persist hard state
    //     if let Some(hs) = ready.hs() {
    //         let storage = node.mut_store().clone(); // CHANGED: Use mut_store()
    //         let hs_clone = hs.clone();
    //         drop(node);

    //         storage.save_hard_state(&hs_clone).await?;

    //         node = self.raw_node.write();
    //     }

    //     // Advance the Raft state machine
    //     let mut light_rd = node.advance(ready);

    //     // Handle light ready (messages to send)
    //     let light_messages = light_rd.take_messages();

    //     Ok([messages, light_messages].concat())
    // }

    // /// Check if there's a ready state and process it
    // pub async fn handle_ready(&self) -> Result<Vec<RaftMessage>> {
    //     let mut node = self.raw_node.write();

    //     if !node.has_ready() {
    //         return Ok(Vec::new());
    //     }

    //     let mut ready = node.ready();
    //     let messages = ready.take_messages();

    //     // Persist entries - clone storage first to avoid holding lock across await
    //     if !ready.entries().is_empty() {
    //         let entries_to_save = ready.entries().to_vec(); // Clone entries
    //         let storage = node.mut_store().clone();
    //         drop(node); // Release lock BEFORE async operation

    //         storage.append_entries(&entries_to_save).await?;

    //         node = self.raw_node.write(); // Reacquire lock
    //     }

    //     // Apply committed entries
    //     let committed_entries = ready.take_committed_entries();
    //     if !committed_entries.is_empty() {
    //         for entry in committed_entries {
    //             if entry.data.is_empty() {
    //                 continue;
    //             }

    //             if let Ok(proposal) = bincode::deserialize::<Proposal>(&entry.data) {
    //                 let entry_index = entry.index;
    //                 drop(node); // Release lock before async operation

    //                 let result = self.state_machine.apply(proposal).await;

    //                 // Notify pending proposals
    //                 if let Some(pending) = self.pending_proposals.write().remove(&entry_index) {
    //                     let _ = pending.response_tx.send(result);
    //                 }

    //                 node = self.raw_node.write(); // Reacquire lock
    //             }
    //         }
    //     }

    //     // Persist hard state
    //     if let Some(hs) = ready.hs() {
    //         let hs_clone = hs.clone();
    //         let storage = node.mut_store().clone();
    //         drop(node); // Release lock before async operation

    //         storage.save_hard_state(&hs_clone).await?;

    //         node = self.raw_node.write(); // Reacquire lock
    //     }

    //     // Advance the Raft state machine
    //     let mut light_rd = node.advance(ready);
    //     let light_messages = light_rd.take_messages();

    //     Ok([messages, light_messages].concat())
    // }

    /// Get the current Raft state for debugging
    pub fn raft_state(&self) -> String {
        let node = self.raw_node.read();
        let voters: Vec<u64> = node.raft.prs().conf().voters().ids().iter().collect();
        let prs_count = node.raft.prs().iter().count();
        format!(
            "state={:?}, term={}, vote={}, lead={}, voters={:?}, prs_count={}",
            node.raft.state, node.raft.term, node.raft.vote, node.raft.leader_id, voters, prs_count
        )
    }

    /// Check if there's a ready state and process it
    pub async fn handle_ready(&self) -> Result<Vec<RaftMessage>> {
        // Step 1: Get Ready and extract data that needs persistence
        let (
            messages,
            persisted_messages,
            entries_to_save,
            ready_committed,
            hard_state_opt,
            storage,
            ready,
        ) = {
            let mut node = self.raw_node.write();

            if !node.has_ready() {
                return Ok(Vec::new());
            }

            let mut ready = node.ready();
            let messages = ready.take_messages();
            let persisted_messages = ready.take_persisted_messages();
            let entries_to_save = ready.entries().to_vec();
            let ready_committed = ready.take_committed_entries();
            let hard_state_opt = ready.hs().cloned();
            let storage = node.mut_store().clone();

            tracing::debug!(
                "Raft Ready: messages={}, persisted_messages={}, entries={}, committed={}, has_hs={}",
                messages.len(),
                persisted_messages.len(),
                entries_to_save.len(),
                ready_committed.len(),
                hard_state_opt.is_some()
            );

            (
                messages,
                persisted_messages,
                entries_to_save,
                ready_committed,
                hard_state_opt,
                storage,
                ready,
            )
        }; // Drop lock to do async persistence

        // Step 2: Persist entries and hard state BEFORE advancing
        if !entries_to_save.is_empty() {
            storage.append_entries(&entries_to_save).await?;
        }

        if let Some(hs) = &hard_state_opt {
            storage.save_hard_state(hs).await?;
        }

        // Step 3: Now advance (this tells raft-rs entries are persisted)
        let (light_messages, light_committed) = {
            let mut node = self.raw_node.write();
            let mut light_rd = node.advance(ready);
            let light_messages = light_rd.take_messages();
            let light_committed = light_rd.take_committed_entries();
            (light_messages, light_committed)
        };

        // Combine all messages and committed entries
        let total_messages = [messages, persisted_messages, light_messages].concat();
        let committed_entries = [ready_committed, light_committed].concat();

        // Apply committed entries
        for entry in committed_entries {
            if entry.data.is_empty() {
                // Skip empty/config entries
                continue;
            }

            if let Ok(proposal) = bincode::deserialize::<Proposal>(&entry.data) {
                let result = self.state_machine.apply(proposal).await;

                // Extract proposal_id from context to notify pending proposal
                if entry.context.len() >= 8 {
                    let proposal_id =
                        u64::from_le_bytes(entry.context[..8].try_into().unwrap_or([0; 8]));
                    if let Some(pending) = self.pending_proposals.write().remove(&proposal_id) {
                        let _ = pending.response_tx.send(result);
                    }
                }
            }
        }

        Ok(total_messages)
    }
}
