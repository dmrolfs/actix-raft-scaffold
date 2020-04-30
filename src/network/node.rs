use std::fmt::Debug;
use std::time::Duration;
use actix::prelude::*;
use actix_raft::NodeId;
use thiserror::Error;
use tracing::*;
use strum_macros::{Display as StrumDisplay};
use serde::{Serialize, Deserialize};
use crate::NodeInfo;
use super::Network;
use super::messages::{ConnectNode, ConnectionAcknowledged, NetworkError};
use proximity::*;
use crate::network::HandleNodeStatusChange;

mod proximity;

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("failed to change cluster {to_add:?} and {to_remove:?}")]
    ChangeClusterError {
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
    },

    #[error("remote node request failed:{0}")]
    RemoteNodeSendError(String),
    // RemoteNodeSendError(actix_web::client::SendRequestError ),

    #[error("error in http request to remote node:{0}")]
    RequestError(#[from] reqwest::Error),

    #[error("failed to parse response from remote node:{0}")]
    ResponseParseError(#[from] serde_json::Error),

    #[error("remote node response failure: {0}")]
    ResponseFailure(String),

    #[error("Node request timed out: {0}")]
    Timeout(String),

    #[error("payload error {0:?}", )]
    PayloadError(String),
    // PayloadError(actix_web::error::PayloadError),

    #[error("Remote node is not leader. Redirect to node#{leader_id:?} at {leader_address:?}.")]
    RemoteNotLeaderError {
        leader_id: Option<NodeId>,
        leader_address: Option<String>,
    },

    #[error("unknown node error: {0}")]
    Unknown(String),
}

#[derive(Clone)]
pub struct NodeRef {
    pub id: NodeId,
    //todo log when cluster config beats join
    pub info: Option<NodeInfo>, // option for the case where cluster config change beats join; log when this occurs
    pub addr: Option<Addr<Node>>,
}

impl Debug for NodeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeRef(id:{}, info:{:?}, started:{})", self.id, self.info, self.addr.is_some())
    }
}

#[derive(Debug, StrumDisplay, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NodeStatus {
    Initialized,
    WeaklyConnected,
    Connected,
    Failure(i8),
    Disconnected,
}

impl NodeStatus {
    pub fn is_connected(&self) -> bool {
        match self {
            NodeStatus::Connected => true,
            NodeStatus::WeaklyConnected => true,
            _ => false,
        }
    }

    pub fn is_disconnected(&self) -> bool { !self.is_connected() }
}

pub struct Node {
    id: NodeId, // id of the node (remote or local)
    local_id: NodeId, // id local to the machine of the node this instance exists
    proximity: Box<dyn ProximityBehavior>,
    status: NodeStatus,
    local_info: NodeInfo,
    // heartbeat_interval: Option<Duration>,
    network: Addr<Network>,
}

impl Node {
    #[tracing::instrument]
    pub fn new(
        id: NodeId,
        info: NodeInfo,
        local_id: NodeId,
        local_info: NodeInfo,
        // heartbeat_interval: Duration,
        network: Addr<Network>,
    ) -> Self {
        let proximity = Node::determine_proximity(local_id, id, &info);

        info!("[Node#{}] Creating Node#{} as {}", local_id, id, proximity);

        Node {
            id,
            local_id,
            proximity,
            status: NodeStatus::Initialized,
            local_info,
            // heartbeat_interval,
            network,
        }
    }

    #[tracing::instrument]
    fn determine_proximity(
        local_id: NodeId,
        node_id: NodeId,
        node_info: &NodeInfo
    ) -> Box<dyn ProximityBehavior> {
        if node_id == local_id {
            Box::new(LocalNode::new(local_id))
        } else {
            Box::new(RemoteNode::new(node_id, node_info.clone()))
        }
    }

    #[tracing::instrument(skip(self))]
    fn apply_status_change(&mut self, status: NodeStatus ) {
        info!("changing node#{} to {}", self.id, status);
        let new_status = match status {
            NodeStatus::Initialized => {
                panic!(
                    format!(
                        "Node#{} shouldn't go back to {} from {}.",
                        self.id, status, self.status
                    )
                )
            },

            NodeStatus::Failure(attempts) if 3 < attempts=> {
                warn!(
                    "Node#{} failed too many ({}) connections attempts - marking as {}",
                    self.id, attempts, NodeStatus::Disconnected
                );

                NodeStatus::Disconnected
            },

            NodeStatus::Failure(_) => status,

            NodeStatus::WeaklyConnected |
            NodeStatus::Connected |
            NodeStatus::Disconnected => { status } ,
        };

        self.network.do_send( HandleNodeStatusChange { id: self.id, status: new_status, });
    }

    // fn handle_change_cluster_config<P>(proximity: &P, to_add: Vec<NodeId>, to_remove: Vec<NodeId>)
    //     where
    //         P: ChangeCluster
    // {
    //     proximity.change_cluster_config(to_add, to_remove, ctx);
    // }
}

impl Debug for Node {
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

    #[tracing::instrument(skip(self, ctx))]
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.notify(Connect);
    }

    #[tracing::instrument(skip(self, ctx))]
    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        self.disconnect(ctx);
        info!("Node #{} disconnected", self.id);
        Running::Stop
    }
}

#[derive(Debug)]
struct Connect;

impl Message for Connect {
    type Result = ();
    // type Result = Result<(), NodeError>;
}

impl Handler<Connect> for Node {
    type Result = ();
    // type Result = ResponseActFuture<Self, (), NodeError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, _msg: Connect, ctx: &mut Self::Context) -> Self::Result {
        let task = self.connect(ctx)
            .map_err(|err, node, _| {
                warn!("error during connection to remote node#{}:{}", node.id, err);
            })
            .and_then(|ack, node, ctx| {
                node.handle_connect_result(ack, ctx)
            });

