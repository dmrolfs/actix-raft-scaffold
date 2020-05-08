use std::collections::BTreeMap;
use std::time::Duration;
use actix::prelude::*;
use actix_raft::{
    Raft, NodeId,
    messages::{
        AppendEntriesRequest, AppendEntriesResponse,
        InstallSnapshotRequest, InstallSnapshotResponse,
        VoteRequest, VoteResponse,
    },
    network::RaftNetwork,
    metrics::{RaftMetrics, State},
};
use tracing::*;
use actix_raft_scaffold::network::Network;
use super::memory_storage::{Data, MemoryStorageResponse, MemoryStorageError, MemoryStorage};

pub type MemRaft = Raft<
    Data,
    MemoryStorageResponse,
    MemoryStorageError,
    Network<Data>,
    MemoryStorage
>;


