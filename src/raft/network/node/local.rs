use actix::prelude::*;
use actix_raft::{AppData, AppDataResponse, AppError, storage::RaftStorage};
use actix_raft::messages as raft_protocol;
use tracing::*;
use crate::raft::Raft;
use crate::network::node::{Node, LocalNode};
use crate::raft::network::proximity::RaftProtocolBehavior;

impl<D, R, E, S> RaftProtocolBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(self, rpc, _ctx))]
    fn append_entries(
        &self,
        rpc: raft_protocol::AppendEntriesRequest<D>,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::AppendEntriesResponse, Error = ()>> {
        Box::new(
            fut::wrap_future::<_, Node<D>>(self.submit_to_raft(rpc))
                .and_then(|res, _, _| fut::result(res))
        )
        // debug!(
        //     proximity = ?self, raft_command = "AppendEntries",
        //     "Submitting Raft RPC to local Raft."
        // );
        //
        // Box::new(
        //     fut::wrap_future::<_, Node<D>>(self.raft.send(msg))
        //         .map_err(|err, n, _| {
        //             error!(
        //                 local_id = n.local_id, proximity = ?n.proximity, error = ?err,
        //                 "Raft AppendEntries RPC failed in actor send."
        //             );
        //             ()
        //         })
        //         .and_then(|res, _, _| fut::result(res))
        // )
    }

    #[tracing::instrument(skip(self, rpc, _ctx))]
    fn install_snapshot(
        &self,
        rpc: raft_protocol::InstallSnapshotRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::InstallSnapshotResponse, Error = ()>> {
        Box::new(
            fut::wrap_future::<_, Node<D>>(self.submit_to_raft(rpc))
                .and_then(|res, _, _| fut::result(res))
        )
        // debug!(
        //     proximity = ?self, raft_command = "InstallSnapshot",
        //     "Submitting Raft RPC to local Raft."
        // );
        //
        // Box::new(
        //     fut::wrap_future::<_, Node<D>>(self.raft.send(rpc))
        //         .map_err(|err, n, _| {
        //             error!(
        //                 local_id = n.local_id, proximity = ?n.proximity, error = ?err,
        //                 "Raft AppendEntries RPC failed in actor send."
        //             );
        //             ()
        //         })
        //         .and_then(|res, _, _| fut::result(res))
        // )
    }

    #[tracing::instrument(skip(self, rpc, _ctx))]
    fn vote(
        &self,
        rpc: raft_protocol::VoteRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::VoteResponse, Error = ()>> {
        Box::new(
            fut::wrap_future::<_, Node<D>>(self.submit_to_raft(rpc))
                .and_then(|res, _, _| fut::result(res))
        )
        // debug!(
        //     proximity = ?self, raft_command = "VoteRequest",
        //     "Submitting Raft RPC to local Raft."
        // );
        //
        // Box::new(
        //     fut::wrap_future::<_, Node<D>>(self.raft.send(rpc))
        //         .map_err(|err, n, _| {
        //             error!(
        //                 local_id = n.local_id, proximity = ?n.proximity, error = ?err,
        //                 "Raft VoteRequest RPC failed in actor send."
        //             );
        //             ()
        //         })
        //         .and_then(|res, _, _| fut::result(res))
        // )
    }
}

impl<D, R, E, S> LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(self))]
    fn submit_to_raft<C>(&self, rpc: C) -> Box<dyn Future<Item = C::Result, Error = ()>>
        where
            C: Message + Send + std::fmt::Debug + 'static,
            C::Result: Send,
            Raft<D, R, E, S>: Handler<C>,
            <Raft<D, R, E, S> as Actor>::Context: actix::dev::ToEnvelope<Raft<D, R, E, S>, C>
    {
        let local_id = self.id;
        let proximity = self.clone();
        debug!(?proximity, raft_rpc = ?rpc, "Submitting Raft RPC to local Raft.");

        Box::new(
            self.raft.send(rpc)
                .map_err(move |err| {
                    error!(local_id, ?proximity, error = ?err,"Raft RPC failed in actor send.");
                    ()
                })
        )
    }
}

