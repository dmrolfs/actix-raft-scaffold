use std::collections::HashSet;
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
    pub connected_nodes: HashSet<NodeId>,
    pub isolated_nodes: HashSet<NodeId>,
    pub metrics: Option<Metrics>,
}


// impl From<Network> for ClusterSummary {
//     fn from(n: Network) -> Self {
//         Self {
//             id: n.id,
//             state: n.state.clone(),
//             info: n.info.clone(),
//             connected_nodes: n.nodes_connected.clone(),
//             isolated_nodes: n.isolated_nodes.clone(),
//             metrics: n.metrics.into(),
//         }
//     }
// }


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


// impl Serialize for RaftMetrics {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//         where
//             S: Serializer,
//     {
//         // 3 is the number of fields in the struct.
//         let mut state = serializer.serialize_struct("RaftMetrics", 6)?;
//         state.serialize_field("id", &self.id)?;
//         let state_rep = match self.state {
//             actix_raft::metrics::State::NonVoter => "NonVoter",
//             actix_raft::metrics::State::Follower => "Follower",
//             actix_raft::metrics::State::Candidate => "Candidate",
//             actix_raft::metrics::State::Leader => "Leader",
//         };
//         state.serialize_field("state", state_rep)?;
//         state.serialize_field("current_term", &self.current_term)?;
//         state.serialize_field("last_log_index", &self.last_log_index)?;
//         state.serialize_field("last_applied", &self.last_applied)?;
//         state.serialize_field("current_leader", &self.current_leader)?;
//         state.end()
//     }
// }

