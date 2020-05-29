use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::{Instant, Duration};
use actix::prelude::*;
use actix_server::Server;
use actix_raft::{
    NodeId,
    Raft, AppData, AppDataResponse, AppError, RaftStorage,
    metrics::{RaftMetrics, State as RaftState},
};
use tokio::timer::Delay;
use tracing::*;
use crate::{
    Configuration,
    NodeInfo,
    ring::RingType,
    ports::{self, PortData},
    // utils,
};
use state::*;
use node::{
    Node, NodeRef, NodeStatus, UpdateProximity,
    ProximityBehavior, LocalNode, RemoteNode,
};
use summary::ClusterSummary;

pub use messages::NetworkError;
pub use messages::{
    HandleNodeStatusChange,
    ConnectNode, Acknowledged,
    ClientRequest,
};
use crate::network::messages::{ChangeClusterConfig, DisconnectNode};
use std::option::NoneError;

pub mod state;
pub mod node;
mod proximity;
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

pub struct Network<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    pub id: NodeId,
    pub info: NodeInfo,
    pub state: NetworkState,
    pub discovery: SocketAddr,
    pub nodes: BTreeMap<NodeId, NodeRef<D>>,
    pub ring: RingType,
    pub metrics: Option<RaftMetrics>,
    pub raft: Option<Addr<Raft<D, R, E, Network<D, R, E, S>, S>>>,
    pub server: Option<Server>,
    marker_data: PhantomData<D>,
}

impl<D, R, E, S> std::fmt::Debug for Network<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
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

impl<D, R, E, S> std::fmt::Display for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Network(id:{} state:{})", self.id, self.state,)
    }
}

impl<D, R, E, S> Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    pub fn new(
        id: NodeId,
        info: &NodeInfo,
        ring: RingType,
        discovery: SocketAddr,
    ) -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            id,
            NodeRef {
                id,
                info: Some(info.clone()),
                addr: None,
            }
        );

        Network {
            id,
            info: info.clone(),
            state: Default::default(),
            discovery,
            nodes,
            ring,
            metrics: None,
            raft: None,
            server: None,
            marker_data: std::marker::PhantomData,
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

        for n in c.seed_nodes.values() {
            let id = n.node_id();
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

    pub fn get_node(&self, id: NodeId) -> Option<Addr<Node<D>>> {
        self.nodes.get(&id).map(|node_ref| node_ref.addr.clone()).flatten()
    }

    pub fn is_leader(&self) -> bool {
        match self.metrics.as_ref().map(|m| m.current_leader) {
            Some(Some(leader_id)) => self.id == leader_id,
            _ => false
        }
    }

    pub fn leader_ref(&self) -> Option<&NodeRef<D>> {
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
        node_info: NodeInfo,
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

        match self.nodes.get_mut(&node_id) {
            Some(NodeRef{id, info: Some(info), addr: Some(_)}) if info == &node_info => {
                info!(network_id = self.id, node_id = ?id, "node is current and connected - no further action.");
                Ok(())
            },

            Some(node_ref) => {
                //todo: refactor to pull into method
                info!(network_id = self.id, node_id = node_ref.id, "updating node info and connecting...");

                node_ref.info = Some(node_info.clone());

                let proximity_res = Self::determine_proximity(self.id, node_id, &node_info, self.raft.clone());
                debug!(network_id = self.id, node_id, "proximity determined: {:?}", proximity_res);
                match proximity_res {
                    Ok(proximity) => {
                        info!(network_id = self.id, node_id, ?proximity, "starting Node actor");
                        let node = Node::new::<R, E, S>(
                            node_id,
                            node_info.clone(),
                            self.id,
                            self.info.clone(),
                            proximity,
                            self_addr.recipient(),
                        );
                        node_ref.addr = Some(node.start());
                    },

                    Err(_) => {
                        info!(
                            network_id = self.id, node_id = node_id,
                            "Cannot set proximity before registered raft."
                        );
                    },
                };

                Ok(())
            },

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

    // #[tracing::instrument(skip(raft))]
    fn determine_proximity(
        local_id: NodeId,
        node_id: NodeId,
        node_info: &NodeInfo,
        raft: Option<Addr<Raft<D, R, E, Network<D, R, E, S>, S>>>
    ) -> Result<Pin<Box<dyn ProximityBehavior<D> + Send>>, ()> {
        if node_id == local_id {
            match raft {
                Some(r) => Ok(Box::pin(LocalNode::new(local_id, r))),
                None => Err(()),
            }
        } else {
            Ok(Box::pin(RemoteNode::new(node_id, node_info.clone())))
        }
    }
}

impl<D, R, E, S> Actor for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Context = Context<Self>;

    #[tracing::instrument(skip(self, ctx))]
    fn started(&mut self, ctx: &mut Self::Context) {
        info!(network_id = self.id, "registering nodes configured with network...");
        for (node_id, node_ref) in self.nodes.clone().iter() {
            if let Some(info) = &node_ref.info {
                self.register_node(*node_id, info.clone(), ctx.address()).unwrap();
            }
        }
    }
}

#[derive(Clone)]
pub struct RegisterRaft<D, R, E, S>(pub Addr<Raft<D, R, E, Network<D, R, E, S>, S>>)
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>;

impl<D, R, E, S> Message for RegisterRaft<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = Result<(), NetworkError>;
}

impl<D, R, E, S> std::fmt::Debug for RegisterRaft<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegisterRaft")
    }
}

