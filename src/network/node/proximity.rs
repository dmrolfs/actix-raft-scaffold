use std::fmt::{Debug, Display};
use actix::prelude::*;
use actix_raft::{NodeId, messages as raft_protocol, admin as raft_admin_protocol};
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};
use tracing::*;
use crate::NodeInfo;
use super::{Node, NodeError};
use crate::raft::Raft;
use crate::ports::http::entities::NodeInfoMessage;
use crate::network::messages;
use crate::ports::http::entities::{self, raft_protocol_command_response as entities_response};


//todo: Change to be asynchronous.
pub trait ChangeClusterBehavior<D: AppData> {
    fn change_cluster_config(
        &self,
        add_members: Vec<NodeId>,
        remove_members: Vec<NodeId>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<(), NodeError>;
}

pub trait ConnectionBehavior<D: AppData> {
    //todo: make nonblocking because of distributed network calls
    fn connect(
        &self,
        local_id_info: (NodeId, &NodeInfo),
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<messages::ConnectionAcknowledged, NodeError>;

    //todo: make nonblocking because of distributed network calls
    fn disconnect(&self, _ctx: &mut <Node<D> as Actor>::Context) -> Result<(), NodeError>;
}

pub trait RaftProtocolBehavior<D: AppData> {
    fn append_entries(
        &self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::AppendEntriesResponse, Error = NodeError>>;

    fn install_snapshot(
        &self,
        msg: raft_protocol::InstallSnapshotRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::InstallSnapshotResponse, Error = NodeError>>;

    fn vote(
        &self,
        msg: raft_protocol::VoteRequest,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::VoteResponse, Error = NodeError>>;
}

pub trait ProximityBehavior<D: AppData> :
ChangeClusterBehavior<D> +
ConnectionBehavior<D> +
RaftProtocolBehavior<D> +
Debug + Display
{ }


pub struct LocalNode<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    id: NodeId,
    raft: Addr<Raft<D, R, E, S>>,
}

impl<D, R, E, S> LocalNode<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(raft))]
    pub fn new(id: NodeId, raft: Addr<Raft<D, R, E, S>>) -> Self {
        Self { id, raft }
    }
}

impl<D, R, E, S> std::clone::Clone for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            raft: self.raft.clone(),
        }
    }
}

impl<D, R, E, S> Debug for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "LocalNode#{}", self.id) }
}

impl<D, R, E, S> Display for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "LocalNode#{}", self.id) }
}

impl<D, R, E, S> ProximityBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{}

impl<D, R, E, S> ChangeClusterBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(ctx))]
    fn change_cluster_config(
        &self,
        add_members: Vec<NodeId>,
        remove_members: Vec<NodeId>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<(), NodeError> {
        let _node_addr = ctx.address();
        warn!(proximity = ?self, ?add_members, ?remove_members, "Changing cluster config.");
        let proximity_rep = std::rc::Rc::new(format!("{:?}", self));
        let prep_1 = proximity_rep.clone();
        let prep_2 = proximity_rep.clone();
        let proposal = raft_admin_protocol::ProposeConfigChange::new(
            add_members.clone(),
            remove_members.clone()
        );

        let task = fut::wrap_future(
            self.raft.send(proposal)
                .map_err(move |err| {
                    error!(
                        proximity = ?prep_1, error = ?err,
                        "Failed to call Node actor to propose config changes."
                    );

                    ()
                })
                .and_then(move |res| {
                    match res {
                        Ok(_) => {
                            info!(
                                proximity = ?prep_2, ?add_members, ?remove_members,
                                 "Raft completed cluster config change."
                            );

                            Ok(())
                        },

                        Err(err) => {
                            error!(
                                proximity = ?prep_2, error = ?err,
                                "Failed to call Node actor to propose config changes."
                            );

                            Err(())
                        }
                    }
                })
        );

        ctx.spawn(task);

        Ok(())
    }
}

impl<D, R, E, S> ConnectionBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(self, local_id_info, _ctx))]
    fn connect(
        &self,
        local_id_info: (NodeId, &NodeInfo),
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<messages::ConnectionAcknowledged, NodeError> {
        info!(proximity = ?self, local_id = local_id_info.0, "LocalNode connected");
        Ok(messages::ConnectionAcknowledged {})
    }

    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&self, _ctx: &mut <Node<D> as Actor>::Context) -> Result<(), NodeError> {
        info!(proximity = ?self, "LocalNode disconnected");
        Ok(())
    }
}

