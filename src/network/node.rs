use std::fmt::Debug;
use std::time::Duration;
use std::pin::Pin;
use actix::prelude::*;
use actix_raft::{NodeId, AppData};
use thiserror::Error;
use tracing::*;
use strum_macros::{Display as StrumDisplay};
use serde::{Serialize, Deserialize};
use crate::NodeInfo;
use super::HandleNodeStatusChange;
use super::messages::{
    NetworkError,
    ConnectNode, Acknowledged,
    ChangeClusterConfig,
};

pub use super::proximity::{ProximityBehavior, LocalNode, RemoteNode};

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

    #[error("Remote node is not leader. Redirect to node#{leader_id:?}.")]
    RemoteNotLeaderError {
        leader_id: Option<NodeId>,
        // leader_address: Option<String>,
    },

    #[error("unknown node error: {0}")]
    Unknown(String),
}

#[derive(Clone)]
pub struct NodeRef<D: AppData> {
    pub id: NodeId,
    //todo log when cluster config beats join
    pub info: Option<NodeInfo>,
    // option for the case where cluster config change beats join; log when this occurs
    pub addr: Option<Addr<Node<D>>>,
}


impl<D: AppData> Debug for NodeRef<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NodeRef(id:{}, info:{:?}, started:{})",
            self.id, self.info, self.addr.is_some()
        )
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

pub struct Node<D: AppData> {
    pub id: NodeId, // id of the node (remote or local)
    pub local_id: NodeId, // id local to the machine of the node this instance exists
    pub proximity: Pin<Box<dyn ProximityBehavior<D>>>,
    pub status: NodeStatus,
    pub local_info: NodeInfo,
    // heartbeat_interval: Option<Duration>,
    pub network: Recipient<HandleNodeStatusChange>,
}

impl<D: AppData> Node<D> {
    #[tracing::instrument(skip(network))]
    pub fn new<R, E, S>(
        id: NodeId,
        info: NodeInfo,
        local_id: NodeId,
        local_info: NodeInfo,
        proximity: Pin<Box<dyn ProximityBehavior<D>>>,
        // heartbeat_interval: Duration,
        network: Recipient<HandleNodeStatusChange>,
    ) -> Self
    where
        R: actix_raft::AppDataResponse,
        E: actix_raft::AppError,
        S: actix_raft::RaftStorage<D, R, E>,
    {
        info!(local_id, node_id = id, ?proximity, "Creating Node");

        Self {
            id,
            local_id,
            proximity,
            status: NodeStatus::Initialized,
            local_info,
            // heartbeat_interval,
            network,
        }
    }

    #[tracing::instrument(skip(self))]
    fn apply_status_change(&mut self, status: NodeStatus ) {
        info!(local_id = self.local_id, node_id = self.id, "changing node status to {}", status);
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
                    local_id = self.local_id, node_id = self.id, attempts,
                    "Node failed too many connection attempts - marking as {}",
                    NodeStatus::Disconnected
                );

                NodeStatus::Disconnected
            },

            NodeStatus::Failure(_) => status,

            NodeStatus::WeaklyConnected |
            NodeStatus::Connected |
            NodeStatus::Disconnected => { status } ,
        };

        let handle_res= self.network.do_send( HandleNodeStatusChange { id: self.id, status: new_status });
        match handle_res {
            Ok(_) => {
                info!(
                    local_id = self.local_id, node_id = self.id,
                    "Network handled node status change to {:?}", new_status
                );
                ()
            },

            Err(err) => {
                error!(
                    local_id = self.local_id, node_id = self.id, error = ?err,
                    "Error in network handling of node status change to {:?}", new_status
                );
                ()
            }
        }
    }

    // fn handle_change_cluster_config<P>(proximity: &P, to_add: Vec<NodeId>, to_remove: Vec<NodeId>)
    //     where
    //         P: ChangeCluster
    // {
    //     proximity.change_cluster_config(to_add, to_remove, ctx);
    // }
}

impl<D: AppData> Debug for Node<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Node(id:{}, proximity:{}, status:{:?})",
            self.id, self.proximity, self.status
        )
    }
}

impl<D: AppData> Actor for Node<D> {
    type Context = Context<Self>;

    #[tracing::instrument(skip(self, ctx))]
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.notify(Connect);
    }

    #[tracing::instrument(skip(self, ctx))]
    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        self.disconnect(ctx);
        info!(local_id = self.local_id, node_id = self.id, "Node disconnected");
        Running::Stop
    }
}

#[derive(Debug)]
struct Connect;

impl Message for Connect {
    type Result = ();
    // type Result = Result<(), NodeError>;
}

impl<D: AppData> Handler<Connect> for Node<D> {
    type Result = ();
    // type Result = ResponseActFuture<Self, (), NodeError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, _msg: Connect, ctx: &mut Self::Context) -> Self::Result {
        let task = self.connect(ctx)
            .map_err(|err, node, _| {
                warn!(
                    local_id = node.local_id, node_id = node.id, error = ?err,
                    "error during connection to remote node.");
            })
            .and_then(|ack, node, ctx| {
                node.handle_connect_result(ack, ctx)
            });

        ctx.spawn(task);
    }
}

