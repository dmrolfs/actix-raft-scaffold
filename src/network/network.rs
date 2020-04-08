use std::collections::BTreeMap;
use actix::prelude::*;
use actix_raft::{
    NodeId,
    metrics::RaftMetrics,
};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tracing::*;
use crate::NodeInfo;

pub struct Network {
    id: NodeId,
    // nodes: BTreeMap<NodeId, Addr<Node>>,
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            // "Network(id:{:?}, state:{:?}, net_type:{:?}, info:{:?}, nodes:{:?}, isolated_nodes:{:?}, metrics:{:?})",
            "Network(id:{:?})",
            self.id,
            // self.state,
            // self.info,
            // self.net_type,
            // self.nodes_connected,
            // self.isolated_nodes,
            // self.metrics,
        )
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Network({})", self.id)
    }
}

impl Network {
    #[tracing::instrument]
    pub fn new(
        id: NodeId,
    ) -> Self {
        Network {
            id
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