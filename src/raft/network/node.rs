use actix::prelude::*;
use actix_raft::{AppData, RaftNetwork, messages as raft_protocol};
use crate::network::node::Node;
use tracing::*;


mod local;
mod remote;

impl<D: AppData> RaftNetwork<D> for Node<D> {}

impl<D: AppData> Handler<raft_protocol::AppendEntriesRequest<D>> for Node<D> {
    type Result = ResponseActFuture<Self, raft_protocol::AppendEntriesResponse, ()>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(
        &mut self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        ctx: &mut Self::Context
    ) -> Self::Result {
        debug!(
            local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
            "delegating Raft command to proximity."
        );

        Box::new(
            self.proximity.append_entries(msg, ctx)
                .map_err(|err, node, _| {
                    error!(
                        local_id = node.local_id, node_id = node.id,
                        proximity = ?node.proximity, error = ?err,
                        "Raft append entries failed."
                    );
                    ()
                })
        )
    }
}

impl<D: AppData> Handler<raft_protocol::InstallSnapshotRequest> for Node<D> {
    type Result = ResponseActFuture<Self, raft_protocol::InstallSnapshotResponse, ()>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(
        &mut self,
        msg: raft_protocol::InstallSnapshotRequest,
        ctx: &mut Self::Context
    ) -> Self::Result {
        debug!(
            local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
            "delegating Raft command to proximity."
        );

        Box::new(
            self.proximity.install_snapshot(msg, ctx)
                .map_err(|err, node, _| {
                    error!(
                        local_id = node.local_id, node_id = node.id,
                        proximity = ?node.proximity, error = ?err,
                        "Raft install snapshot failed."
                    );
                    ()
                })
        )
    }
}

impl<D: AppData> Handler<raft_protocol::VoteRequest> for Node<D> {
    type Result = ResponseActFuture<Self, raft_protocol::VoteResponse, ()>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(
        &mut self,
        msg: raft_protocol::VoteRequest,
        ctx: &mut Self::Context
    ) -> Self::Result {
        debug!(
            local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
            "delegating Raft command to proximity."
        );

        Box::new(
            self.proximity.vote(msg, ctx)
                .map_err(|err, node, _| {
                    error!(
                        local_id = node.local_id, node_id = node.id,
                        proximity = ?node.proximity, error = ?err,
                        "Raft vote failed."
                    );
                    ()
                })
        )
    }
}
