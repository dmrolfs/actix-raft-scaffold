use actix::prelude::*;
use actix_web::client::Client;
use actix_raft::NodeId;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tracing::*;
use crate::NodeInfo;


#[derive(Debug, PartialEq)]
enum Status {
    Registered,
    Connected,
}

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("failed to change cluster {to_add:?} and {to_remove:?}")]
    ChangeClusterError {
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
    },

    #[error("unknown node error")]
    Unknown,
}

pub struct NodeRef {
    pub id: NodeId,
    pub info: NodeInfo,
    pub addr: Option<Addr<Node>>,
}

impl std::fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeRef(id:{}, info:{:?})", self.id, self.info)
    }
}

trait ChangeCluster {
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError>;
}

trait ProximityBehavior : ChangeCluster + std::fmt::Display { }

#[derive(Debug)]
struct LocalNode;

impl LocalNode {
    #[tracing::instrument]
    fn new() -> LocalNode { LocalNode {} }
}

impl std::fmt::Display for LocalNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Local") }
}

impl ProximityBehavior for LocalNode {}

impl ChangeCluster for LocalNode {
    #[tracing::instrument]
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError> {
        let node_addr = ctx.address();

        unimplemented!()
    }
}


struct RemoteNode {
    client: Client,
    scope: String,
}

impl RemoteNode {
    #[tracing::instrument]
    fn new<S>(discovery_address: S) -> RemoteNode
        where
            S: AsRef<str> + std::fmt::Debug
    {
        //todo: use encryption
        let scope = format!("http://{}/api/cluster", discovery_address.as_ref());

        RemoteNode {
            client: Client::default(),
            scope,
        }
    }
}

impl std::fmt::Display for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode(to:{})", self.scope)
    }
}

impl std::fmt::Debug for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode(to:{})", self.scope)
    }
}

impl ProximityBehavior for RemoteNode {}

impl ChangeCluster for RemoteNode {
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

pub struct Node {
    id: NodeId, // id of the node (remote or local)
    local_id: NodeId, // id local to the machine of the node this instance exists
    proximity: Box<dyn ProximityBehavior>,
    status: Status,
}

impl Node {
    #[tracing::instrument]
    pub fn new<S>(
        id: NodeId,
        local_id: NodeId,
        discovery_address: S,
    ) -> Self
    where
        S: AsRef<str> + std::fmt::Debug
    {
        let proximity = Node::determine_proximity(id, local_id, discovery_address);

        info!("Registering NODE id:{} @ {} => {}", id, local_id, proximity);

        Node {
            id,
            local_id,
            proximity,
            status: Status::Registered,
        }
    }

    #[tracing::instrument]
    fn determine_proximity<S>(
        node_id: NodeId,
        local_id: NodeId,
        discovery_address: S
    ) -> Box<dyn ProximityBehavior>
    where
        S: AsRef<str> + std::fmt::Debug
    {
        if node_id == local_id {
            Box::new(LocalNode::new())
        } else {
            Box::new(RemoteNode::new(discovery_address))
        }
    }

    // fn handle_change_cluster_config<P>(proximity: &P, to_add: Vec<NodeId>, to_remove: Vec<NodeId>)
    //     where
    //         P: ChangeCluster
    // {
    //     proximity.change_cluster_config(to_add, to_remove, ctx);
    // }
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Node(id:{}, proximity:{}, status:{:?})",
            self.id, self.proximity, self.status
        )
    }
}

impl Actor for Node {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Node #{} is connecting.", self.id);
        unimplemented!()
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        unimplemented!()
    }
}

#[derive(Debug)]
struct ChangeClusterConfig {
    to_add: Vec<NodeId>,
    to_remove: Vec<NodeId>,
}

impl Message for ChangeClusterConfig {
    type Result = Result<(), NodeError>;
}

impl Handler<ChangeClusterConfig> for Node {
    type Result = Result<(), NodeError>;

    #[tracing::instrument]
    fn handle(&mut self, msg: ChangeClusterConfig, ctx: &mut Self::Context) -> Self::Result {
        self.proximity.change_cluster_config(msg.to_add, msg.to_remove, ctx)
    }
}