use actix::prelude::*;
use actix_raft::AppData;
use actix_raft::messages as raft_protocol;
use serde::{Serialize, Deserialize};
use tracing::*;
use crate::network::node::{Node, RemoteNode};
use crate::raft::network::proximity::RaftProtocolBehavior;
use crate::ports::http::entities as port_entities;

impl<D: AppData> RaftProtocolBehavior<D> for RemoteNode {
    #[tracing::instrument(skip(self, rpc, ctx))]
    fn append_entries(
        &self,
        rpc: raft_protocol::AppendEntriesRequest<D>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::AppendEntriesResponse, Error = ()>> {
        self.send_raft_command::<
            D,
            raft_protocol::AppendEntriesRequest<D>,
            raft_protocol::AppendEntriesResponse,
            port_entities::RaftAppendEntriesRequest,
            port_entities::RaftAppendEntriesResponse
        >(rpc, "entries", ctx)
    }

    #[tracing::instrument(skip(self, rpc, ctx))]
    fn install_snapshot(
        &self,
        rpc: raft_protocol::InstallSnapshotRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::InstallSnapshotResponse, Error = ()>> {
        self.send_raft_command::<
            D,
            raft_protocol::InstallSnapshotRequest,
            raft_protocol::InstallSnapshotResponse,
            port_entities::RaftInstallSnapshotRequest,
            port_entities::RaftInstallSnapshotResponse
        >(rpc, "snapshots", ctx)
    }

    #[tracing::instrument(skip(self, rpc, ctx))]
    fn vote(
        &self,
        rpc: raft_protocol::VoteRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::VoteResponse, Error = ()>> {
        self.send_raft_command::<
            D,
            raft_protocol::VoteRequest,
            raft_protocol::VoteResponse,
            port_entities::RaftVoteRequest,
            port_entities::RaftVoteResponse
        >(rpc, "vote", ctx)
    }
}

impl RemoteNode {
    #[tracing::instrument(skip(self, _ctx))]
    fn send_raft_command<D, M, R, M0, R0>(
        &self,
        command: M,
        path: &str,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = R, Error = ()>>
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
        debug!(proximity = ?self, ?command, %route, "send Raft RPC to remote node.");

        let body: M0 = command.into();

        let task = self.client
            .post(&route)
            .json(&body)
            .send()
            .and_then(|mut resp| resp.json::<R0>())
            .map_err(|err| {
                //todo handle redirect, which indicates leader, but this should be reflected in raft_metrics and adjusted via consensus
                self.log_raft_protocol_error(err);
                ()
            })
            .and_then(|resp| Ok(resp.into()));

        Box::new(fut::result(task))
    }

    fn log_raft_protocol_error(&self, error: reqwest::Error) {
        match error {
            e if e.is_timeout() => {
                warn!(proximity = ?self, error = ?e, "Raft RPC to remote timed out.");
            },

            e if e.is_serialization() => {
                error!(proximity = ?self, error = ?e, "Raft RPC serialization failure.");
                panic!(e); //todo: panic to identify intermittent test error log that I think occurs on failing to parse json response.
            },

            // e if e.is_timeout() => NodeError::Timeout(e.to_string()),
            // e if e.is_client_error() => NodeError::RequestError(e),
            // e if e.is_serialization() => NodeError::ResponseFailure(e.to_string()),
            // e if e.is_http() => NodeError::RequestError(e),
            // e if e.is_server_error() => {},
            // e if e.is_redirect() => {

            e => {
                error!(proximity = ?self, error = ?e, "Raft RPC to remote failed");
            },
        }
    }
}