impl<D, R, E, S> Handler<RegisterRaft<D, R, E, S>> for Network<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    type Result = ResponseActFuture<Self, (), NetworkError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, msg: RegisterRaft<D, R, E, S>, ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "Registering Raft actor with Network - updating LocalNode.");
        self.raft = Some(msg.0);
        let network_id = self.id;

        let node_ref = self.nodes.get(&network_id).map(|r| r.clone());
        match node_ref {
            Some(node_ref @ NodeRef { id: _, info: Some(_), addr: Some(_) }) => {
                Box::new(self.register_raft(node_ref, self.raft.as_ref().unwrap().clone()))
            }

            // Some(NodeRef{ id, info: Some(info), addr: None }) => {
            Some(node_ref @ NodeRef{ id: _, info: Some(_), addr: None }) => {
                Box::new(self.register_raft_and_create_local_node(node_ref, ctx))
            },

            Some(NodeRef{ id: _, info: None, addr: _ }) => {
                info!(network_id, "Registering raft but NodeInfo info needed to complete LocalNode.");
                Box::new(fut::ok(()))
            },

            None => {
                error!(network_id, "Registering raft, but Network wasn't started.");
                panic!("Shouldn't happen: RegisterRaft command sent to unstarted Network actor.");
                // Box::new(fut::err(
                //     NetworkError::Unknown("Shouldn't happen: RegisterRaft command sent to unstarted Network actor.".to_string())
                // ))
            }
        }
    }
}

impl<D, R, E, S> Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(self, raft))]
    fn register_raft(
        &self,
        node_ref: NodeRef<D>,
        raft: Addr<Raft<D, R, E, Network<D, R, E, S>, S>>,
    ) -> impl ActorFuture<Actor = Self, Item = (), Error = NetworkError> {
        let network_id = self.id;
        info!(network_id, "Registering raft with new proximity in existing LocalNode.");
        assert!(node_ref.info.is_some());
        assert!(node_ref.addr.is_some());

        let local_id = self.id;
        let node_id = node_ref.id;

        fut::result::<_, _, Self>(Self::determine_proximity(
            local_id,
            node_id,
            node_ref.info.as_ref().unwrap(),
            Some(raft)
        ))
            .map_err(move |_, _, _| {
                error!(
                    network_id, node_id,
                    "Failed to determine node proximity since Raft registration failed."
                );
                NetworkError::Unknown(
                    format!(
                        "[network_id={}] [node_id={}] Failed to determine node proximity since Raft registration failed.",
                        network_id, node_id
                    )
                )
            })
            .and_then(move |p, _, _| {
                let cmd = UpdateProximity::new(node_id, p);
                fut::wrap_future::<_, Self>(node_ref.addr.as_ref().unwrap().send(cmd))
                    .map_err(|err, _, _| NetworkError::from(err))
            })
    }

    #[tracing::instrument(skip(self))]
    fn register_raft_and_create_local_node(
        &mut self,
        node_ref: NodeRef<D>,
        ctx: &mut <Network<D, R, E, S> as Actor>::Context,
    ) -> impl ActorFuture<Actor = Self, Item = (), Error = NetworkError> {
        let network_id = self.id;
        let node_id = node_ref.id;
        info!(network_id, "Registering raft and creating LocalNode.");
        assert!(node_ref.info.is_some());
        assert!(node_ref.addr.is_none());

        // raft setup in local node on create
        let register_res = self.register_node(
            node_id,
            node_ref.info.as_ref().unwrap().clone(),
            ctx.address()
        );

        match register_res {
            Ok(_) => {
                info!(network_id, node_id, "node fully registered.");
                fut::ok(())
            },

            Err(err) => {
                error!(network_id, node_id, error = ?err, "Error during node registration");
                fut::err(err)
            }
        }
    }
}


