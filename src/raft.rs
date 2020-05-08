use std::time::Duration;
use actix::prelude::*;
use actix_raft::{
    NodeId, Raft,
    AppData, AppDataResponse, AppError, RaftStorage,
    config::{Config, SnapshotPolicy},
};
use crate::network::Network;

pub mod network;

pub type ScaffoldRaft<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
= Raft<D, R, E, Network<D>, S>;