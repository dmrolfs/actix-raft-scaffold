use actix::prelude::*;
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};
use thiserror::Error;
use crate::network::Network;

pub mod http;

#[derive(Error, Debug)]
pub enum PortError {
    #[error("failed to bind port endpoint")]
    BindError(#[from] std::io::Error),

    #[error("unknown port error")]
    Unknown,
}

pub struct PortData<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    pub network: Addr<Network<D, R, E, S>>,
}

impl<D, R, E, S> std::clone::Clone for PortData<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn clone(&self) -> Self {
        Self {
            network: self.network.clone(),
        }
    }
}

impl<D, R, E, S> std::fmt::Debug for PortData<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "PortData") }
}
