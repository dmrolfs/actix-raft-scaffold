use actix::prelude::*;
use actix_raft::{NodeId, RaftNetwork, AppData, messages as raft_protocol};
use tracing::*;

use crate::network::{Network, NetworkError};

pub mod node;

const ERR_ROUTING_FAILURE: &str = "Failed to send RCP to node target.";

impl<D: AppData> RaftNetwork<D> for Network<D> {}

impl<D: AppData> Handler<raft_protocol::AppendEntriesRequest<D>> for Network<D> {
    type Result = ResponseActFuture<Self, raft_protocol::AppendEntriesResponse, ()>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(
        &mut self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        if let Some(node) = self.get_node(msg.target) {
            if self.state.is_isolated_node(msg.target) ||
                self.state.is_isolated_node(msg.leader_id) {
                Box::new(fut::err::<raft_protocol::AppendEntriesResponse, (), Self>(()))
            } else {
                let target_id = msg.target;

                Box::new(
                    fut::wrap_future::<_, Self>(node.send(msg))
                        .map_err(move |err, _, _| {
                            error!(
                            node_id = target_id, error = ?err,
                            "Append Entries - {}", ERR_ROUTING_FAILURE
                        )
                        })
                        .and_then(|res, _, _| fut::result(res))
                )
            }
        } else {
            Box::new(fut::err::<raft_protocol::AppendEntriesResponse, (), Self>(()))
        }
    }
}

impl<D: AppData> Handler<raft_protocol::VoteRequest> for Network<D> {
    type Result = ResponseActFuture<Self, raft_protocol::VoteResponse, ()>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(
        &mut self,
        msg: raft_protocol::VoteRequest,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        if let Some(node) = self.get_node(msg.target) {
            if self.state.is_isolated_node(msg.target) || self.state.is_isolated_node(msg.candidate_id) {
                return Box::new(fut::err::<raft_protocol::VoteResponse, (), Self>(()));
            } else {
                return Box::new(
                    fut::wrap_future::<_, Self>(node.send(msg))
                        .map_err(|err, _, _| {
                            error!(
                            error = ?err,
                            "Vote Request - {}", ERR_ROUTING_FAILURE
                        );
                            ()
                        })
                        .and_then(|res, _, _| {
                            fut::result::<raft_protocol::VoteResponse, (), Self>(res)
                        })
                );
            }
        } else {
            return Box::new(fut::err::<raft_protocol::VoteResponse, (), Self>(()));
        }
    }
}

impl<D: AppData> Network<D> {
    fn ignore_in_isolation(
        &self,
        target_id: NodeId,
        leader_id: NodeId
    ) -> impl ActorFuture<Actor = Self, Item = (), Error = ()> {
        if !self.nodes.contains_key(&target_id) ||
            self.state.is_isolated_node(target_id) ||
            self.state.is_isolated_node(leader_id) {
                fut::err(())
            } else {
                fut::ok(())
            }
    }
}

impl<D: AppData> Handler<raft_protocol::InstallSnapshotRequest> for Network<D> {
    type Result = ResponseActFuture<Self, raft_protocol::InstallSnapshotResponse, ()>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(
        &mut self,
        msg: raft_protocol:: InstallSnapshotRequest,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        Box::new(
            self.ignore_in_isolation(msg.target, msg.leader_id)
                .and_then(|_, network, _| {
                    let target_id = msg.target;
                    let leader_id = msg.leader_id;

                    let node = network.get_node(msg.target).unwrap();
                    fut::wrap_future::<_, Self>(node.send(msg))
                        .map_err(move |err, _, _| {
                            error!(
                                node_id = target_id, leader_id, error = ?err,
                                "Install Snapshot - {}", ERR_ROUTING_FAILURE
                            )
                        })
                        .and_then(|res, _, _| fut::result(res))
                })
        )
    }
}