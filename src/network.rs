use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use std::time::{Instant, Duration};
use actix::prelude::*;
use actix_server::Server;
use actix_raft::{
    NodeId,
    metrics::RaftMetrics,
};
use tokio::timer::Delay;
use serde::{Serialize, Deserialize};
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

    #[error("error in delay time {0}")]
    DelayError(#[from] tokio::timer::Error),

    #[error("error in actor mailbox {0}")]
    ActorMailBoxError(#[from] actix::MailboxError),

    #[error("unknown network error {0}")]
    Unknown(String),
}


// impl Network {
//     fn self_send_after_delay<M>(&mut self, msg: M, delay: Duration, ctx: &mut Self::Context) -> actix::MessageResponse<Self, M>
//     where
//         M: Message,
//     {
//         Box::new(
//             fut::wrap_future::<_, Self>( Delay::new(Instant::now() + delay))
//                 .map_err(|err, _, _| NetworkError::from(err))
//                 .and_then( move |_, _, ctx| {
//                     fut::wrap_future::<_, Self>(ctx.address().send(msg))
//                         .map_err(|err, _, _| NetworkError::from(err))
//                         .and_then(|res, _, _| fut::result(res))
//                 })
//         )
//     }
// }

pub struct Network {
    pub id: NodeId,
    pub info: NodeInfo,
    pub state: NetworkState,
    pub discovery: SocketAddr,
    pub nodes: BTreeMap<NodeId, NodeRef>,
    pub nodes_connected: HashSet<NodeId>,
    pub isolated_nodes: HashSet<NodeId>,
    pub ring: RingType,
    pub metrics: Option<RaftMetrics>,
    pub server: Option<Server>,
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
                None => {
                    let err_msg = format!("No node#{} registered in Network#{}.", node_id, self.id);
                    return Err(NetworkError::Unknown(err_msg))
                },
            }
        };
        debug!(network_id = self.id, "Registering node {:?}...", &node_ref);

        if node_ref.addr.is_none() {
            info!(network_id = self.id, "Starting node#{}...", node_ref.id);

            let node = Node::new(
                node_ref.id,
                self.id,
                node_ref.info.cluster_address.as_str(),
                self.info.clone(),
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
    #[tracing::instrument(skip(self))]
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
            self.register_node(*node_id, &node_ref.info, ctx.address()).unwrap();
        }
    }
}

impl Handler<RaftMetrics> for Network {
    type Result = ();

    fn handle(&mut self, msg: RaftMetrics, _ctx: &mut Self::Context) -> Self::Result {
        debug!(
            network_id = self.id,
            "RAFT node={} state={:?} leader={:?} term={} index={} applied={} cfg={{join={} members={:?} non_voters={:?} removing={:?}}}",
            msg.id, msg.state, msg.current_leader, msg.current_term, msg.last_log_index,
            msg.last_applied, msg.membership_config.is_in_joint_consensus,
            msg.membership_config.members, msg.membership_config.non_voters,
            msg.membership_config.removing,
        );

        self.metrics = Some(msg);
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

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, bind: BindEndpoint, _ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "Network binding to http endpoint: {}...", self.info.cluster_address);
        let server = ports::http::start_server(self.info.cluster_address.as_str(), bind.data)?;
        info!(network_id = self.id, "Network http endpoint: {} started.", self.info.cluster_address);
        self.server = Some(server);
        Ok(())
    }
}

// #[derive(Debug)]
// pub struct GetNode{
// cluster_address or node_id???
//     pub node_id: NodeId,
// }
//
// impl GetNode {
//     pub fn new(id: NodeId) -> Self { GetNode { node_id: id, }
// }
//
// impl Message for GetNode {
//     type Result = Result<(NodeId, String), NetworkError>;
// }
//
// impl Handler<GetNode> for Network {
//     type Result = Result<(NodeId, String), NetworkError>;
//
//     fn handle(&mut self, msg: GetNode, ctx: &mut Self::Context) -> Self::Result {
//         let ring = self.ring.read()?;
//         let node_id = ring.get_node(msg.cluster_address).unwrap();
//         // let default_info = ;
//         let node = self.nodes
//             .get(node_id)
//             .map(|r| r.info)
//             .unwrap_or(NodeInfo::default());
//
//         Ok((*node_id, node.public_address.to_owned()))
//     }
// }

// GetCurrentLeader //////////////////////////////////////////////////////////
/// Get the current leader of the cluster from the perspective of the Raft metrics.
///
/// A return value of Ok(None) indicates that the current leader is unknown or the cluster hasn't
/// come to consensus on the leader yet.
#[derive(Debug, Clone)]
pub struct GetCurrentLeader {
    attempts: u8,
}

