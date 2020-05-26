use serde::{Serialize, Deserialize};
use actix_raft::{AppData, messages as raft_protocol};
use serde_cbor as cbor;
use tracing::*;


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all="camelCase")]
pub enum RaftProtocolCommand {
    ProposeConfigChange { add_members: Vec<NodeId>, remove_members: Vec<NodeId> },
}

// pub mod raft_protocol_command_response {
//     use serde::{Serialize, Deserialize};
//
//     #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
//     #[serde(rename_all="camelCase")]
//     pub(crate) enum RaftProtocolResponse {
//         Acknowledged,
//         Failure(super::Failure),
//         C
//     }
//
// }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Any {
    /// Prost Any
    ///
    /// A URL/resource name that uniquely identifies the type of the serialized
    /// protocol buffer message. This string must contain at least
    /// one "/" character. The last segment of the URL's path must represent
    /// the fully qualified name of the type (as in
    /// `path/google.protobuf.Duration`). The name should be in a canonical form
    /// (e.g., leading "." is not accepted).
    ///
    /// In practice, teams usually precompile into the binary all types that they
    /// expect it to use in the context of Any. However, for URLs which use the
    /// scheme `http`, `https`, or no scheme, one can optionally set up a type
    /// server that maps type URLs to message definitions as follows:
    ///
    /// * If no scheme is provided, `https` is assumed.
    /// * An HTTP GET on the URL must yield a [google.protobuf.Type][]
    ///   value in binary format, or produce an error.
    /// * Applications are allowed to cache lookup results based on the
    ///   URL, or have them precompiled into a binary to avoid any
    ///   lookup. Therefore, binary compatibility needs to be preserved
    ///   on changes to types. (Use versioned type names to manage
    ///   breaking changes.)
    ///
    /// Note: this functionality is not currently available in the official
    /// protobuf release, and it is not used for type URLs beginning with
    /// type.googleapis.com.
    ///
    /// Schemes other than `http`, `https` (or the empty scheme) might be
    /// used with implementation specific semantics.
    ///
    // pub type_url: std::string::String,
    /// Must be a valid serialized protocol buffer of the above specified type.
    pub value: std::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub description: std::string::String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeId {
    pub id: u64,
}

impl NodeId {
    pub fn vec_to_raft(ids: &Vec<NodeId>) -> Vec<actix_raft::NodeId> {
        ids.iter().map(|id| (*id).into()).collect()
    }

    pub fn vec_from_raft(ids: &Vec<actix_raft::NodeId>) -> Vec<NodeId> {
        ids.iter().map(|id| (*id).into()).collect()
    }
}

impl From<actix_raft::NodeId> for NodeId {
    fn from( node_id: actix_raft::NodeId) -> Self { Self { id: node_id, } }
}

impl Into<actix_raft::NodeId> for NodeId {
    fn into(self) -> u64 { self.id }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub name: std::string::String,
    pub cluster_address: std::string::String,
    pub app_address: std::string::String,
    pub public_address: std::string::String,
}

impl From<crate::NodeInfo> for NodeInfo {
    fn from(info: crate::NodeInfo) -> Self {
        Self {
            name: info.name,
            cluster_address: info.cluster_address.to_owned(),
            app_address: info.app_address.to_owned(),
            public_address: info.public_address.to_owned(),
        }
    }
}

