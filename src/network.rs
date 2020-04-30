use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::{Instant, Duration};
use actix::prelude::*;
use actix_server::Server;
use actix_raft::{
    NodeId,
    metrics::RaftMetrics,
};
use tokio::timer::Delay;
use tracing::*;
use crate::{
    Configuration,
    NodeInfo,
    ring::RingType,
    ports::{self, PortData},
    utils,
};
use state::*;
use node::{Node, NodeRef, NodeStatus};
use summary::ClusterSummary;

pub use messages::NetworkError;
pub use messages::{
    HandleNodeStatusChange,
    ConnectNode, ConnectionAcknowledged
};

pub mod state;
pub mod node;
pub mod messages;
pub mod summary;


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
    // pub connected_nodes: HashSet<NodeId>,
    // pub isolated_nodes: HashSet<NodeId>,
    pub ring: RingType,
    pub metrics: Option<RaftMetrics>,
    pub server: Option<Server>,
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            // "Network(id:{:?}, state:{:?}, net_type:{:?}, info:{:?}, nodes:{:?}, isolated_nodes:{:?}, metrics:{:?})",
            "Network(id:{:?}, state:{:?}, discovery:{:?}, metrics:{:?})",
            self.id,
            self.state,
            // self.connected_nodes,
            // self.isolated_nodes,
            self.discovery,
            self.metrics,
        )
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Network(id:{} state:{})", self.id, self.state,)
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
            state: NetworkState::default(),
            discovery,
            nodes: BTreeMap::new(),
            // connected_nodes: HashSet::new(),
            // isolated_nodes: HashSet::new(),
            ring,
            metrics: None,
            server: None,
        }
    }

    pub fn summarize(&self) -> ClusterSummary {
        ClusterSummary {
            id: self.id,
            state: self.state.clone(),
            info: self.info.clone(),
            metrics: self.metrics.clone().map(|m| m.into()),
        }
    }

    #[tracing::instrument(skip(self, c))]
    pub fn configure_with(&mut self, c: &Configuration) {
        info!("configuration:{:?}", c);

        for n in c.nodes.values() {
            let id = utils::generate_node_id(n.cluster_address.as_str());
            let node_ref = NodeRef {
                id,
                info: Some(n.clone()),
                addr: None
            };
            info!(
                network_id = self.id,
                "adding configured {} {:?}",
                if self.id == id { "LOCAL" } else { "remote" },
                node_ref
            );

            self.nodes.insert(id, node_ref);
        }
    }

    fn is_leader(&self) -> bool {
        match self.metrics.as_ref().map(|m| m.current_leader) {
            Some(Some(leader_id)) => self.id == leader_id,
            _ => false
        }
    }

    fn leader_ref(&self) -> Option<&NodeRef> {
        self.metrics
            .as_ref()
            .map(|m| {
                m.current_leader
                    .map(|id| self.nodes.get(&id))
                    .flatten()
            })
            .flatten()
    }


    #[tracing::instrument(skip(self, self_addr))]
    fn register_node(
        &mut self,
        node_id: NodeId,
        node_info: &NodeInfo,
        self_addr: Addr<Self>
    ) -> Result<(), NetworkError> {
        if !self.nodes.contains_key(&node_id) {
            let node_ref = NodeRef {
                id: node_id,
                info: Some(node_info.clone()),
                addr: None,
            };
            self.nodes.insert(node_id, node_ref);
        }
        let node_ref = self.nodes.get(&node_id)
            .expect(format!("NodeRef assigned for {}", node_id).as_str());

        debug!(network_id = self.id, "Registering node {:?}...", &node_ref);

        match self.nodes.get_mut(&node_id) {
            Some(node_ref) => {
                node_ref.info = Some(node_info.clone());
                if node_ref.addr.is_none() {
                    info!(network_id = self.id, "Starting node#{}...", node_ref.id);

                    let node = Node::new(
                        node_ref.id,
                        node_info.clone(),
                        self.id,
                        self.info.clone(),
                        self_addr,
                    );

                    node_ref.addr = Some(node.start());
                }

                Ok(())
            }

            None => {
                let err_msg = format!("No node#{} registered in Network#{}.", node_id, self.id);
                Err(NetworkError::Unknown(err_msg))
            }
        }
            .and_then(|res| {
                self.restore_node(node_id);
                Ok(res)
            })
    }

    /// Isolate the network of the specified node.
    #[tracing::instrument(skip(self))]
    fn isolate_node(&mut self, id: NodeId) { self.state.isolate_node(id); }

    /// Restore the network of the specified node.
    #[tracing::instrument(skip(self))]
    fn restore_node(&mut self, id: NodeId) { self.state.restore_node(id); }
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
            if let Some(info) = &node_ref.info {
                self.register_node(*node_id, info, ctx.address()).unwrap();
            }
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

