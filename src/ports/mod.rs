use actix::prelude::*;
use actix_raft::AppData;
use thiserror::Error;
use crate::network::Network;
use super::fib::FibActor;

pub mod http;

#[derive(Error, Debug)]
pub enum PortError {
    #[error("failed to bind port endpoint")]
    BindError(#[from] std::io::Error),

    #[error("unknown port error")]
    Unknown,
}

#[derive(Clone)]
pub struct PortData<D: AppData> {
    pub fib: Addr<FibActor>,
    pub network: Addr<Network<D>>,
}

impl<D: AppData> std::fmt::Debug for PortData<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "AppPortData") }
}