        ctx.spawn(task);
    }
}

impl Node {
    #[tracing::instrument(skip(self, ack, _ctx))]
    fn handle_connect_result(
        &mut self,
        ack: ConnectionAcknowledged,
        _ctx: &mut <Node as Actor>::Context
    ) -> impl ActorFuture<Actor=Self, Item=(), Error=()> {
        info!("connection made to node#{}: {:?}", self.id, ack);
        self.apply_status_change(NodeStatus::Connected);
        fut::ok(())
    }
}

impl Node {
    #[tracing::instrument(skip(self, ctx))]
    fn connect(
        &mut self,
        ctx: &mut Context<Self>
    ) -> impl ActorFuture<Actor=Self, Item=ConnectionAcknowledged, Error=NodeError> {
        if self.status.is_connected() {
            return fut::ok(ConnectionAcknowledged {});
        } else {
            info!("Connecting Node#{}({:?}) to {}...", self.local_id, self.status, self.proximity);
            let task = self.proximity.connect((self.local_id, &self.local_info), ctx)
                .map_err(|err| {
                    //todo consider limiting retries
                    let new_status = match self.status {
                        NodeStatus::Failure(attempts) => NodeStatus::Failure(attempts + 1),
                        _status => NodeStatus::Failure(1),
                    };

                    warn!(
                        "{:?} in connection attempt local_id#{} => node#{}: {:?}",
                        new_status, self.local_id, self.id, err
                    );

                    self.apply_status_change(new_status);

                    ctx.run_later(
                        Duration::from_secs(3),
                        |node, ctx| {
                            debug!("after delay trying again to connect to {}...", node.proximity);
                            ctx.notify(Connect);
                        }
                    );

                    err
                })
                .and_then(|ack| {
                    info!("Connection made local#{} to {}", self.local_id, self.proximity);

                    self.apply_status_change(NodeStatus::WeaklyConnected);
                    Ok(ack)
                });

            return fut::result(task);
        }
    }
}




#[derive(Debug)]
pub struct Disconnect;

impl Message for Disconnect {
    type Result = Result<(), NodeError>;
}

impl Handler<Disconnect> for Node {
    type Result = ResponseActFuture<Self, (), NodeError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, _msg: Disconnect, ctx: &mut Self::Context) -> Self::Result {
        Box::new(fut::result(Ok(self.disconnect(ctx))))
    }
}

impl Node {
    #[tracing::instrument(skip(self, ctx))]
    fn disconnect(&mut self, ctx: &mut <Node as Actor>::Context) {
        if self.status.is_connected() == true {
            info!("Disconnecting Node #{} from {}.", self.id, self.proximity);
            let res = self.proximity.disconnect(ctx);
            match res {
                Ok(_) => {
                    info!("Disconnection of local_id#{} from #{}:{}", self.local_id, self.id, self.proximity);
                    self.apply_status_change(NodeStatus::Disconnected );
                }

                Err(err) => {
                    warn!("Error in disconnection of local_id#{} from node#{}: {:?}", self.local_id, self.id, err);
                    let failure = match self.status {
                        NodeStatus::Failure(attempts) => NodeStatus::Failure(attempts + 1),
                        _status => NodeStatus::Failure(1),
                    };

                    self.apply_status_change(failure);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangeClusterConfig {
    add_members: Vec<NodeId>,
    remove_members: Vec<NodeId>,
}

impl ChangeClusterConfig {
    pub fn new_to_add_remove(to_add: Vec<NodeId>, to_remove: Vec<NodeId>) -> Self {
        Self { add_members: to_add, remove_members: to_remove, }
    }

    pub fn new_to_add(to_add: Vec<NodeId>) -> Self {
        ChangeClusterConfig::new_to_add_remove(to_add, vec![])
    }

    pub fn new_to_remove(to_remove: Vec<NodeId>) -> Self {
        ChangeClusterConfig::new_to_add_remove(vec![], to_remove)
    }
}

impl Message for ChangeClusterConfig {
    type Result = Result<(), NodeError>;
}

impl Handler<ChangeClusterConfig> for Node {
    type Result = Result<(), NodeError>;

    #[tracing::instrument]
    fn handle(&mut self, msg: ChangeClusterConfig, ctx: &mut Self::Context) -> Self::Result {
        self.proximity.change_cluster_config(msg.add_members, msg.remove_members, ctx)
    }
}

impl Node {
    #[tracing::instrument]
    fn join_node(&self, node_id: NodeId, ctx: &<Node as Actor>::Context) -> Result<(), NodeError>{
        self.proximity.change_cluster_config(vec![node_id], vec![], ctx)
    }
}

impl Handler<ConnectNode> for Node {
    type Result = ResponseActFuture<Self, ConnectionAcknowledged, NetworkError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, msg: ConnectNode, ctx: &mut Self::Context) -> Self::Result {
        let change_msg = ChangeClusterConfig::new_to_add(vec![msg.id]);

        let task = fut::wrap_future::<_, Self>(ctx.address().send(change_msg))
            .map_err(|err, _, _| NetworkError::from(err))
            .and_then(move |res, node, _| {
                info!(node_id = node.id, "node#{} join submitted to RAFT cluster: {:?}", msg.id, &msg.info);
                fut::result(
                    res
                        .map_err(|err| NetworkError::from(err))
                        .map(|_| ConnectionAcknowledged {} )
                )
            });

        Box::new(task)
    }
}