impl<D, R, E, S> RaftProtocolBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(self, msg, _ctx))]
    fn append_entries(
        &self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::AppendEntriesResponse, Error = NodeError>> {
        //todo: WORK HERE
        Box::new(
            futures::future::ok(raft_protocol::AppendEntriesResponse {
                term: 1,
                success: true,
                conflict_opt: None,
            })
        )
    }

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn install_snapshot(
        &self,
        msg: raft_protocol::InstallSnapshotRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::InstallSnapshotResponse, Error = NodeError>> {
        //todo WORK HERE
        Box::new(futures::future::ok(raft_protocol::InstallSnapshotResponse { term: 1 }))
    }

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn vote(
        &self,
        msg: raft_protocol::VoteRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::VoteResponse, Error = NodeError>> {
        //todo WORK HERE
        Box::new(
            futures::future::ok(raft_protocol::VoteResponse {
                term: 1,
                vote_granted: true,
                is_candidate_unknown: false,
            })
        )
    }
}


pub struct RemoteNode {
    remote_id: NodeId,
    remote_info: NodeInfo,
    client: reqwest::Client,
}

impl RemoteNode {
    #[tracing::instrument]
    pub fn new(remote_id: NodeId, remote_info: NodeInfo) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .expect(format!("prebuilt client for RemoteNode#{}", remote_id).as_str());

        Self { remote_id, remote_info, client, }
    }

    fn scope(&self) -> String {
        //todo: use encryption
        format!("http://{}/api/cluster", self.remote_info.cluster_address.as_str())
    }
}

impl Display for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode#{}(to:{})", self.remote_id, self.scope())
    }
}

impl Debug for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode#{}(to:{})", self.remote_id, self.scope())
    }
}

impl std::clone::Clone for RemoteNode {
    fn clone(&self) -> Self {
        Self {
            remote_id: self.remote_id,
            remote_info: self.remote_info.clone(),
            client: self.client.clone(),
        }
    }
}

impl<D: AppData> ProximityBehavior<D> for RemoteNode {}

