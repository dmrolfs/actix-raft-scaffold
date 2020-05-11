use std::time::Duration;
use actix::prelude::*;
use actix_server::Server;
use actix_raft::{
    NodeId, Raft as ActixRaft,
    AppData, AppDataResponse, AppError, RaftStorage,
    config::{Config, SnapshotPolicy},
};
use tracing::*;
use crate::network::Network;
use crate::storage::StorageFactory;
use crate::ring::RingType;
use crate::{Configuration, ConfigurationError};

pub use self::builder::RaftBuilder;

pub mod network;
mod builder;

pub type Raft<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
= ActixRaft<D, R, E, Network<D>, S>;