impl<D, R, E, S> Handler<RaftMetrics> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = ();

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: RaftMetrics, _ctx: &mut Self::Context) -> Self::Result {
        debug!(
            network_id = self.id,
            "RAFT node={} state={:?} leader={:?} term={} index={} applied={} cfg={{consensus={} members={:?} non_voters={:?} removing={:?}}}",
            msg.id, msg.state, msg.current_leader, msg.current_term, msg.last_log_index,
            msg.last_applied, msg.membership_config.is_in_joint_consensus,
            msg.membership_config.members, msg.membership_config.non_voters,
            msg.membership_config.removing,
        );

        self.advance_state_on_raft_status(&msg);
        self.metrics = Some(msg);
    }
}

impl<D, R, E, S>  Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip())]
    fn advance_state_on_raft_status(&mut self, metrics: &RaftMetrics) {
        let status = &self.state.status;
        let raft_state = &metrics.state;

        match (status, raft_state) {
            (Status::Joining, RaftState::NonVoter) => {},
            (Status::Joining, _) => {
                self.do_advance_state(Status::Up, "Raft is actively functioning and full member");
            },

            (Status::WeaklyUp, RaftState::NonVoter) => {},
            (Status::WeaklyUp, _) => {
                self.do_advance_state(Status::Up, "Raft is actively functioning and full member");
            },

            (Status::Down, RaftState::NonVoter) => {
                self.do_advance_state(Status::Joining, "Raft is instantiated but NonVoter.");
            },
            (Status::Down, _) => {},

            (Status::Up, RaftState::NonVoter) => {
                self.do_advance_state(Status::Leaving, "Raft demoted to NonVoter - DOES THIS MEAN LEAVING??")
            },
            (Status::Up, _) => {},

            (Status::Leaving, _) => {},
            (Status::Exiting, _) => {},
            (Status::Removed, _) => {},
        }
    }

    fn do_advance_state(&mut self, new_status: Status, reason: &str) {
        let old_status = self.state.advance(new_status);
        info!(
            network_id = self.id, %old_status, %new_status,
            "Network promoted to {} since {})", new_status, reason
        );
    }
}

#[derive(Clone)]
pub struct BindEndpoint<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    data: PortData<D, R, E, S>
}

impl<D, R, E, S> BindEndpoint<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    pub fn new(data: PortData<D, R, E, S>) -> Self { BindEndpoint { data } }
}

impl<D, R, E, S> Message for BindEndpoint<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = Result<(), NetworkError>;
}

impl<D, R, E, S> std::fmt::Debug for BindEndpoint<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f:&mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BindEndpoint({:?})", self.data)
    }
}

impl<D, R, E, S> Handler<BindEndpoint<D, R, E, S>> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = Result<(), NetworkError>;
    // type Result = ResponseActFuture<Self, (), NetworkError>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, bind: BindEndpoint<D, R, E, S>, _ctx: &mut Self::Context) -> Self::Result {
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


impl<D, R, E, S> Handler<ClientRequest<D, R, E>> for Network<D, R, E, S>
    where
        D: AppData,
        Node<D>: Actor<Context = Context<Node<D>>> + Handler<ClientRequest<D, R, E>>,
        R: AppDataResponse,
        E: AppError,
        E: From<NetworkError> + From<NoneError> + From<MailboxError>,
        S: RaftStorage<D, R, E>,
{
    type Result = ResponseActFuture<Self, R, E>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, request: ClientRequest<D, R, E>, _ctx: &mut Self::Context) -> Self::Result {
        Box::new(
            self.leader_delegate()
                .then(|res, network, _| {
                    let res = match res {
                        Ok(leader) => Ok(leader),

                        Err(NetworkError::NotLeader { leader_id: Some(lid), leader_address }) => {
                            let leader = network.nodes
                                .get(&lid)
                                .and_then(|leader_ref| leader_ref.addr.clone());

                            match leader {
                                Some(leader_node) => Ok(leader_node),
                                None => Err(E::from(NetworkError::NotLeader {
                                    leader_id: Some(lid),
                                    leader_address,
                                })),
                            }
                        },

                        Err(e) => Err(e.into()),
                    };

                    fut::result::<Addr<Node<D>>, E, Self>(res)
                })
                .and_then(move |leader, _, _| {
                    fut::wrap_future::<_, Self>(leader.send(request))
                        .map_err(|err, _, _| E::from(err))
                })
                .and_then(|res, _, _| fut::result(res))
        )
    }
}