impl Message for GetClusterSummary {
    type Result = Result<ClusterSummary, NetworkError>;
}

impl Handler<GetClusterSummary> for Network {
    type Result = Result<ClusterSummary, NetworkError>;

    #[tracing::instrument]
    fn handle(&mut self, msg: GetClusterSummary, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.summarize())
    }
}

impl Network {
    fn leader_delegate(&self) -> impl ActorFuture<Actor = Self, Item = Addr<Node>, Error = NetworkError> {
        fut::result(
            match self.leader_ref() {
                Some(NodeRef{ id, info: _, addr}) if id == &self.id && addr.is_some() => Ok(addr.clone().unwrap()),

                Some(lref) => {
                    Err(
                        if self.id == lref.id {
                            NetworkError::Unknown("Local leader node is not started".to_string())
                        } else if lref.info.is_some() {
                            NetworkError::NotLeader {
                                leader_id: Some(lref.id),
                                leader_address: lref.info.clone().map(|info| info.cluster_address.clone()),
                            }
                        } else {
                            NetworkError::Unknown(format!("Leader#{} actor is not registered.", lref.id))
                        }
                    )
                },

                None => Err(NetworkError::NoElectedLeader),
            }
        )
    }
}

impl Handler<HandleNodeStatusChange> for Network {
    type Result = ();

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: HandleNodeStatusChange, _ctx: &mut Self::Context) -> Self::Result {
        info!("Node#{} status changed to: {}", msg.id, msg.status);
        if self.nodes.contains_key(&msg.id) {
            match msg.status {
                NodeStatus::Initialized => { self.isolate_node(msg.id); },
                NodeStatus::WeaklyConnected => { self.isolate_node(msg.id); },
                NodeStatus::Connected => { self.restore_node(msg.id); },
                NodeStatus::Failure(attempts) => {
                    info!("Node#{} having trouble connecting with {} attempts", msg.id, attempts);
                },
                NodeStatus::Disconnected => {
                    info!("This network cannot reach Node#{} - isolating.", msg.id);
                    self.isolate_node(msg.id);
                },
            };


        }
    }
}

impl Handler<ConnectNode> for Network {
    type Result = ResponseActFuture<Self, ConnectionAcknowledged, NetworkError>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: ConnectNode, _ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "handling RegisterNode#{} request...", msg.id);

        let target_node = msg.clone();

        Box::new(
        self.leader_delegate()
            .and_then(move |delegate, _, _| {
                fut::wrap_future(delegate.send(msg.clone()))
                    .map_err(|err, _, _| NetworkError::from(err))
            })
            .and_then(move |ack, net, ctx| {
                info!("Node#{:?} connection acknowledged - registering with local network...", target_node.id );
                fut::result(
                    net.register_node(target_node.id, &target_node.info, ctx.address())
                        .and_then(|_| ack)
                )
            })
        )
    }
}
//         //todo: find leader node
//         //todo: determine network state based on # nodes connected (0=>initialize, 1=>SingleNode, +=>Clustered)
//         //todo: send Join to leader node
// //todo: leader's local_node interprets Join into ChangeClusterConfig command;
//         //todo: and_then register_node( msg.id, msg.info, ctx.address() )
//     }
// }
