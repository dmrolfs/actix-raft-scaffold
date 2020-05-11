use actix::prelude::*;
use actix::dev::ToEnvelope;
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};

pub trait RaftStorageType<D, R, E> : RaftStorage<D, R, E>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
{
    // <Self as storage::RaftStorageType<D, R, E>>::Actor
    type Actor: Actor<Context = <Self as RaftStorageType<D, R, E>>::Context> +
        Handler<actix_raft::storage::GetInitialState<E>> +
        Handler<actix_raft::storage::SaveHardState<E>> +
        Handler<actix_raft::storage::GetLogEntries<D, E>> +
        Handler<actix_raft::storage::AppendEntryToLog<D, E>> +
        Handler<actix_raft::storage::ReplicateToLog<D, E>> +
        Handler<actix_raft::storage::ApplyEntryToStateMachine<D, R, E>> +
        Handler<actix_raft::storage::ReplicateToStateMachine<D, E>> +
        Handler<actix_raft::storage::CreateSnapshot<E>> +
        Handler<actix_raft::storage::InstallSnapshot<E>> +
        Handler<actix_raft::storage::GetCurrentSnapshot<E>>;

    type Context: ActorContext +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::GetInitialState<E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::SaveHardState<E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::GetLogEntries<D, E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::AppendEntryToLog<D, E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::ReplicateToLog<D, E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::ApplyEntryToStateMachine<D, R, E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::ReplicateToStateMachine<D, E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::CreateSnapshot<E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::InstallSnapshot<E>> +
        actix::dev::ToEnvelope<<Self as RaftStorageType<D, R, E>>::Actor, actix_raft::storage::GetCurrentSnapshot<E>>;
}

pub trait StorageFactory<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorageType<D, R, E>,
    S: Actor<Context = Context<S>>,
    S: Handler<actix_raft::storage::GetInitialState<E>>,
    S: Handler<actix_raft::storage::SaveHardState<E>>,
    S: Handler<actix_raft::storage::GetLogEntries<D, E>>,
    S: Handler<actix_raft::storage::AppendEntryToLog<D, E>>,
    S: Handler<actix_raft::storage::ReplicateToLog<D, E>>,
    S: Handler<actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
    S: Handler<actix_raft::storage::ReplicateToStateMachine<D, E>>,
    S: Handler<actix_raft::storage::CreateSnapshot<E>>,
    S: Handler<actix_raft::storage::InstallSnapshot<E>>,
    S: Handler<actix_raft::storage::GetCurrentSnapshot<E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::GetInitialState<E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::SaveHardState<E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::GetLogEntries<D, E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::AppendEntryToLog<D, E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::ReplicateToLog<D, E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::ReplicateToStateMachine<D, E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::CreateSnapshot<E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::InstallSnapshot<E>>,
    S: actix::dev::ToEnvelope<S, actix_raft::storage::GetCurrentSnapshot<E>>,



    // S: RaftStorage<D, R, E> + Actor<Context = Context<S>>,
{
    fn create(&self) -> Addr<<S as actix_raft::storage::RaftStorage<D, R, E>>::Actor>;
}
//todo: see raftor-dissection::raft/mod.rs ln68