impl Into<crate::NodeInfo> for NodeInfo {
    fn into(self) -> crate::NodeInfo {
        crate::NodeInfo {
            name: self.name,
            cluster_address: self.cluster_address.to_owned(),
            app_address: self.app_address.to_owned(),
            public_address: self.public_address.to_owned(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRejectedNotLeader {
    pub leader_id: ::std::option::Option<NodeId>,
    // pub leader_address: std::string::String,
}

/// Raft log entry.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// This entry's term.
    pub term: u64,
    /// This entry's index.
    pub index: u64,
    /// This entry's payload.
    pub payload: ::std::option::Option<entry::Payload>,
}

impl<D: AppData> From<raft_protocol::Entry<D>> for Entry {
    fn from(that: raft_protocol::Entry<D>) -> Self {
        Self {
            term: that.term,
            index: that.index,
            payload: Some(that.payload.into()),
        }
    }
}

impl<D: AppData> Into<raft_protocol::Entry<D>> for Entry {
    fn into(self) -> raft_protocol::Entry<D> {
        if self.payload.is_none() {
            error!( entry = ?self, "Unexpected empty entry payload");
        }

        let payload: raft_protocol::EntryPayload<D> =
            self.payload.map(|p| p.into()).expect("payload populated");

        raft_protocol::Entry {
            term: self.term,
            index: self.index,
            payload,
        }
    }
}

pub mod entry {
    use serde::{Serialize, Deserialize};
    use actix_raft::{AppData, messages as raft_protocol};

    /// This entry's payload.
    #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
    pub enum Payload {
        Normal(super::EntryNormal),
        ConfigChange(super::MembershipConfig),
        SnapshotPointer(super::SnapshotPath),
        Blank(()),
    }

    impl<D:AppData> From<raft_protocol::EntryPayload<D>> for Payload {
        fn from(that: raft_protocol::EntryPayload<D>) -> Self {
            match that {
                raft_protocol::EntryPayload::Blank => Payload::Blank(()),
                raft_protocol::EntryPayload::Normal(entry) => Payload::Normal(entry.into()),
                raft_protocol::EntryPayload::ConfigChange(change) => {
                    Payload::ConfigChange(change.membership.into())
                },
                raft_protocol::EntryPayload::SnapshotPointer(pointer) => {
                    Payload::SnapshotPointer(pointer.into())
                },
            }
        }
    }

    impl<D: AppData> Into<raft_protocol::EntryPayload<D>> for Payload {
        fn into(self) -> raft_protocol::EntryPayload<D> {
            match self {
                Payload::Blank(_) => raft_protocol::EntryPayload::Blank,
                Payload::Normal(entry) => {
                    raft_protocol::EntryPayload::Normal(entry.into())
                },
                Payload::ConfigChange(change) => {
                    raft_protocol::EntryPayload::ConfigChange(
                        raft_protocol::EntryConfigChange { membership: change.into() }
                    )
                },
                Payload::SnapshotPointer(pointer) => {
                    raft_protocol::EntryPayload::SnapshotPointer(pointer.into())
                },
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryNormal {
    pub entry: ::std::option::Option<Normal>,
}

impl<D: AppData> From<raft_protocol::EntryNormal<D>> for EntryNormal {
    fn from(that: raft_protocol::EntryNormal<D>) -> Self {
        match cbor::to_vec(&that.data) {
            Err(err) => {
                error!(that = ?that, error = ?err, "Failed to serialize data entry");
                panic!(format!("Failed to serialize data entry: {:?}", err));
            },

            Ok(bytes) => {
                Self {
                    entry: Some(Normal {
                        data: Some(Any { value: bytes })
                    })
                }
            }
        }
    }
}

impl<D: AppData> Into<raft_protocol::EntryNormal<D>> for EntryNormal {
    fn into(self) -> raft_protocol::EntryNormal<D> {
        // if self.entry.is_none() || self.entry.unwrap().data.is_none() {
        //     error!(entry = ?self, "Unexpected empty normal entry.");
        // }

        let bytes = &self.entry.as_ref().unwrap().data.as_ref().unwrap().value;
        let data = match cbor::from_slice::<D>(bytes.as_slice()) {
            Ok(d) => d,
            Err(err) => {
                error!(entry = ?self, error = ?err, "Unexpected empty normal entry.");
                panic!(format!("failed to deserialize normal data payload: {:?}", err));
            }
        };

        raft_protocol::EntryNormal { data }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Normal {
    pub data: ::std::option::Option<Any>,
}

impl Normal {
    pub fn new(data: Any) -> Self { Self { data: Some(data) } }
}

// impl<D: AppData> From<D> for Normal {
//     fn from(that: D) -> Self {
//         let data = cbor::to_vec(&that);
//         match data {
//             Err(err) => {
//                 error!(that = ?that, error = ?err, "Failed to serialize data entry");
//                 panic!(format!("Failed to serialize data entry: {:?}", err));
//             },
//
//             Ok(bytes) => Self { data: Some(Any { value: bytes }) },
//         }
//     }
// }
//
//
// impl<D: AppData> Into<D> for Normal {
//     fn into(self) -> D {
//         if self.data.is_none() {
//             error!(entry = ?self, "Expected entry data to exist.");
//         }
//
//         let bytes = self.data.unwrap().value;
//         cbor::from_slice(bytes.as_slice())
//     }
// }


/// A model of the membership configuration of the cluster.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipConfig {
    /// A flag indicating if the system is currently in a joint consensus state.
    pub is_in_joint_consensus: bool,
    /// Voting members of the Raft cluster.
    pub members: ::std::vec::Vec<NodeId>,
    /// Non-voting members of the cluster.
    ///
    /// These nodes are being brought up-to-speed by the leader and will be transitioned over to
    /// being standard members once they are up-to-date.
    pub non_voters: ::std::vec::Vec<NodeId>,
    /// The set of nodes which are to be removed after joint consensus is complete.
    pub removing: ::std::vec::Vec<NodeId>,
}

impl From<raft_protocol::MembershipConfig> for MembershipConfig {
    fn from(that: raft_protocol::MembershipConfig) -> Self {
        Self {
            is_in_joint_consensus: that.is_in_joint_consensus,
            members: NodeId::vec_from_raft(&that.members),
            non_voters: NodeId::vec_from_raft(&that.non_voters),
            removing: NodeId::vec_from_raft(&that.removing),
            // members: that.members.iter().map(|id| id.into()).collect(),
            // non_voters: that.non_voters.iter().map(|id| id.into()).collect(),
            // removing: that.removing.iter().map(|id| id.into()).collect(),
        }
    }
}

impl Into<raft_protocol::MembershipConfig> for MembershipConfig {
    fn into(self) -> raft_protocol::MembershipConfig {
        raft_protocol::MembershipConfig {
            is_in_joint_consensus: self.is_in_joint_consensus,
            members: NodeId::vec_to_raft(&self.members),
            non_voters: NodeId::vec_to_raft(&self.non_voters),
            removing: NodeId::vec_to_raft(&self.removing),
            // members: self.members.iter().map(|id| id.into()).collect(),
            // non_voters: self.non_voters.iter().map(|id| id.into()).collect(),
            // removing: self.removing.iter().map(|id| id.into()).collect(),
        }
    }
}

/// A log entry pointing to a snapshot.
///
/// This will only be present when read from storage. An entry of this type will never be
/// transmitted from a leader during replication, an `InstallSnapshotRequest`
/// RPC will be sent instead.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotPath {
    pub path: std::string::String,
}

impl From<raft_protocol::EntrySnapshotPointer> for SnapshotPath {
    fn from(that: raft_protocol::EntrySnapshotPointer) -> Self { Self { path: that.path } }
}

impl Into<raft_protocol::EntrySnapshotPointer> for SnapshotPath {
    fn into(self) -> raft_protocol::EntrySnapshotPointer {
        raft_protocol::EntrySnapshotPointer { path: self.path }
    }
}

/// A struct used to implement the _conflicting term_ optimization outlined in §5.3 for
/// log replication.
///
/// This value will only be present, and should only be considered, when an
/// `AppendEntriesResponse` object has a `success` value of `false`.
///
/// This implementation of Raft uses this value to more quickly synchronize a leader with its
/// followers which may be some distance behind in replication, may have conflicting entries, or
/// which may be new to the cluster.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictOpt {
    /// The term of the most recent entry which does not conflict with the received request.
    pub term: u64,
    /// The index of the most recent entry which does not conflict with the received request.
    pub index: u64,
}

impl From<raft_protocol::ConflictOpt> for ConflictOpt {
    fn from(that: raft_protocol::ConflictOpt) -> Self {
        Self {
            term: that.term,
            index: that.index,
        }
    }
}

impl Into<raft_protocol::ConflictOpt> for ConflictOpt {
    fn into(self) -> raft_protocol::ConflictOpt {
        raft_protocol::ConflictOpt {
            term: self.term,
            index: self.index,
        }
    }
}


#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIdMessage {
    pub node_id: ::std::option::Option<NodeId>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfoMessage {
    pub node_id: ::std::option::Option<NodeId>,
    pub node_info: ::std::option::Option<NodeInfo>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterNodesRequest {}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterNodesResponse {
    pub nodes: ::std::collections::HashMap<u64, NodeInfo>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterStateRequest {}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterStateResponse {
    pub state: ClusterState,
}

/// Applications using this Raft implementation are responsible for implementing the
/// networking/transport layer which must move RPCs between nodes. Once the application instance
/// recieves a Raft RPC, it must send the RPC to the Raft node via its `actix::Addr` and then
/// return the response to the original sender.
///
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftAppendEntriesRequest {
    /// A non-standard field, this is the ID of the intended recipient of this RPC.
    pub target: u64,
    /// The leader's current term.
    pub term: u64,
    /// The leader's ID. Useful in redirecting clients.
    pub leader_id: u64,
    /// The index of the log entry immediately preceding the new entries.
    pub prev_log_index: u64,
    /// The term of the `prev_log_index` entry.
    pub prev_log_term: u64,
    /// The new log entries to store.
    ///
    /// This may be empty when the leader is sending heartbeats. Entries
    /// may be batched for efficiency.
    pub entries: ::std::vec::Vec<Entry>,
    /// The leader's commit index.
    pub leader_commit: u64,
}

impl<D: AppData> From<raft_protocol::AppendEntriesRequest<D>> for RaftAppendEntriesRequest {
    fn from(that: raft_protocol::AppendEntriesRequest<D>) -> Self {
        Self {
            target: that.target,
            term: that.term,
            leader_id: that.leader_id,
            prev_log_index: that.prev_log_index,
            prev_log_term: that.prev_log_term,
            leader_commit: that.leader_commit,
            entries: that.entries.into_iter().map(|e| e.into()).collect(),
        }
    }
}

impl<D: AppData> Into<raft_protocol::AppendEntriesRequest<D>> for RaftAppendEntriesRequest {
    fn into(self) -> raft_protocol::AppendEntriesRequest<D> {
        raft_protocol::AppendEntriesRequest {
            target: self.target,
            term: self.term,
            leader_id: self.leader_id,
            prev_log_index: self.prev_log_index,
            prev_log_term: self.prev_log_term,
            leader_commit: self.leader_commit,
            entries: self.entries.into_iter().map(|e| e.into()).collect(),
        }
    }
}


/// The Raft spec assigns no significance to failures during the handling or sending of RPCs
/// and all RPCs are handled in an idempotent fashion, so Raft will almost always retry
/// sending a failed RPC, depending on the state of the Raft.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftAppendEntriesResponse {
    /// The responding node's current term, for leader to update itself.
    pub term: u64,
    /// Will be true if follower contained entry matching `prev_log_index` and `prev_log_term`.
    pub success: bool,
    /// A value used to implement the _conflicting term_ optimization outlined in §5.3.
    ///
    /// This value will only be present, and should only be considered, when `success` is `false`.
    pub conflict: ::std::option::Option<raft_append_entries_response::Conflict>,
}

impl Into<raft_protocol::AppendEntriesResponse> for RaftAppendEntriesResponse {
    fn into(self) -> raft_protocol::AppendEntriesResponse {
        raft_protocol::AppendEntriesResponse {
            term: self.term,
            success: self.success,
            conflict_opt: self.conflict.map(|c| c.into()),
        }
    }
}

impl From<raft_protocol::AppendEntriesResponse> for RaftAppendEntriesResponse {
    fn from(that: raft_protocol::AppendEntriesResponse) -> Self {
        Self {
            term: that.term,
            success: that.success,
            conflict: that.conflict_opt.map(|c| c.into()),
        }
    }
}

pub mod raft_append_entries_response {
    use serde::{Serialize, Deserialize};
    use actix_raft::messages as raft_protocol;

    /// A value used to implement the _conflicting term_ optimization outlined in §5.3.
    ///
    /// This value will only be present, and should only be considered, when `success` is `false`.
    #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
    pub enum Conflict {
        ConflictOpt(super::ConflictOpt),
    }

    impl From<raft_protocol::ConflictOpt> for Conflict {
        fn from(that: raft_protocol::ConflictOpt) -> Self {
            Conflict::ConflictOpt(that.into())
        }
    }

    impl Into<raft_protocol::ConflictOpt> for Conflict {
        fn into(self) -> raft_protocol::ConflictOpt {
            match self {
                Conflict::ConflictOpt(c) => c.into(),
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftInstallSnapshotRequest {
    /// A non-standard field, this is the ID of the intended recipient of this RPC.
    pub target: u64,
    /// The leader's current term.
    pub term: u64,
    /// The leader's ID. Useful in redirecting clients.
    pub leader_id: u64,
    /// The snapshot replaces all log entries up through and including this index.
    pub last_included_index: u64,
    /// The term of the `last_included_index`.
    pub last_included_term: u64,
    /// The byte offset where chunk is positioned in the snapshot file.
    pub offset: u64,
    /// The raw Vec<u8> of the snapshot chunk, starting at `offset`.
    pub data: std::vec::Vec<u8>,
    /// Will be `true` if this is the last chunk in the snapshot.
    pub done: bool,
}

impl From<raft_protocol::InstallSnapshotRequest> for RaftInstallSnapshotRequest {
    fn from(that: raft_protocol::InstallSnapshotRequest) -> Self {
        Self {
            target: that.target,
            term: that.term,
            leader_id: that.leader_id,
            last_included_index: that.last_included_index,
            last_included_term: that.last_included_term,
            offset: that.offset,
            data: that.data,
            done: that.done,
        }
    }
}

impl Into<raft_protocol::InstallSnapshotRequest> for RaftInstallSnapshotRequest {
    fn into(self) -> raft_protocol::InstallSnapshotRequest {
        raft_protocol::InstallSnapshotRequest {
            target: self.target,
            term: self.term,
            leader_id: self.leader_id,
            last_included_index: self.last_included_index,
            last_included_term: self.last_included_term,
            offset: self.offset,
            data: self.data,
            done: self.done,
        }
    }
}


/// An RPC response to an `RaftInstallSnapshotResponse` message.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftInstallSnapshotResponse {
    /// The receiving node's current term, for leader to update itself.
    pub term: u64,
}

impl Into<raft_protocol::InstallSnapshotResponse> for RaftInstallSnapshotResponse {
    fn into(self) -> raft_protocol::InstallSnapshotResponse {
        raft_protocol::InstallSnapshotResponse { term: self.term }
    }
}

impl From<raft_protocol::InstallSnapshotResponse> for RaftInstallSnapshotResponse {
    fn from(that: raft_protocol::InstallSnapshotResponse) -> Self { Self { term: that.term } }
}


#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftVoteRequest {
    /// A non-standard field, this is the ID of the intended recipient of this RPC.
    pub target: u64,
    /// The candidate's current term.
    pub term: u64,
    /// The candidate's ID.
    pub candidate_id: u64,
    /// The index of the candidate’s last log entry (§5.4).
    pub last_log_index: u64,
    /// The term of the candidate’s last log entry (§5.4).
    pub last_log_term: u64,
}

impl From<raft_protocol::VoteRequest> for RaftVoteRequest {
    fn from(that: raft_protocol::VoteRequest) -> Self {
        Self {
            target: that.target,
            term: that.term,
            candidate_id: that.candidate_id,
            last_log_index: that.last_log_index,
            last_log_term: that.last_log_term,
        }
    }
}

impl Into<raft_protocol::VoteRequest> for RaftVoteRequest {
    fn into(self) -> raft_protocol::VoteRequest {
        raft_protocol::VoteRequest::new(
            self.target,
            self.term,
            self.candidate_id,
            self.last_log_index,
            self.last_log_term
        )
    }
}

/// An RPC response to an `RaftVoteResponse` message.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftVoteResponse {
    /// The current term of the responding node, for the candidate to update itself.
    pub term: u64,
    /// Will be true if the candidate received a vote from the responder.
    pub vote_granted: bool,
    /// Will be true if the candidate is unknown to the responding node's config.
    ///
    /// If this field is true, and the sender's (the candidate's) index is greater than 0, then it
    /// should revert to the NonVoter state; if the sender's index is 0, then resume campaigning.
    pub is_candidate_unknown: bool,
}

impl Into<raft_protocol::VoteResponse> for RaftVoteResponse {
    fn into(self) -> raft_protocol::VoteResponse {
        raft_protocol::VoteResponse {
            term: self.term,
            vote_granted: self.vote_granted,
            is_candidate_unknown: self.is_candidate_unknown,
        }
    }
}

impl From<raft_protocol::VoteResponse> for RaftVoteResponse {
    fn from(that: raft_protocol::VoteResponse) -> Self {
        Self {
            term: that.term,
            vote_granted: that.vote_granted,
            is_candidate_unknown: that.is_candidate_unknown,
        }
    }
}

// #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
// pub struct JoinClusterRequest {
//     pub node_id: ::std::option::Option<NodeId>,
//     pub node_info: ::std::option::Option<NodeInfo>,
// }

// #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
// pub struct LeaveClusterRequest {
//     pub node_id: ::std::option::Option<NodeId>,
// }

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftProtocolResponse {
    pub response: ::std::option::Option<raft_protocol_command_response::Response>,
}

pub mod raft_protocol_command_response {
    use serde::{Serialize, Deserialize};
    use crate::network::messages::RaftProtocolError;
    use crate::ports::http::entities;

    #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum Response {
        Result(super::ResponseResult),
        Failure(super::Failure),
        CommandRejectedNotLeader(super::CommandRejectedNotLeader),
    }

    impl From<RaftProtocolError> for Response {
        fn from(error: RaftProtocolError) -> Self { match error {
            RaftProtocolError::NodeNotLeader(id) => {
                Response::CommandRejectedNotLeader(
                    entities::CommandRejectedNotLeader { leader_id: id.map(|i| i.into())}
                )
            },

            RaftProtocolError::ClientError(description) => {
                Response::Failure(entities::Failure { description })
            },

            err => {
                Response::Failure(entities::Failure { description: err.to_string() })
            }
        }}
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResponseResult {
    Acknowledged,
    ConnectionAcknowledged { node_id: ::std::option::Option<NodeId>, },
}

// impl Into<crate::network::messages::ConnectionAcknowledged> for ConnectionAcknowledged {
//     fn into(self) -> crate::network::messages::ConnectionAcknowledged {
//         crate::network::messages::ConnectionAcknowledged {}
//     }
// }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum ClusterState {
    Initialized = 0,
    SingleNode = 1,
    Cluster = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum MembershipAction {
    Added = 0,
    Removed = 1,
}

impl From<crate::network::messages::MembershipAction> for MembershipAction {
    fn from(that: crate::network::messages::MembershipAction) -> Self {
        match that {
            crate::network::messages::MembershipAction::Joining => MembershipAction::Added,
            crate::network::messages::MembershipAction::Leaving => MembershipAction::Removed,
        }
    }
}

impl Into<crate::network::messages::MembershipAction> for MembershipAction {
    fn into(self) -> crate::network::messages::MembershipAction {
        match self {
            MembershipAction::Added => crate::network::messages::MembershipAction::Joining,
            MembershipAction::Removed => crate::network::messages::MembershipAction::Leaving,
        }
    }
}