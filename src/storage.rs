use actix::prelude::*;
use actix::dev::ToEnvelope;
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};

// pub trait Storage<D, R, E> : RaftStorage<D, R, E> + std::fmt::Debug
// where
//     D: AppData,
//     R: AppDataResponse,
//     E: AppError,
// {
//     // <Self as storage::RaftStorageType<D, R, E>>::Actor
//     type Actor: Actor<Context = <Self as Storage<D, R, E>>::Context> +
//         Handler<actix_raft::storage::GetInitialState<E>> +
//         Handler<actix_raft::storage::SaveHardState<E>> +
//         Handler<actix_raft::storage::GetLogEntries<D, E>> +
//         Handler<actix_raft::storage::AppendEntryToLog<D, E>> +
//         Handler<actix_raft::storage::ReplicateToLog<D, E>> +
//         Handler<actix_raft::storage::ApplyEntryToStateMachine<D, R, E>> +
//         Handler<actix_raft::storage::ReplicateToStateMachine<D, E>> +
//         Handler<actix_raft::storage::CreateSnapshot<E>> +
//         Handler<actix_raft::storage::InstallSnapshot<E>> +
//         Handler<actix_raft::storage::GetCurrentSnapshot<E>>;
//
//     type Context: ActorContext +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::GetInitialState<E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::SaveHardState<E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::GetLogEntries<D, E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::AppendEntryToLog<D, E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::ReplicateToLog<D, E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::ApplyEntryToStateMachine<D, R, E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::ReplicateToStateMachine<D, E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::CreateSnapshot<E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::InstallSnapshot<E>> +
//         ToEnvelope<<Self as Storage<D, R, E>>::Actor, actix_raft::storage::GetCurrentSnapshot<E>>;
// }

pub trait StorageFactory<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,

    S: RaftStorage<D, R, E>,
    S: std::fmt::Debug,
    // S: Actor<Context = Context<S>>,

    // S: Handler<actix_raft::storage::GetInitialState<E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::GetInitialState<E>>,
    //
    // S: Handler<actix_raft::storage::SaveHardState<E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::SaveHardState<E>>,
    //
    // S: Handler<actix_raft::storage::GetLogEntries<D, E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::GetLogEntries<D, E>>,
    //
    // S: Handler<actix_raft::storage::AppendEntryToLog<D, E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::AppendEntryToLog<D, E>>,
    //
    // S: Handler<actix_raft::storage::ReplicateToLog<D, E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::ReplicateToLog<D, E>>,
    //
    // S: Handler<actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
    //
    // S: Handler<actix_raft::storage::ReplicateToStateMachine<D, E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::ReplicateToStateMachine<D, E>>,
    //
    // S: Handler<actix_raft::storage::CreateSnapshot<E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::CreateSnapshot<E>>,
    //
    // S: Handler<actix_raft::storage::InstallSnapshot<E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::InstallSnapshot<E>>,
    //
    // S: Handler<actix_raft::storage::GetCurrentSnapshot<E>>,
    // S::Context: ToEnvelope<S, actix_raft::storage::GetCurrentSnapshot<E>>,
{
    fn create(&self) -> Addr<<S as actix_raft::RaftStorage<D, R, E>>::Actor>;
}
//todo: see raftor-dissection::raft/mod.rs ln68