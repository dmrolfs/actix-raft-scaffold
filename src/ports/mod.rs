use actix::prelude::*;
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
pub struct PortData {
    pub fib: Addr<FibActor>,
    pub network: Addr<Network>,
}

impl std::fmt::Debug for PortData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "AppPortData") }
}
