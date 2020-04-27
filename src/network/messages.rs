use std::fmt::Debug;
use actix::Message;
use actix_raft::NodeId;
use serde::{Serialize, Deserialize};
use strum_macros::{Display as StrumDisplay};
use thiserror::Error;
use tracing::*;
use crate::{
    NodeInfo,
    ports,
};


#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("failed to bind network endpoint")]
    NetworkBindError(#[from] ports::PortError),

    #[error("error in delay time {0}")]
    DelayError(#[from] tokio::timer::Error),

    #[error("error in actor mailbox {0}")]
    ActorMailBoxError(#[from] actix::MailboxError),

    #[error("request made to non-leader node (id#{leader_id}, address:{leader_address}")]
    NotLeader {
        leader_id: NodeId,
        leader_address: String,
    },

    #[error("no elected RAFT leader")]
    NoElectedLeader,

    #[error("error in delegated node {0}")]
    NodeError(#[from] super::node::NodeError),

    #[error("unknown network error {0}")]
    Unknown(String),
}



#[derive(Debug, Clone)]
pub struct RegisterNode {
    pub id: NodeId,
    pub info: NodeInfo,
}

impl Message for RegisterNode {
    type Result = Result<ClusterMembershipChange, NetworkError>;
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

#[derive(Serialize, Deserialize, Message, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChangeClusterConfig {
    pub to_add: Vec<NodeId>,
    pub to_remove: Vec<NodeId>,
}