impl<D: AppData> Node<D> {
    #[tracing::instrument(skip(self, ack, _ctx))]
    fn handle_connect_result(
        &mut self,
        ack: Acknowledged,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> impl ActorFuture<Actor=Self, Item=(), Error=()> {
        info!(local_id = self.local_id, node_id = self.id, "connection made to node: {:?}", ack);
        self.apply_status_change(NodeStatus::Connected);
        fut::ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    fn connect(
        &mut self,
        ctx: &mut Context<Self>
    ) -> impl ActorFuture<Actor=Self, Item=Acknowledged, Error=NodeError> {
        if self.status.is_connected() {
            return fut::ok(Acknowledged {});
        } else {
            info!(
                local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
                "Connecting Node..."
            );
            let task = self.proximity.connect((self.local_id, &self.local_info), ctx)
                .map_err(|err| {
                    //todo consider limiting retries
                    let new_status = match self.status {
                        NodeStatus::Failure(attempts) => NodeStatus::Failure(attempts + 1),
                        _status => NodeStatus::Failure(1),
                    };

                    warn!(
                        local_id = self.local_id, node_id = self.id,
                        proximity = ?self.proximity, error = ?err,
                        "{:?} in connection attempt.", new_status
                    );

                    self.apply_status_change(new_status);

                    let delay = Duration::from_secs(3);
                    ctx.run_later(
                        delay.clone(),
                        move |node, ctx| {
                            debug!(
                                local_id = node.local_id, node_id = node.id, proximity = ?node.proximity,
                                "after {:?} delay trying again to connect...", delay
                            );
                            ctx.notify(Connect);
                        }
                    );

                    err
                })
                .and_then(|ack| {
                    info!(
                        local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
                        "Connection made."
                    );

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

impl<D: AppData> Handler<Disconnect> for Node<D> {
    type Result = ResponseActFuture<Self, (), NodeError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, _msg: Disconnect, ctx: &mut Self::Context) -> Self::Result {
        Box::new(fut::result(Ok(self.disconnect(ctx))))
    }
}

impl<D: AppData> Node<D> {
    #[tracing::instrument(skip(self, ctx))]
    fn disconnect(&mut self, ctx: &mut <Node<D> as Actor>::Context) {
        if self.status.is_connected() == true {
            info!(
                local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
                "Disconnecting node..."
            );
            let res = self.proximity.disconnect(ctx);
            match res {
                Ok(_) => {
                    info!(
                        local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
                        "Node disconnected."
                    );
                    self.apply_status_change(NodeStatus::Disconnected );
                }

                Err(err) => {
                    warn!(
                        local_id = self.local_id, node_id = self.id,
                        proximity = ?self.proximity, error = ?err,
                        "Error in node disconnection."
                    );
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

#[derive(Message, Debug)]
pub struct UpdateProximity<D: AppData> {
    pub node_id: NodeId,
    pub proximity: Pin<Box<dyn ProximityBehavior<D> + Send>>,
}

impl<D: AppData> UpdateProximity<D> {
    pub fn new(node_id: NodeId, new_proximity: Pin<Box<dyn ProximityBehavior<D> + Send>>) -> Self {
        Self {
            node_id,
            proximity: new_proximity,
        }
    }
}

// impl<D: AppData> std::clone::Clone for UpdateProximity<D> {
//     fn clone(&self) -> Self {
//
//         Self {
//             node_id: self.node_id,
//             proximity: self.proximity,
//         }
//     }
// }

impl<D: AppData> Handler<UpdateProximity<D>> for Node<D> {
    type Result = ();

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: UpdateProximity<D>, _ctx: &mut Self::Context) -> Self::Result {
        info!(
            local_id = self.local_id, node_id = self.id, proximity = ?self.proximity,
            "Updating proximity behavior."
        );

        self.proximity = msg.proximity;
    }
}

impl<D: AppData> Handler<ChangeClusterConfig> for Node<D> {
    type Result = Result<(), NetworkError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, msg: ChangeClusterConfig, ctx: &mut Self::Context) -> Self::Result {
        self.proximity
            .change_cluster_config(msg.add_members, msg.remove_members, ctx)
            .map_err(|err| NetworkError::from(err))
    }
}

impl<D: AppData> Node<D> {
    #[tracing::instrument]
    fn join_node(&self, node_id: NodeId, ctx: &mut <Node<D> as Actor>::Context) -> Result<(), NodeError>{
        self.proximity
            .change_cluster_config(vec![node_id], vec![], ctx)
    }
}

impl<D: AppData> Handler<ConnectNode> for Node<D> {
    type Result = ResponseActFuture<Self, Acknowledged, NetworkError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, msg: ConnectNode, ctx: &mut Self::Context) -> Self::Result {
        let change_msg = ChangeClusterConfig::new_to_add(vec![msg.id]);

        let task = fut::wrap_future::<_, Self>(ctx.address().send(change_msg))
            .map_err(|err, _, _| NetworkError::from(err))
            .and_then(move |res, node, _| {
                info!(
                    local_id = node.local_id, node_id = node.id, proximity = ?node.proximity,
                    "node#{} join submitted to RAFT cluster: {:?}", msg.id, &msg.info
                );
                fut::result(
                    res
                        .map_err(|err| NetworkError::from(err))
                        .map(|_| Acknowledged {} )
                )
            });

        Box::new(task)
    }
}
