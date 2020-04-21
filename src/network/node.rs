use std::fmt::Debug;
use std::time::Duration;
use actix::prelude::*;
use actix_raft::NodeId;
use thiserror::Error;
use tracing::*;
use strum_macros::{Display as StrumDisplay};
use crate::NodeInfo;
use proximity::*;

mod proximity;

// #[derive(Debug, PartialEq)]
// enum Status {
//     Registered,
//     Connected,
// }

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

#[derive(Debug, StrumDisplay, Clone, Copy, PartialEq)]
enum NodeStatus {
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
}

impl Node {
    #[tracing::instrument]
    pub fn new<S: AsRef<str> + Debug>(
        id: NodeId,
        local_id: NodeId,
        discovery_address: S,
        local_info: NodeInfo,
        // heartbeat_interval: Duration,
    ) -> Self {
        let proximity = Node::determine_proximity(id, local_id, discovery_address);

        info!("Registering NODE id:{} @ {} => {}", id, local_id, proximity);

        Node {
            id,
            local_id,
            proximity,
            status: NodeStatus::Initialized,
            local_info,
            // heartbeat_interval,
        }
    }

    #[tracing::instrument]
    fn determine_proximity<S: AsRef<str> + Debug>(
        node_id: NodeId,
        local_id: NodeId,
        discovery_address: S
    ) -> Box<dyn ProximityBehavior> {
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

    #[tracing::instrument(skip(self, ctx))]
    fn connect(&mut self, ctx: &mut <Node as Actor>::Context) {
        if self.status.is_disconnected() {
            info!("Connecting Node#{} {}...", self.id, self.proximity);
            let res = self.proximity.connect(self.local_id, &self.local_info, ctx);
            match res {
                Ok(_) => {
                    info!("Connection made local_id#{} to node#{}:{}", self.local_id, self.id, self.proximity);
                    self.status = NodeStatus::WeaklyConnected;
                }

                Err(err) => {
                    //todo consider limiting retries
                    self.status = match self.status {
                        NodeStatus::Failure(attempts) => NodeStatus::Failure(attempts + 1),
                        _status => NodeStatus::Failure(1),
                    };

                    warn!(
                        "{:?} in connection attempt local_id#{} => node#{}: {:?}",
                        self.status, self.local_id, self.id, err
                    );

                    ctx.run_later(
                        Duration::from_secs(3),
                        |_node, ctx| {
                            ctx.notify(Connect);
                        });
                }
            }
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    fn disconnect(&mut self, ctx: &mut <Node as Actor>::Context) {
        if self.status.is_connected() == true {
            info!("Disconnecting Node #{} from {}.", self.id, self.proximity);
            let res = self.proximity.disconnect(ctx);
            match res {
                Ok(_) => {
                    info!("Disconnection of local_id#{} from #{}:{}", self.local_id, self.id, self.proximity);
                    self.status = NodeStatus::Disconnected;
                }

                Err(err) => {
                    self.status = match self.status {
                        NodeStatus::Failure(attempts) => NodeStatus::Failure(attempts + 1),
                        _status => NodeStatus::Failure(1),
                    };

                    warn!("Error in disconnection of local_id#{} from node#{}: {:?}", self.local_id, self.id, err);
                }
            }
        }
    }
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
    type Result = Result<(), NodeError>;
}

impl Handler<Connect> for Node {
    type Result = ResponseActFuture<Self, (), NodeError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, _msg: Connect, ctx: &mut Self::Context) -> Self::Result {
        Box::new( fut::result(Ok(self.connect(ctx))))
        // self.connect(ctx);
        // ctx.run_later(self.heartbeat_interval, |node, ctx| {
        //     node.connect(ctx);
            // ctx.notify(Connect);
        // });
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