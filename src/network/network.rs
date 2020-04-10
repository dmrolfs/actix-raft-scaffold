use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use actix::prelude::*;
use actix_raft::{
    NodeId,
    metrics::RaftMetrics,
};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tracing::*;
use crate::NodeInfo;
use crate::ring::RingType;
use super::node::*;

pub struct Network {
    id: NodeId,
    info: NodeInfo,
    discovery_socket_address: SocketAddr,
    nodes: BTreeMap<NodeId, Addr<Node>>,
    nodes_connected: HashSet<NodeId>,
    pub isolated_nodes: HashSet<NodeId>,
    ring: RingType,
    metrics: Option<RaftMetrics>,
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            // "Network(id:{:?}, state:{:?}, net_type:{:?}, info:{:?}, nodes:{:?}, isolated_nodes:{:?}, metrics:{:?})",
            "Network(id:{:?}, nodes:{:?}, isolated_nodes:{:?}, metrics:{:?}, discovery:{:?})",
            self.id,
            self.nodes_connected,
            self.isolated_nodes,
            self.metrics,
            self.discovery_socket_address,
        )
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Network(id:{:?}, nodes:{}, isolated_nodes:{}, metrics:{:?})",
            self.id,
            self.nodes_connected.len(),
            self.isolated_nodes.len(),
            self.metrics,
        )
    }
}

// discovery_socket_address: SocketAddr,
// nodes: BTreeMap<NodeId, Addr<Node>>,
// nodes_connected: HashSet<NodeId>,
// pub isolated_nodes: HashSet<NodeId>,
// ring: RingType,
// metrics: Option<RaftMetrics>,

impl Network {
    pub fn new<S>(
        id: NodeId,
        info: NodeInfo,
        ring: RingType,
        discovery_address: S,
    ) -> Self
    where
        S: AsRef<str> + std::fmt::Debug
    {
        let discovery = discovery_address.as_ref().parse::<SocketAddr>().unwrap();

        Network {
            id,
            info,
            discovery_socket_address: discovery,
            nodes: BTreeMap::new(),
            nodes_connected: HashSet::new(),
            isolated_nodes: HashSet::new(),
            ring,
            metrics: None,
        }
    }
}

impl Actor for Network {
    type Context = Context<Self>;

    #[tracing::instrument]
    fn started(&mut self, ctx: &mut Self::Context) {
        //todo: create LocalNode for this id and push into nodes_connected;
        info!("starting Network actor for node: {}", self.id);
    }
}

#[derive(Message, Debug)]
pub struct Join {
    pub id: NodeId,
    pub info: NodeInfo,
}

#[derive(Serialize, Deserialize, Message, Clone)]
pub struct ChangeClusterConfig {
    pub to_add: Vec<NodeId>,
    pub to_remove: Vec<NodeId>,
}

impl Handler<Join> for Network {
    type Result = ();

    #[tracing::instrument]
    fn handle(&mut self, msg: Join, ctx: &mut Self::Context) -> Self::Result {
        info!("{} handling Join request...", self);
        let change = ChangeClusterConfig { to_add: vec![msg.id], to_remove: vec![], };
        //todo: find leader node
        //todo: send change to leader node
        //todo: and_then register_node( msg.id, msg.info, ctx.address() )
    }
}