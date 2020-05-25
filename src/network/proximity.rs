use std::fmt::{Debug, Display};
use actix::prelude::*;
use actix_raft::NodeId;
use actix_raft::AppData;
use crate::NodeInfo;
use super::node::{Node, NodeError};
use crate::network::messages;


//todo: Change to be asynchronous.
pub trait ChangeClusterBehavior<D: AppData> {
    fn change_cluster_config(
        &self,
        add_members: Vec<NodeId>,
        remove_members: Vec<NodeId>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<(), NodeError>;
}

pub trait ConnectionBehavior<D: AppData> {
    //todo: make nonblocking because of distributed network calls
    fn connect(
        &self,
        local_id_info: (NodeId, &NodeInfo),
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<messages::Acknowledged, NodeError>;

    //todo: make nonblocking because of distributed network calls
    fn disconnect(&self, _ctx: &mut <Node<D> as Actor>::Context) -> Result<(), NodeError>;
}


pub trait ProximityBehavior<D: AppData> :
ChangeClusterBehavior<D> +
ConnectionBehavior<D> +
crate::raft::network::proximity::RaftProtocolBehavior<D> +
Debug + Display
{ }
