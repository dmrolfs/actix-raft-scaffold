use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use actix::prelude::*;
use actix_server::Server;
use actix_raft::{
    NodeId,
    metrics::RaftMetrics,
};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use thiserror::Error;
use tracing::*;
use crate::{
    Configuration,
    NodeInfo,
    ring::RingType,
    ports::{self, PortData},
    utils,
};
use super::{
    NetworkState,
    node::*
};

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("failed to bind network endpoint")]
    NetworkBindError(#[from] ports::PortError),

    #[error("unknown network error")]
    Unknown,
}

pub struct Network {
    id: NodeId,
    info: NodeInfo,
    state: NetworkState,
    discovery: SocketAddr,
    nodes: BTreeMap<NodeId, NodeRef>,
    nodes_connected: HashSet<NodeId>,
    pub isolated_nodes: HashSet<NodeId>,
    ring: RingType,
    metrics: Option<RaftMetrics>,
    server: Option<Server>,
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
            self.discovery,
        )
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Network(id:{})", self.id, )
    }
}

impl Network {
    pub fn new(
        id: NodeId,
        info: &NodeInfo,
        ring: RingType,
        discovery: SocketAddr,
    ) -> Self {
        Network {
            id,
            info: info.clone(),
            state: NetworkState::Initialized,
            discovery,
            nodes: BTreeMap::new(),
            nodes_connected: HashSet::new(),
            isolated_nodes: HashSet::new(),
            ring,
            metrics: None,
            server: None,
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn configure_with(&mut self, c: &Configuration) {
        for n in c.nodes.values() {
            let id = utils::generate_node_id(n.cluster_address.as_str());
            let node_ref = NodeRef {
                id,
                info: n.clone(),
                addr: None
            };
            info!(network_id = self.id, "configuring node ref:{:?}", node_ref);

            self.nodes.insert(id, node_ref);
        }
    }

    #[tracing::instrument(skip(self, _self_addr))]
    fn register_node(
        &mut self,
        node_id: NodeId,
        node_info: &NodeInfo,
        _self_addr: Addr<Self>
    ) -> Result<(), NetworkError> {
        let node_ref = if let Some(node_ref) = self.nodes.get_mut(&node_id) {
            node_ref
        } else {
            let node_ref = NodeRef {
                id: node_id,
                info: node_info.clone(),
                addr: None,
            };
            self.nodes.insert(node_id, node_ref);

            match self.nodes.get_mut(&node_id) {
                Some(nref) => nref,
                None => return Err(NetworkError::Unknown),
            }
        };
        debug!(network_id = self.id, "Registering node {:?}...", &node_ref);

        if node_ref.addr.is_none() {
            info!(network_id = self.id, "Starting node#{}...", node_ref.id);

            let node = Node::new(
                node_ref.id,
                self.id,
                node_ref.info.cluster_address.as_str()
            );

            node_ref.addr = Some(node.start());
        }

        self.restore_node(node_id);
        Ok(())
    }

    /// Isolate the network of the specified node.
    fn isolate_node(&mut self, id: NodeId) {
        if self.isolated_nodes.contains(&id) {
            info!("Network node already isolated {}.", &id);
        } else {
            info!("Isolating network for node {}.", &id);
            self.isolated_nodes.insert(id);
        }
    }

    /// Restore the network of the specified node.
    fn restore_node(&mut self, id: NodeId) {
        if self.isolated_nodes.contains(&id) {
            info!("Restoring network for node #{}", &id);
            self.isolated_nodes.remove(&id);
        }
    }
}

impl Actor for Network {
    type Context = Context<Self>;

    #[tracing::instrument(skip(self, ctx))]
    fn started(&mut self, ctx: &mut Self::Context) {
        //todo: create LocalNode for this id and push into nodes_connected;
        // info!("starting Network actor for node: {}", self.id);
        info!(network_id = self.id, "registering nodes configured with network...");
        let nodes = self.nodes.clone();
        for (node_id, node_ref) in nodes.iter() {
            self.register_node(*node_id, &node_ref.info, ctx.address());
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindEndpoint {
    data: PortData
}

impl BindEndpoint {
    pub fn new(data: PortData) -> Self { BindEndpoint { data } }
}

impl Message for BindEndpoint {
    type Result = Result<(), NetworkError>;
}

impl Handler<BindEndpoint> for Network {
    type Result = Result<(), NetworkError>;
    // type Result = ResponseActFuture<Self, (), NetworkError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, bind: BindEndpoint, ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "Network binding to http endpoint: {}...", self.info.cluster_address);
        let server = ports::http::start_server(self.info.cluster_address.as_str(), bind.data)?;
        info!(network_id = self.id, "Network http endpoint: {} started.", self.info.cluster_address);
        self.server = Some(server);
        Ok(())
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

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, msg: Join, ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "handling Join request...");
        let change = ChangeClusterConfig { to_add: vec![msg.id], to_remove: vec![], };
        //todo: find leader node
        //todo: send change to leader node
        //todo: and_then register_node( msg.id, msg.info, ctx.address() )
    }
}