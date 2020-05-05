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
use crate::network::messages::ChangeClusterConfig;

pub mod state;
pub mod node;
pub mod messages;
pub mod summary;


// impl Network {
//     fn self_send_after_delay<M, I>(
//         &mut self,
//         msg: M,
//         delay: Duration,
//         ctx: &mut Context<Self>
//     ) -> impl ActorFuture<Actor = Self, Item = M::Result::Item, Error = E>
//     where
//         M: Message,
//         M::Result = I,
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
        info!(network_id = self.id, "configuration:{:?}", c);

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
        // let node_ref = self.nodes.get(&node_id)
        //     .expect(format!("NodeRef assigned for {}", node_id).as_str());
        //
        // debug!(network_id = self.id, "Registering node {:?}...", &node_ref);

        match self.nodes.get_mut(&node_id) {
            Some(NodeRef{id, info: Some(info), addr: Some(addr)}) if info == node_info => {
                info!(network_id = self.id, node_id = ?id, "node is current and connected - no further action.");
                Ok(())
            },

            Some(node_ref) => {
                info!(network_id = self.id, node_id = node_ref.id, "updating node info and connecting...");
                node_ref.info = Some(node_info.clone());
                let node = Node::new(
                    node_id,
                    node_info.clone(),
                    self.id,
                    self.info.clone(),
                    self_addr,
                );
                node_ref.addr = Some(node.start());
                Ok(())
            },

            // Some(node_ref) => {
            //     node_ref.info = Some(node_info.clone());
            //     if node_ref.addr.is_none() {
            //         info!(network_id = self.id, node_id = node_ref.id, "Starting node...");
            //
            //         let node = Node::new(
            //             node_ref.id,
            //             node_info.clone(),
            //             self.id,
            //             self.info.clone(),
            //             self_addr,
            //         );
            //
            //         node_ref.addr = Some(node.start());
            //     }
            //
            //     Ok(())
            // }
            //
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
        info!(
            network_id = self.id,
            endpoint_address = self.info.cluster_address.as_str(),
            "Network binding to http endpoint..."
        );
        let server = ports::http::start_server(self.info.cluster_address.as_str(), bind.data)?;
        info!(
            network_id = self.id,
            endpoint_address = self.info.cluster_address.as_str(),
            "Network http endpoint started."
        );
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
    /// Returns the Addr<Node> if this network is the leader; otherwise if there is consensus, a
    /// NotLeader error referencing the leader or notice there is no elected leader.
    #[tracing::instrument(skip(self))]
    fn leader_delegate(&self) -> impl ActorFuture<Actor = Self, Item = Addr<Node>, Error = NetworkError> {
        let leader = self.leader_ref();
        fut::result(
            match self.leader_ref() {
                Some(NodeRef{ id, info: _, addr}) if id == &self.id && addr.is_some() => {
                    debug!(network_id = self.id, "Network's node is leader delegate.");
                    Ok(addr.clone().unwrap())
                },

                Some(lref) => {
                    let err = Err(
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
                    );

                    debug!(
                        network_id = self.id, error = ?err,
                        "Leader identified other than this network."
                    );

                    err
                },

                None => {
                    let err = Err(NetworkError::NoElectedLeader);
                    debug!(network_id = self.id, "No elected leader.");
                    err
                },
            }
        )
    }
}

impl Handler<HandleNodeStatusChange> for Network {
    type Result = ();

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: HandleNodeStatusChange, _ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, node_id = msg.id, status = ?msg.status, "Node status changed");
        if self.nodes.contains_key(&msg.id) {
            match msg.status {
                NodeStatus::Initialized => { self.isolate_node(msg.id); },
                NodeStatus::WeaklyConnected => { self.isolate_node(msg.id); },
                NodeStatus::Connected => { self.restore_node(msg.id); },
                NodeStatus::Failure(attempts) => {
                    info!(
                        network_id = self.id,
                        node_id = msg.id,
                        attempts,
                        "Node having trouble connecting."
                    );
                },
                NodeStatus::Disconnected => {
                    info!(
                        network_id = self.id,
                        node_id = msg.id,
                        "Network cannot reach Node - isolating."
                    );
                    self.isolate_node(msg.id);
                },
            };


        }
    }
}

impl Network {
    fn node_for_id(&self, id: NodeId) -> Option<Addr<Node>> {
        self.nodes.get(&id).map(|node_ref| node_ref.addr.clone()).flatten()
    }

