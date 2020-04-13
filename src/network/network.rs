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

    #[tracing::instrument]
    pub fn configure_with(&mut self, c: &Configuration) {
        for n in c.nodes.values() {
            let id = utils::generate_node_id(n.cluster_address.as_str());
            let node_ref = NodeRef {
                id,
                info: n.clone(),
                addr: None
            };
            info!("configuring node ref:{:?}", node_ref);

            self.nodes.insert(id, node_ref);
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

    fn handle(&mut self, bind: BindEndpoint, ctx: &mut Self::Context) -> Self::Result {
        info!("{} binding to http endpoint: {}...", self, self.info.cluster_address);
        let server = ports::http::start_server(self.info.cluster_address.as_str(), bind.data)?;
        info!("{} http endpoint: {} started.", self, self.info.cluster_address);
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

    #[tracing::instrument]
    fn handle(&mut self, msg: Join, ctx: &mut Self::Context) -> Self::Result {
        info!("handling Join request...");
        let change = ChangeClusterConfig { to_add: vec![msg.id], to_remove: vec![], };
        //todo: find leader node
        //todo: send change to leader node
        //todo: and_then register_node( msg.id, msg.info, ctx.address() )
    }
}