#[derive(Debug, Clone)]
pub struct GetConnectedNodes;

impl Message for GetConnectedNodes {
    type Result = Result<HashMap<NodeId, Option<NodeInfo>>, NetworkError>;
}

impl<D, R, E, S> Handler<GetConnectedNodes> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    // type Result = ResponseActFuture<Self, HashMap<NodeId, NodeInfo>, NetworkError>;
    type Result = Result<HashMap<NodeId, Option<NodeInfo>>, NetworkError>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, _msg: GetConnectedNodes, _ctx: &mut Self::Context) -> Self::Result {
        let nodes = self.nodes.iter()
            .map(|(id, node_ref)| (*id, node_ref.info.clone()))
            .collect();
        debug!(network_id = self.id, "Network identified nodes: {:?}", nodes);
        Ok(nodes)
    }
}

#[derive(Debug, Clone)]
pub struct GetNode {
    pub node_id: Option<NodeId>,
    pub cluster_address: Option<String>,
}

impl GetNode {
    pub fn for_id(id: NodeId) -> Self { GetNode { node_id: Some(id), cluster_address: None, } }

    pub fn for_cluster_address<S>(address: String) -> Self where S: AsRef<str>, {
        GetNode { node_id: None, cluster_address: Some(address.to_string()), }
    }
}

impl Message for GetNode {
    type Result = Result<(NodeId, Option<NodeInfo>), NetworkError>;
}

impl<D, R, E, S> Handler<GetNode> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = Result<(NodeId, Option<NodeInfo>), NetworkError>;

    fn handle(&mut self, msg: GetNode, _ctx: &mut Self::Context) -> Self::Result {
        let node_id = msg.node_id
            .unwrap_or_else( || {
                let ring = self.ring.read().unwrap();
                msg.cluster_address
                    .and_then(|addr| ring.get_node(addr).map(|id| *id))
                    .unwrap_or(0)
            });

        let info = self.nodes.get(&node_id).and_then(|r| r.info.clone());
        Ok((node_id, info))
    }
}

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

impl<D, R, E, S> Handler<GetCurrentLeader> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
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

impl<D, R, E, S> Handler<GetClusterSummary> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = Result<ClusterSummary, NetworkError>;

    #[tracing::instrument]
    fn handle(&mut self, msg: GetClusterSummary, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.summarize())
    }
}

