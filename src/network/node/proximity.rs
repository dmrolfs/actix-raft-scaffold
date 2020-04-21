use std::fmt::{Debug, Display};
use actix::prelude::*;
use actix_raft::NodeId;
use actix_web::client::Client;
use crate::NodeInfo;
use super::{Node, NodeError};

pub trait ChangeClusterBehavior {
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError>;
}

pub trait ConnectionBehavior {
    #[tracing::instrument(skip(self, _ctx))]
    fn connect(&mut self, local_id: NodeId, local_info: &NodeInfo, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        Ok(())
    }

    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&mut self, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        Ok(())
    }
}


pub trait ProximityBehavior :
ChangeClusterBehavior +
ConnectionBehavior +
Debug + Display
{ }


#[derive(Debug)]
pub struct LocalNode;

impl LocalNode {
    #[tracing::instrument]
    pub fn new() -> LocalNode { LocalNode {} }
}

impl Display for LocalNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "LocalNode") }
}

impl ProximityBehavior for LocalNode {}

impl ChangeClusterBehavior for LocalNode {
    #[tracing::instrument]
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError> {
        let _node_addr = ctx.address();

        unimplemented!()
    }
}

impl ConnectionBehavior for LocalNode {}

pub struct RemoteNode {
    client: Client,
    scope: String,
}

impl RemoteNode {
    #[tracing::instrument]
    pub fn new<S: AsRef<str> + Debug>(discovery_address: S) -> RemoteNode {
        //todo: use encryption
        let scope = format!("http://{}/api/cluster", discovery_address.as_ref());

        RemoteNode {
            client: Client::default(),
            scope,
        }
    }
}

impl Display for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode(to:{})", self.scope)
    }
}

impl Debug for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode(to:{})", self.scope)
    }
}

impl ProximityBehavior for RemoteNode {}

impl ChangeClusterBehavior for RemoteNode {
    #[tracing::instrument]
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError> {

        // client.put(cluster_nodes_route)
        //     .header("Content-Type", "application/json")
        //     .send_json(&act.id)
        unimplemented!()
    }
}

impl ConnectionBehavior for RemoteNode {}
