use std::collections::BTreeMap;
use std::time::Duration;
use actix::prelude::*;
use actix_raft::{
    NodeId,
    AppData, AppDataResponse, AppError,
    messages::{
        AppendEntriesRequest, AppendEntriesResponse,
        InstallSnapshotRequest, InstallSnapshotResponse,
        VoteRequest, VoteResponse,
    },
    network::RaftNetwork,
    metrics::{RaftMetrics, State},
};
use tracing::*;
use actix_raft_scaffold::raft::Raft;
use super::memory_storage::{MemoryStorageData, MemoryStorageResponse, MemoryStorageError, MemoryStorage};

pub type MemRaft = Raft<
    MemoryStorageData,
    MemoryStorageResponse,
    MemoryStorageError,
    MemoryStorage
>;