impl<D: AppData> ChangeClusterBehavior<D> for RemoteNode {
    #[tracing::instrument(skip(_ctx))]
    fn change_cluster_config(
        &self,
        add_members: Vec<NodeId>,
        remove_members: Vec<NodeId>,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<(), NodeError> {
        //todo: I think cluster mutating operations should Err(NotLeader).
        unimplemented!();

        let command = entities::RaftProtocolCommand::ProposeConfigChange {
            add_members: add_members.iter().map(|m| (*m).into()).collect(),
            remove_members: remove_members.iter().map(|m| (*m).into()).collect(),
        };


        let post_raft_command_route = format!("{}/admin", self.scope());
        debug!(
            proximity = ?self,
            remote_id = self.remote_id,
            ?command,
            route = post_raft_command_route.as_str(),
            "post Raft protocol command to (leader) RemoteNode."
        );

        self.client
            // .get("https://my-json-server.typicode.com/dmrolfs/json-test-server/connection")
            .post(&post_raft_command_route)
            .json(&command)
            .send()
            .map_err(|err| self.convert_error(err))?
            .json::<entities::RaftProtocolResponse>()
            .map_err(|err| self.convert_error(err))
            .and_then(|cresp| {
                if let Some(response) = cresp.response {
                    match response {
                        entities_response::Response::Result(_) => Ok(()),

                        entities_response::Response::Failure(f) => {
                            Err(NodeError::ResponseFailure(f.description))
                        }

                        entities_response::Response::CommandRejectedNotLeader(leader) => {
                            Err(NodeError::RemoteNotLeaderError {
                                leader_id: leader.leader_id.map(|id| id.into()),
                                // leader_address: Some(leader.leader_address.to_owned()),
                            })
                        }
                    }
                } else {
                    Err(NodeError::Unknown(
                        "good ChangeClusterResponse had empty response".to_string()
                    ))
                }
            })
    }
}


impl RemoteNode {
    fn convert_error( &self, error: reqwest::Error ) -> NodeError {
        match error {
            e if e.is_timeout() => NodeError::Timeout(e.to_string()),
            e if e.is_serialization() => NodeError::ResponseFailure(e.to_string()),
            e if e.is_client_error() => NodeError::RequestError(e),
            e if e.is_http() => NodeError::RequestError(e),
            e if e.is_server_error() => {
                NodeError::ResponseFailure(format!("Error in {:?} server", self))
            },
            e if e.is_redirect() => {
                // need to parse redirect into:
                // NodeError::RemoteNotLeaderError {
                //         leader_id: Option<NodeId>,
                //         leader_address: Option<String>,
                //     }
                NodeError::Unknown(
                    format!(
                        "TODO: PROPERLY HANDLE REDIRECT; E.G., IF REMOTE IS NOT LEADER: {:?}",
                        e
                    )
                )
            },
            e => NodeError::from(e),
        }
    }
}

impl<D: AppData> ConnectionBehavior<D> for RemoteNode {
    #[tracing::instrument(skip(self, local_id_info, _ctx))]
    fn connect(
        &self,
        local_id_info: (NodeId, &NodeInfo),
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<messages::ConnectionAcknowledged, NodeError> {
        //todo WORK HERE
        let register_node_route = format!("{}/nodes/{}", self.scope(), self.remote_id);
        debug!(
            proximity = ?self, local_id = local_id_info.0, remote_id = self.remote_id,
            "connect to RemoteNode via {}", register_node_route
        );

        let body = NodeInfoMessage {
            node_id: Some(local_id_info.0.into()),
            node_info: Some(local_id_info.1.clone().into()),
        };

        self.client
            // .get("https://my-json-server.typicode.com/dmrolfs/json-test-server/connection")
            .post(&register_node_route)
            .json(&body)
            .send()
            .map_err(|err| self.convert_error(err))?
            .json::<entities::RaftProtocolResponse>()
            .map_err(|err| self.convert_error(err))
            .and_then(|cresp| {
                if let Some(response) = cresp.response {
                    match response {
                        entities_response::Response::Result(r) => {
                            let ack: messages::ConnectionAcknowledged = r.into();
                            Ok(ack)
                        }

                        entities_response::Response::Failure(f) => {
                            Err(NodeError::ResponseFailure(f.description))
                        }

                        entities_response::Response::CommandRejectedNotLeader(leader) => {
                            Err(NodeError::RemoteNotLeaderError {
                                leader_id: leader.leader_id.map(|id| id.into()),
                                // leader_address: Some(leader.leader_address.to_owned()),
                            })
                        }
                    }
                } else {
                    Err(NodeError::Unknown(
                        "good ChangeClusterResponse had empty response".to_string()
                    ))
                }
            })
    }


    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&self, _ctx: &mut <Node<D> as Actor>::Context) -> Result<(), NodeError> {
        info!(proximity = ?self, remote_id = self.remote_id, "disconnecting RemoteNode");
        //todo WORK HERE
        Ok(())
    }
}

impl<D: AppData> RaftProtocolBehavior<D> for RemoteNode {
    #[tracing::instrument(skip(self, msg, _ctx))]
    fn append_entries(
        &self,
        msg: raft_protocol::AppendEntriesRequest<D>,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::AppendEntriesResponse, Error = NodeError>> {
        //todo WORK HERE
        Box::new(
            futures::future::ok(raft_protocol::AppendEntriesResponse {
                term: 1,
                success: true,
                conflict_opt: None,
            })
        )
    }

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn install_snapshot(
        &self,
        msg: raft_protocol::InstallSnapshotRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::InstallSnapshotResponse, Error = NodeError>> {
        //todo WORK HERE
        Box::new(futures::future::ok(raft_protocol::InstallSnapshotResponse { term: 1 }))
    }

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn vote(
        &self,
        msg: raft_protocol::VoteRequest,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Box<dyn Future<Item = raft_protocol::VoteResponse, Error = NodeError>> {
        //todo WORK HERE
        Box::new(
            futures::future::ok(raft_protocol::VoteResponse {
                term: 1,
                vote_granted: true,
                is_candidate_unknown: false,
            })
        )
    }
}
