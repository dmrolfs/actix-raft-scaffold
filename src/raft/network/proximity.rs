use actix::prelude::*;
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};
use actix_raft::messages as raft_protocol;
use tracing::*;
use crate::network::node::{Node, LocalNode, RemoteNode};
use crate::ports::http::entities as port_entities;
use serde::{Serialize, Deserialize};

pub trait RaftProtocolBehavior<D: AppData> {
    fn append_entries(
        &self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::AppendEntriesResponse, Error = ()>>;
                           // Box<dyn Future<Item = raft_protocol::AppendEntriesResponse, Error = ()>>;

    fn install_snapshot(
        &self,
        msg: raft_protocol::InstallSnapshotRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::InstallSnapshotResponse, Error = ()>>;
                             // Box<dyn Future<Item = raft_protocol::InstallSnapshotResponse, Error = ()>>;

    fn vote(
        &self,
        msg: raft_protocol::VoteRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::VoteResponse, Error = ()>>;
                 // Box<dyn Future<Item = raft_protocol::VoteResponse, Error = ()>>;
}


impl<D, R, E, S> RaftProtocolBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(self, msg, _ctx))]
    fn append_entries(
        &self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::AppendEntriesResponse, Error = ()>> {
                           // Box<dyn Future<Item = raft_protocol::AppendEntriesResponse, Error = ()>> {
        debug!(
            proximity = ?self, raft_command = "AppendEntries",
            "Submitting Raft RPC to local Raft."
        );

        Box::new(
            fut::wrap_future::<_, Node<D>>(self.raft.send(msg))
                .map_err(|err, n, _| {
                    error!(
                        local_id = n.local_id, proximity = ?n.proximity, error = ?err,
                        "Raft AppendEntries RPC failed in actor send."
                    );
                    ()
                })
                .and_then(|res, _, _| fut::result(res))
        )
    }

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn install_snapshot(
        &self,
        msg: raft_protocol::InstallSnapshotRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::InstallSnapshotResponse, Error = ()>> {
                             // Box<dyn Future<Item = raft_protocol::InstallSnapshotResponse, Error = ()>> {
        debug!(
            proximity = ?self, raft_command = "InstallSnapshot",
            "Submitting Raft RPC to local Raft."
        );

        Box::new(
            fut::wrap_future::<_, Node<D>>(self.raft.send(msg))
                .map_err(|err, n, _| {
                    error!(
                        local_id = n.local_id, proximity = ?n.proximity, error = ?err,
                        "Raft AppendEntries RPC failed in actor send."
                    );
                    ()
                })
                .and_then(|res, _, _| fut::result(res))
        )
    }

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn vote(
        &self,
        msg: raft_protocol::VoteRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::VoteResponse, Error = ()>> {
                 // Box<dyn Future<Item = raft_protocol::VoteResponse, Error = ()>> {
        debug!(
            proximity = ?self, raft_command = "VoteRequest",
            "Submitting Raft RPC to local Raft."
        );

        Box::new(
            fut::wrap_future::<_, Node<D>>(self.raft.send(msg))
                .map_err(|err, n, _| {
                    error!(
                        local_id = n.local_id, proximity = ?n.proximity, error = ?err,
                        "Raft VoteRequest RPC failed in actor send."
                    );
                    ()
                })
                .and_then(|res, _, _| fut::result(res))
        )

        // self.raft.send(msg)
        //         .map_err(|err| {
        //             error!(error = ?err, "VoteRequest failed in actor mailbox");
        //             ()
        //         })
        //         .and_then(|res| futures::future::result(res))
        // )
    }
}


impl<D: AppData> RaftProtocolBehavior<D> for RemoteNode {
    #[tracing::instrument(skip(self, msg, ctx))]
    fn append_entries(
        &self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::AppendEntriesResponse, Error = ()>> {
                           // Box<dyn Future<Item = raft_protocol::AppendEntriesResponse, Error = ()>> {
        self.send_raft_command::<
            D,
            raft_protocol::AppendEntriesRequest<D>,
            raft_protocol::AppendEntriesResponse,
            port_entities::RaftAppendEntriesRequest,
            port_entities::RaftAppendEntriesResponse
        >(msg, "entries", ctx)
    }

    #[tracing::instrument(skip(self, msg, ctx))]
    fn install_snapshot(
        &self,
        msg: raft_protocol::InstallSnapshotRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::InstallSnapshotResponse, Error = ()>> {
                             // Box<dyn Future<Item = raft_protocol::InstallSnapshotResponse, Error = ()>> {
        self.send_raft_command::<
            D,
            raft_protocol::InstallSnapshotRequest,
            raft_protocol::InstallSnapshotResponse,
            port_entities::RaftInstallSnapshotRequest,
            port_entities::RaftInstallSnapshotResponse
        >(msg, "snapshots", ctx)
    }

    #[tracing::instrument(skip(self, msg, ctx))]
    fn vote(
        &self,
        msg: raft_protocol::VoteRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::VoteResponse, Error = ()>> {
                 // Box<dyn Future<Item = raft_protocol::VoteResponse, Error = ()>> {
        self.send_raft_command::<
            D,
            raft_protocol::VoteRequest,
            raft_protocol::VoteResponse,
            port_entities::RaftVoteRequest,
            port_entities::RaftVoteResponse
        >(msg, "vote", ctx)
    }
}

impl RemoteNode {
    #[tracing::instrument(skip(self, ctx))]
    fn send_raft_command<D, M, R, M0, R0>(
        &self,
        command: M,
        path: &str,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = R, Error = ()>>
                                               // Box<dyn Future<Item = R, Error = ()>>
    where
        D: AppData,
        M: Message + std::fmt::Debug,
        M: Into<M0>,
        R: 'static,
        M0: Serialize,
        R0: for<'de> Deserialize<'de>,
        R0: Into<R>,
    {
        let route = format!("{}/{}", self.scope(), path);
        debug!(
            proximity = ?self, ?command,
            "send Raft RPC to remote node."
        );

        let body: M0 = command.into();

        let task = self.client
            .post(&route)
            .json(&body)
            .send()
            .and_then(|mut resp| resp.json::<R0>())
            .map_err(|err| {
                self.log_raft_protocol_error(err);
                ()
            })
            .and_then(|resp| Ok(resp.into()));

        Box::new(fut::result(task))
        // Box::new(futures::future::result(task))
    }

    fn log_raft_protocol_error(&self, error: reqwest::Error) {
        match error {
            e if e.is_timeout() => {
                warn!(proximity = ?self, error = ?e, "Raft RPC to remote timed out.");
            },

            e => {
                error!(proximity = ?self, error = ?e, "Raft RPC to remote failed");
            },

            // e if e.is_serialization() => {
            //     error!(proximity = ?self, error = ?e, "VoteRequest serialization failure.");
            // }

            // e if e.is_timeout() => NodeError::Timeout(e.to_string()),
            // e if e.is_serialization() => NodeError::ResponseFailure(e.to_string()),
            // e if e.is_client_error() => NodeError::RequestError(e),
            // e if e.is_http() => NodeError::RequestError(e),
            // e if e.is_server_error() => {},
            // e if e.is_redirect() => {
        }
    }
}