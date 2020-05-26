use std::fmt::Debug;
use actix::Message;
use actix_raft::NodeId;
use serde::{Serialize, Deserialize};
use strum_macros::{Display as StrumDisplay};
use thiserror::Error;
use tracing::*;
use super::node::NodeStatus;
use crate::{
    NodeInfo,
    ports,
    ports::http::entities,
};


#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("failed to bind network endpoint")]
    NetworkBindError(#[from] ports::PortError),

    #[error("error in delay time {0}")]
    DelayError(#[from] tokio::timer::Error),

    #[error("error in actor mailbox {0}")]
    ActorMailBoxError(#[from] actix::MailboxError),

    #[error("request made to non-leader node (id#{leader_id:?}, address:{leader_address:?}")]
    NotLeader {
        leader_id: Option<NodeId>,
        leader_address: Option<String>,
    },

    #[error("no elected RAFT leader")]
    NoElectedLeader,

    #[error("error in delegated node {0}")]
    NodeError(#[from] super::node::NodeError),

    #[error("unknown network error {0}")]
    Unknown(String),
}

#[derive(Error, Debug)]
pub enum RaftProtocolError {
    /// An error related to the processing of the config change request.
    ///
    /// Errors of this type will only come about from the internals of applying the config change
    /// to the Raft log and the process related to that workflow.
    #[error("An error related to the processing of the config change request:{0}")]
    ClientError(String),

    /// The given config would leave the cluster in an inoperable state.
    ///
    /// This error will be returned if the full set of changes, once fully applied, would leave
    /// the cluster with less than two members.
    #[error("The given config would leave the cluster in an inoperable state.")]
    InoperableConfig,

    /// An internal error has taken place.
    ///
    /// These should never normally take place, but if one is encountered, it should be safe to
    /// retry the operation.
    #[error("An internal error has taken place.")]
    Internal,

    /// The node the config change proposal was sent to was not the leader of the cluster.
    ///
    /// If the current cluster leader is known, its ID will be wrapped in this variant.
    #[error("The node the config change proposal was sent to was not the leader of the cluster - leader:{0:?}")]
    NodeNotLeader(Option<NodeId>),

    /// The proposed config changes would make no difference to the current config.
    ///
    /// This takes into account a current joint consensus and the end result of the config.
    ///
    /// This error will be returned if the proposed add & remove elements are empty; all of the
    /// entries to be added already exist in the current config and/or all of the entries to be
    /// removed have already been scheduled for removal and/or do not exist in the current config.
    #[error("The proposed config changes would make no difference to the current config.")]
    Noop,
}

use http::status::StatusCode;

impl actix_web::ResponseError for RaftProtocolError {
    fn error_response(&self) -> actix_http::Response<actix_web::body::Body> {
        match self {
            RaftProtocolError::NodeNotLeader(_id) => {
                actix_http::Response::new(StatusCode::TEMPORARY_REDIRECT)
            },

            _ => {
                actix_http::Response::new(StatusCode::INTERNAL_SERVER_ERROR)
            },
        }
    }
}

impl<D, R, E> From<actix_raft::admin::ProposeConfigChangeError<D, R, E>> for RaftProtocolError
where
    D: actix_raft::AppData,
    R: actix_raft::AppDataResponse,
    E: actix_raft::AppError,
{
    fn from(error: actix_raft::admin::ProposeConfigChangeError<D, R, E>) -> Self {
        match error {
            actix_raft::admin::ProposeConfigChangeError::ClientError(client_err) => {
                let description = format!("{}", client_err);
                RaftProtocolError::ClientError(description)
            },

            actix_raft::admin::ProposeConfigChangeError::InoperableConfig => {
                RaftProtocolError::InoperableConfig
            },

            actix_raft::admin::ProposeConfigChangeError::Internal => {
                RaftProtocolError::Internal
            },

            actix_raft::admin::ProposeConfigChangeError::NodeNotLeader(id) => {
                RaftProtocolError::NodeNotLeader(id)
            },

            actix_raft::admin::ProposeConfigChangeError::Noop => {
                RaftProtocolError::Noop
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleNodeStatusChange {
    pub id: NodeId,
    pub status: NodeStatus,
}

impl Message for HandleNodeStatusChange {
    type Result = ();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectNode {
    pub id: NodeId,
    pub info: NodeInfo,
}

impl Message for ConnectNode {
    type Result = Result<Acknowledged, NetworkError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectNode(pub NodeId);

impl Message for DisconnectNode {
    type Result = Result<Acknowledged, NetworkError>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Acknowledged;

impl From<entities::ResponseResult> for Acknowledged {
    fn from(that: entities::ResponseResult) -> Self {
        match that {
            entities::ResponseResult::ConnectionAcknowledged { node_id: _ } => Self,
            entities::ResponseResult::Acknowledged => Self,
        }
    }
}


#[derive(Debug, StrumDisplay, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum MembershipAction {
    Joining,
    Leaving,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClusterMembershipChange {
    pub node_id: Option<NodeId>,
    pub action: MembershipAction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeClusterConfig {
    pub add_members: Vec<NodeId>,
    pub remove_members: Vec<NodeId>,
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
    type Result = Result<(), NetworkError>;
}