    #[tracing::instrument(skip(self, command, ctx))]
    fn handle_connect_remote_node(
        &mut self,
        command: ConnectNode,
        ctx: &mut Context<Self>
    ) -> impl ActorFuture<Actor = Self, Item = (), Error = NetworkError> {
        let task = match self.nodes.get(&command.id) {
            Some(NodeRef { id: _, info: Some(current_info), addr: _} )
            if &command.info == current_info => {
                debug!(
                    network_id = self.id, ?command,
                    "no change to existing node connection - finalizing without change."
                );
                Ok(())
            },

            Some(NodeRef { id: _, info: Some(current_info), addr: _} ) => {
                info!(
                    network_id = self.id, ?command,
                    "node info changed - re-registering node."
                );
                self.register_node(command.id, &command.info, ctx.address())
            },

            None | Some(NodeRef { id: _, info: None, addr: _ }) => {
                info!(
                    network_id = self.id, ?command,
                    "no info previous set - registering node."
                );
                self.register_node(command.id, &command.info, ctx.address())
            },

            // None | Some(NodeRef { id: _, info: None, addr: None }) => {
            //     info!(
            //         network_id = self.id, ?command,
            //         "no node info or  previous set - registering node."
            //     );
            //     self.register_node(command.id, &command.info, ctx.address())
            // },
        };

        fut::result(task)
    }

    #[tracing::instrument(skip(self, _ctx))]
    fn delegate_to_leader<M, I, E>(
        &self,
        command: M,
        _ctx: &mut Context<Self>,
    ) -> impl ActorFuture<Actor = Self, Item = I, Error = E>
    where
        M: Message + std::fmt::Debug + Send + 'static,
        M::Result: Into<Result<I, E>>,
        M::Result: Send,
        Node: Handler<M>,
        E: From<actix::MailboxError>,
        E: From<NetworkError>,
    {
        debug!(network_id = self.id, ?command, "lookup and delegate command to leader...");

        Box::new(
            self.leader_delegate()
                .map_err(|err, network, _| {
                    error!(
                        network_id = network.id, error = ?err,
                        "error in determining leader"
                    );
                    err.into()
                })
                .and_then( move |leader, network, _| {
                    info!(network_id = network.id, ?command, "Sending command to leader...");
                    let command_desc = format!("{:?}", command);

                    fut::wrap_future::<_, Self>(leader.send(command))
                        .map_err(|err, network, _| {
                            error!(
                                network_id = network.id, error = ?err,
                                "command delegation to leader failed."
                            );
                            err.into()
                        })
                        .and_then(move |res, network, _| {
                            debug!(
                                network_id = network.id, command = ?command_desc,
                                "command delegation to leader succeeded."
                            );
                            fut::result(res.into())
                        })
                })
        )
    }
}

impl Handler<ConnectNode> for Network {
    type Result = ResponseActFuture<Self, ConnectionAcknowledged, NetworkError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, command: ConnectNode, ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "handling ConnectNode#{} command...", command.id);

        Box::new(
            self.handle_connect_remote_node(command.clone(), ctx)
                .and_then(move |_, network, ctx| {
                    let change_cluster_command = ChangeClusterConfig::new_to_add(vec!(command.id));

                    network.delegate_to_leader(change_cluster_command, ctx)
                        .then(move |res, network, _| {
                            fut::result(
                                match res {
                                    Ok(_) => {
                                        info!(
                                            network_id = network.id, node_id = command.id,
                                            "cluster config change handled by leader: {:?}", res
                                        );
                                        Ok(ConnectionAcknowledged {})
                                    },
                                    Err(NetworkError::NoElectedLeader) => {
                                        warn!(
                                            network_id = network.id, node_id = command.id,
                                            "no elected leader -- maybe ask to retry???"
                                        );
                                        Ok(ConnectionAcknowledged {})
                                    },
                                    Err(NetworkError::NotLeader{leader_id: _, leader_address: _}) => {
                                        warn!(
                                            network_id = network.id, node_id = command.id,
                                            "Network is *not* elected leader -- retry???"
                                        );
                                        Ok(ConnectionAcknowledged {})
                                    },
                                    Err(err) => {
                                        error!(
                                            network_id = network.id, node_id = command.id,
                                            error = ?err,
                                            "failed to change config in leader -- retry???"
                                        );
                                        Err(err)
                                    },
                                }
                            )
                        })
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


// impl Handler<ConnectNode> for Network {
//     type Result = ResponseActFuture<Self, ConnectionAcknowledged, NetworkError>;
//
//     #[tracing::instrument(skip(self, _ctx))]
//     fn handle(&mut self, msg: ConnectNode, _ctx: &mut Self::Context) -> Self::Result {
//         info!(network_id = self.id, "handling RegisterNode#{} request...", msg.id);
//
//         let target_node = msg.clone();
//
//         Box::new(
//         self.leader_delegate()
//             .and_then(move |delegate, _, _| {
//                 fut::wrap_future(delegate.send(msg.clone()))
//                     .map_err(|err, _, _| NetworkError::from(err))
//             })
//             .and_then(move |ack, net, ctx| {
//                 info!("Node#{:?} connection acknowledged - registering with local network...", target_node.id );
//                 fut::result(
//                     net.register_node(target_node.id, &target_node.info, ctx.address())
//                         .and_then(|_| ack)
//                 )
//             })
//         )
//     }
// }
