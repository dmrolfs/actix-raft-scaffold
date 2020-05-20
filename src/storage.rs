use actix::prelude::*;
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};

pub trait StorageFactory<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
    S: std::fmt::Debug,
{
    fn create(&self) -> Addr<<S as actix_raft::RaftStorage<D, R, E>>::Actor>;
}
