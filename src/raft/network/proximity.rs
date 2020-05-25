use actix::prelude::*;
use actix_raft::AppData;
use actix_raft::messages as raft_protocol;
use crate::network::node::Node;

pub trait RaftProtocolBehavior<D: AppData> {
    fn append_entries(
        &self,
        rpc: raft_protocol::AppendEntriesRequest<D>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::AppendEntriesResponse, Error = ()>>;
    fn install_snapshot(
        &self,
        rpc: raft_protocol::InstallSnapshotRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::InstallSnapshotResponse, Error = ()>>;

    fn vote(
        &self,
        rpc: raft_protocol::VoteRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn ActorFuture<Actor = Node<D>, Item = raft_protocol::VoteResponse, Error = ()>>;
}
