use std::fmt::Debug;
use actix_raft::{NodeId, messages::MembershipConfig};
use serde::{Serialize, Deserialize};
use strum_macros::{EnumString, Display as StrumDisplay};
use crate::NodeInfo;
use super::NetworkState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterSummary {
    pub id: NodeId,
    pub state: NetworkState,
    pub info: NodeInfo,
    pub metrics: Option<Metrics>,
}


/// All possible states of a Raft node.
#[derive(Clone, Debug, PartialEq, Eq, StrumDisplay, EnumString, Serialize, Deserialize)]
pub enum RaftState {
    /// The node is completely passive; replicating entries, but not voting or timing out.
    NonVoter,
    /// The node is actively replicating logs from the leader.
    Follower,
    /// The node has detected an election timeout so is requesting votes to become leader.
    Candidate,
    /// The node is actively functioning as the Raft cluster leader.
    Leader,
}

impl From<actix_raft::metrics::State> for RaftState {
    fn from(s: actix_raft::metrics::State) -> Self {
        match s {
            actix_raft::metrics::State::NonVoter => RaftState::NonVoter,
            actix_raft::metrics::State::Follower => RaftState::Follower,
            actix_raft::metrics::State::Candidate => RaftState::Candidate,
            actix_raft::metrics::State::Leader => RaftState::Leader,
        }
    }
}

/// Baseline metrics of the current state of the subject Raft node.
///
/// See the [module level documentation](https://docs.rs/actix-raft/latest/actix-raft/metrics/index.html)
/// for more details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    /// The ID of the Raft node.
    pub id: NodeId,
    /// The state of the Raft node.
    pub state: RaftState,
    /// The current term of the Raft node.
    pub current_term: u64,
    /// The last log index to be appended to this Raft node's log.
    pub last_log_index: u64,
    /// The last log index to be applied to this Raft node's state machine.
    pub last_applied: u64,
    /// The current cluster leader.
    pub current_leader: Option<NodeId>,
    /// The current membership config of the cluster.
    pub membership_config: MembershipConfig,
}

impl From<actix_raft::RaftMetrics> for Metrics {
    fn from(m: actix_raft::RaftMetrics) -> Self {
        Self {
            id: m.id,
            state: m.state.into(),
            current_term: m.current_term,
            last_log_index: m.last_log_index,
            last_applied: m.last_applied,
            current_leader: m.current_leader,
            membership_config: m.membership_config,
        }
    }
}
