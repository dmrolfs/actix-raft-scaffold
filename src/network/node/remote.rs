use actix::prelude::*;
use actix_raft::NodeId;
use actix_raft::AppData;
use tracing::*;
use failsafe::{Config, CircuitBreaker, backoff, failure_policy}; //todo: apply circuit breaker
use crate::NodeInfo;
use crate::network::proximity::{ProximityBehavior, ChangeClusterBehavior, ConnectionBehavior};
use crate::network::messages;
use crate::ports::http::entities::{
    self as port_entities,
    raft_protocol_command_response as port_protocol_response,
};
use super::{Node, NodeError};

pub struct RemoteNode {
    pub remote_id: NodeId,
    pub remote_info: NodeInfo,
    pub client: reqwest::Client,
}

impl RemoteNode {
    #[tracing::instrument]
    pub fn new(remote_id: NodeId, remote_info: NodeInfo) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .expect(format!("prebuilt client for RemoteNode#{}", remote_id).as_str());

        Self { remote_id, remote_info, client, }
    }

    pub fn scope(&self) -> String {
        //todo: use encryption
        format!("http://{}/api/cluster", self.remote_info.cluster_address.as_str())
    }
}

impl std::fmt::Display for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode#{}(to:{})", self.remote_id, self.scope())
    }
}

impl std::fmt::Debug for RemoteNode {
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
    #[tracing::instrument(skip(self, _ctx))]
    fn change_cluster_config(
        &self,
        add_members: Vec<NodeId>,
        remove_members: Vec<NodeId>,
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<(), NodeError> {
        error!(proximity = ?self, "RECEIVED CHANGE CLUSTER CONFIG");
        //todo: I think cluster mutating operations should Err(NotLeader).
        unimplemented!();

        let command = port_entities::RaftProtocolCommand::ProposeConfigChange {
            add_members: add_members.iter().map(|m| (*m).into()).collect(),
            remove_members: remove_members.iter().map(|m| (*m).into()).collect(),
        };


        let post_raft_command_route = format!("{}/admin", self.scope());
        debug!(
            proximity = ?self,
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
            .json::<port_entities::RaftProtocolResponse>()
            .map_err(|err| self.convert_error(err))
            .and_then(|cresp| {
                if let Some(response) = cresp.response {
                    match response {
                        port_protocol_response::Response::Result(_) => Ok(()),

                        port_protocol_response::Response::Failure(f) => {
                            Err(NodeError::ResponseFailure(f.description))
                        }

                        port_protocol_response::Response::CommandRejectedNotLeader(leader) => {
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
    fn convert_error(&self, error: reqwest::Error) -> NodeError {
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
                    //todo consider redirection to leader..
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
    ) -> Result<messages::Acknowledged, NodeError> {
        let register_node_route = format!("{}/nodes/{}", self.scope(), self.remote_id);
        debug!(
            proximity = ?self, local_id = local_id_info.0,
            "connect to RemoteNode via {}", register_node_route
        );

        let body = port_entities::NodeInfoMessage {
            node_id: Some(local_id_info.0.into()),
            node_info: Some(local_id_info.1.clone().into()),
        };

        self.client
            .post(&register_node_route)
            .json(&body)
            .send()
            .map_err(|err| self.convert_error(err))?
            .json::<port_entities::RaftProtocolResponse>()
            .map_err(|err| self.convert_error(err))
            .and_then(|cresp| {
                if let Some(response) = cresp.response {
                    match response {
                        port_protocol_response::Response::Result(r) => {
                            let ack: messages::Acknowledged = r.into();
                            Ok(ack)
                        }

                        port_protocol_response::Response::Failure(f) => {
                            Err(NodeError::ResponseFailure(f.description))
                        }

                        port_protocol_response::Response::CommandRejectedNotLeader(leader) => {
                            Err(NodeError::RemoteNotLeaderError {
                                leader_id: leader.leader_id.map(|id| id.into()),
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
        info!(proximity = ?self, "disconnecting RemoteNode");
        //todo WORK HERE
        Ok(())
    }
}