impl GetCurrentLeader {
    pub fn new() -> Self { GetCurrentLeader::default() }

    pub fn attempts_remaining(&self) -> u8 { self.attempts }

    pub fn retry(self) -> Result<GetCurrentLeader, ()> {
        if 0 < self.attempts {
            Ok(GetCurrentLeader {
                attempts: self.attempts - 1,
            })
        } else {
            Err(())
        }
    }
}

impl Default for GetCurrentLeader {
    fn default() -> Self {
        GetCurrentLeader {
            attempts: 3,
        }
    }
}

#[derive(Debug)]
pub struct CurrentLeader(Option<NodeId>);

impl std::fmt::Display for CurrentLeader {
    fn fmt(&self, f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let leader = match self.0 {
            Some(nid) => format!("#{}", nid),
            None => "No consensus".to_string(),
        };

        write!(f, "CurrentLeader({})", leader)
    }
}

impl Message for GetCurrentLeader {
    type Result = Result<CurrentLeader, NetworkError>;
}

impl Handler<GetCurrentLeader> for Network {
    type Result = ResponseActFuture<Self, CurrentLeader, NetworkError>;

    #[tracing::instrument(skip(self,_ctx))]
    fn handle(&mut self, msg: GetCurrentLeader, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(ref mut metrics) = self.metrics {
            if let Some(leader) = metrics.current_leader {
                Box::new(fut::result(Ok(CurrentLeader(Some(leader)))))
            } else {
                Box::new(
                    fut::wrap_future::<_, Self>( Delay::new(Instant::now() + Duration::from_secs(1)))
                        .map_err(|err, _, _| NetworkError::from(err))
                        .and_then(move |_, _, ctx| {
                            let msg_retry = msg.clone().retry().unwrap();
                            fut::wrap_future::<_, Self>(ctx.address().send(msg_retry))
                                .map_err(|err, _, _| NetworkError::from(err))
                                .and_then(|res, _, _| fut::result(res))
                        })
                )
            }
        } else {
            Box::new(
                fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(1)))
                    .map_err(|err, _, _| NetworkError::from(err))
                    .and_then(move |_, _, ctx| {
                        let msg_retry = msg.clone().retry().unwrap();
                        fut::wrap_future::<_, Self>(ctx.address().send(msg_retry))
                            .map_err(|err, _, _| NetworkError::from(err))
                            .and_then(|res, _, _| fut::result(res))
                    })
            )
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GetClusterSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSummary {
    pub id: NodeId,
    pub state: NetworkState,
    pub info: NodeInfo,
    pub connected_nodes: HashSet<NodeId>,
    pub isolated_nodes: HashSet<NodeId>,
    // pub metrics: Option<RaftMetrics>,
}

impl ClusterSummary {
    pub fn from_network(n: &Network) -> Self {
        Self {
            id: n.id,
            state: n.state,
            info: n.info.clone(),
            connected_nodes: n.nodes_connected.clone(),
            isolated_nodes: n.isolated_nodes.clone(),
            // metrics: n.metrics,
        }
    }
}

impl Message for GetClusterSummary {
    type Result = Result<ClusterSummary, NetworkError>;
}

impl Handler<GetClusterSummary> for Network {
    type Result = Result<ClusterSummary, NetworkError>;

    #[tracing::instrument]
    fn handle(&mut self, msg: GetClusterSummary, _ctx: &mut Self::Context) -> Self::Result {
        Ok(ClusterSummary::from_network(self))
    }
}

#[derive(Message, Debug)]
pub struct Join {
    pub id: NodeId,
    pub info: NodeInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JoinAcknowledged {
    pub joining_id: NodeId,
    pub accepted_by: NodeId,
    //todo WORK HERE
}

#[derive(Serialize, Deserialize, Message, Clone)]
pub struct ChangeClusterConfig {
    pub to_add: Vec<NodeId>,
    pub to_remove: Vec<NodeId>,
}

impl Handler<Join> for Network {
    type Result = ();

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: Join, _ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "handling Join request...");
        // let _change = ChangeClusterConfig { to_add: vec![msg.id], to_remove: vec![], };

        //todo: find leader node
        //todo: determine network state based on # nodes connected (0=>initialize, 1=>SingleNode, +=>Clustered)
        //todo: send Join to leader node
//todo: leader's local_node interprets Join into ChangeClusterConfig command;
        //todo: and_then register_node( msg.id, msg.info, ctx.address() )
    }
}

#[derive(Message, Debug)]
pub struct Handshake(pub NodeId, pub NodeInfo);

impl Handler<Handshake> for Network {
    type Result = ();

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: Handshake, _ctx: &mut Self::Context) -> Self::Result {
        self.restore_node(msg.0);
    }
}