impl<D, R, E, S> Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    /// Returns the Addr<Node> if this network is the leader; otherwise if there is consensus, a
    /// NotLeader error referencing the leader or notice there is no elected leader.
    #[tracing::instrument(skip(self))]
    fn leader_delegate(&self) -> impl ActorFuture<Actor = Self, Item = Addr<Node<D>>, Error = NetworkError> {
        fut::result(
            match self.leader_ref() {
                Some(NodeRef{ id, info: _, addr}) if id == &self.id && addr.is_some() => {
                    debug!(network_id = self.id, "Network's node is LEADER.");
                    Ok(addr.clone().unwrap())
                },

                Some(lref) => {
                    let err = Err(
                        if self.id == lref.id {
                            NetworkError::Unknown("Local leader node is not started".to_string())
                        } else if lref.info.is_some() {
                            NetworkError::NotLeader {
                                leader_id: Some(lref.id),
                                leader_address: lref.info.as_ref().map(|info| info.cluster_address.clone()),
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

impl<D, R, E, S> Handler<HandleNodeStatusChange> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
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

impl<D, R, E, S> Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
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

            Some(NodeRef { id: _, info: Some(_), addr: _} ) => {
                info!(
                    network_id = self.id, ?command,
                    "node info changed - re-registering node."
                );
                self.register_node(command.id, command.info, ctx.address())
            },

            None | Some(NodeRef { id: _, info: None, addr: _ }) => {
                info!(
                    network_id = self.id, ?command,
                    "no info previous set - registering node."
                );
                self.register_node(command.id, command.info, ctx.address())
            },
        };

        fut::result(task)
    }

    #[tracing::instrument(skip(self, _ctx))]
    fn delegate_to_leader<M, I, E0>(
        &self,
        command: M,
        _ctx: &mut Context<Self>,
    ) -> impl ActorFuture<Actor = Self, Item = I, Error = E0>
    where
        M: Message + std::fmt::Debug + Send + 'static,
        M::Result: Into<Result<I, E0>>,
        M::Result: Send,
        Node<D>: Actor<Context = Context<Node<D>>> + Handler<M>,
        E0: From<actix::MailboxError>,
        E0: From<NetworkError>,
    {
        debug!(network_id = self.id, ?command, "lookup and delegate command to leader...");

        let command_desc = format!("{:?}", command);
        let command_desc_2 = command_desc.clone();

        Box::new(
            self.leader_delegate()
                .map_err(move |err, network, _| {
                    match err {
                        NetworkError::NotLeader{leader_id: _, leader_address: _} => {
                            warn!(
                                network_id = network.id, command = ?command_desc_2, error = ?err,
                                "Network is *not* elected leader."
                            );
                        },

                        NetworkError::NoElectedLeader => {
                            warn!(
                                network_id = network.id, command = ?command_desc_2, error = ?err,
                                "No elected leader."
                            );
                        },

                        _ => {
                            error!(
                                network_id = network.id, command = ?command_desc_2, error = ?err,
                                "failure while looking for leader."
                            );
                        },
                    };
                    err.into()
                })
                .and_then( move |leader, network, _| {
                    info!(network_id = network.id, ?command, "Sending command to leader...");
                    let command_desc_2 = command_desc.clone();

                    fut::wrap_future::<_, Self>(leader.send(command))
                        .map_err(move |err, network, _| {
                            error!(
                                network_id = network.id, command = ?command_desc_2, error = ?err,
                                "command delegation to leader failed."
                            );
                            err.into()
                        })
                        .and_then(move |res, network, _| {
                            info!(
                                network_id = network.id, command = ?command_desc,
                                "command delegation to leader succeeded."
                            );
                            fut::result(res.into())
                        })
                })
        )
    }
}

impl<D, R, E, S> Handler<ConnectNode> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = ResponseActFuture<Self, Acknowledged, NetworkError>;

    #[tracing::instrument(skip(self, ctx))]
    fn handle(&mut self, command: ConnectNode, ctx: &mut Self::Context) -> Self::Result {
        info!(network_id = self.id, "handling ConnectNode#{} command...", command.id);

        Box::new(
            self.handle_connect_remote_node(command.clone(), ctx)
                .and_then(move |_, network, ctx| {
                    let change_cluster_command = ChangeClusterConfig::new_to_add(vec!(command.id));

                    network.delegate_to_leader(change_cluster_command, ctx)
                        .then(move |res, n, _| {
                            fut::result(
                                match res {
                                    Ok(_) => {
                                        info!(
                                            network_id = n.id,
                                            "Node connection and cluster change ackd by leader."
                                        );

                                        Ok(Acknowledged {})
                                    },

                                    Err(NetworkError::NotLeader{leader_id: _, leader_address: _}) => {
                                        info!(
                                            network_id = n.id,
                                            "Node connection ackd but host is not leader."
                                        );

                                        //todo: finalize how to handle not leader
                                        Ok(Acknowledged {})
                                    },

                                    Err(NetworkError::NoElectedLeader) => {
                                        info!(
                                            network_id = n.id,
                                            "Node connection ackd but there is no elected leader."
                                        );

                                        Ok(Acknowledged {})
                                    },

                                    Err(err) => {
                                        error!(
                                            network_id = n.id,
                                            error = ?err,
                                            "cluster change delegation failed by leader."
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


impl<D, R, E, S> Handler<DisconnectNode> for Network<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Result = Result<Acknowledged, NetworkError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(&mut self, msg: DisconnectNode, _ctx: &mut Self::Context) -> Self::Result {
        let discarded = self.nodes.remove(&msg.0);
        debug!(
            network_id = self.id, disconnected_node_ref = ?discarded,
            "Disconnected node from network."
        );
        Ok(Acknowledged)
    }
}
