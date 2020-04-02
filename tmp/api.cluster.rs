#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Failure {
    #[prost(string, tag = "1")]
    pub description: std::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NodeId {
    #[prost(uint64, tag = "1")]
    pub id: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NodeInfo {
    #[prost(string, tag = "1")]
    pub ui_address: std::string::String,
    #[prost(string, tag = "2")]
    pub app_address: std::string::String,
    #[prost(string, tag = "3")]
    pub cluster_address: std::string::String,
}
/// Raft log entry.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Entry {
    /// This entry's term.
    #[prost(uint64, tag = "1")]
    pub term: u64,
    /// This entry's index.
    #[prost(uint64, tag = "2")]
    pub index: u64,
    /// This entry's payload.
    #[prost(oneof = "entry::Payload", tags = "3, 4, 5, 6")]
    pub payload: ::std::option::Option<entry::Payload>,
}
pub mod entry {
    /// This entry's payload.
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Payload {
        #[prost(message, tag = "3")]
        Normal(super::EntryNormal),
        #[prost(message, tag = "4")]
        ConfigChange(super::MembershipConfig),
        #[prost(message, tag = "5")]
        SnapshotPointer(super::SnapshotPath),
        #[prost(message, tag = "6")]
        Blank(()),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EntryNormal {
    #[prost(message, optional, tag = "1")]
    pub entry: ::std::option::Option<Normal>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Normal {
    #[prost(message, optional, tag = "1")]
    pub data: ::std::option::Option<::prost_types::Any>,
}
/// A model of the membership configuration of the cluster.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MembershipConfig {
    /// A flag indicating if the system is currently in a joint consensus state.
    #[prost(bool, tag = "1")]
    pub is_in_joint_consensus: bool,
    /// Voting members of the Raft cluster.
    #[prost(message, repeated, tag = "2")]
    pub members: ::std::vec::Vec<NodeId>,
    /// Non-voting members of the cluster.
    ///
    /// These nodes are being brought up-to-speed by the leader and will be transitioned over to
    /// being standard members once they are up-to-date.
    #[prost(message, repeated, tag = "3")]
    pub non_voters: ::std::vec::Vec<NodeId>,
    /// The set of nodes which are to be removed after joint consensus is complete.
    #[prost(message, repeated, tag = "4")]
    pub removing: ::std::vec::Vec<NodeId>,
}
/// A log entry pointing to a snapshot.
///
/// This will only be present when read from storage. An entry of this type will never be
/// transmitted from a leader during replication, an `InstallSnapshotRequest`
/// RPC will be sent instead.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SnapshotPath {
    #[prost(string, tag = "1")]
    pub path: std::string::String,
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
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConflictOpt {
    /// The term of the most recent entry which does not conflict with the received request.
    #[prost(uint64, tag = "1")]
    pub term: u64,
    /// The index of the most recent entry which does not conflict with the received request.
    #[prost(uint64, tag = "2")]
    pub index: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NodeIdMessage {
    #[prost(message, optional, tag = "1")]
    pub node_id: ::std::option::Option<NodeId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NodeInfoMessage {
    #[prost(message, optional, tag = "1")]
    pub node_id: ::std::option::Option<NodeId>,
    #[prost(message, optional, tag = "2")]
    pub node_info: ::std::option::Option<NodeInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClusterNodesRequest {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClusterNodesResponse {
    #[prost(map = "uint64, message", tag = "1")]
    pub nodes: ::std::collections::HashMap<u64, NodeInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClusterStateRequest {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClusterStateResponse {
    #[prost(enumeration = "ClusterState", tag = "1")]
    pub state: i32,
}
/// Applications using this Raft implementation are responsible for implementing the
/// networking/transport layer which must move RPCs between nodes. Once the application instance
/// recieves a Raft RPC, it must send the RPC to the Raft node via its `actix::Addr` and then
/// return the response to the original sender.
///
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftAppendEntriesRequest {
    //// A non-standard field, this is the ID of the intended recipient of this RPC.
    #[prost(uint64, tag = "1")]
    pub target: u64,
    //// The leader's current term.
    #[prost(uint64, tag = "2")]
    pub term: u64,
    //// The leader's ID. Useful in redirecting clients.
    #[prost(uint64, tag = "3")]
    pub leader_id: u64,
    //// The index of the log entry immediately preceding the new entries.
    #[prost(uint64, tag = "4")]
    pub prev_log_index: u64,
    //// The term of the `prev_log_index` entry.
    #[prost(uint64, tag = "5")]
    pub prev_log_term: u64,
    //// The new log entries to store.
    ////
    //// This may be empty when the leader is sending heartbeats. Entries
    //// may be batched for efficiency.
    #[prost(message, repeated, tag = "6")]
    pub entries: ::std::vec::Vec<Entry>,
    //// The leader's commit index.
    #[prost(uint64, tag = "7")]
    pub leader_commit: u64,
}
/// The Raft spec assigns no significance to failures during the handling or sending of RPCs
/// and all RPCs are handled in an idempotent fashion, so Raft will almost always retry
/// sending a failed RPC, depending on the state of the Raft.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftAppendEntriesResponse {
    /// The responding node's current term, for leader to update itself.
    #[prost(uint64, tag = "1")]
    pub term: u64,
    /// Will be true if follower contained entry matching `prev_log_index` and `prev_log_term`.
    #[prost(bool, tag = "2")]
    pub success: bool,
    /// A value used to implement the _conflicting term_ optimization outlined in §5.3.
    ///
    /// This value will only be present, and should only be considered, when `success` is `false`.
    #[prost(oneof = "raft_append_entries_response::Conflict", tags = "3")]
    pub conflict: ::std::option::Option<raft_append_entries_response::Conflict>,
}
pub mod raft_append_entries_response {
    /// A value used to implement the _conflicting term_ optimization outlined in §5.3.
    ///
    /// This value will only be present, and should only be considered, when `success` is `false`.
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Conflict {
        #[prost(message, tag = "3")]
        ConflictOpt(super::ConflictOpt),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftInstallSnapshotRequest {
    /// A non-standard field, this is the ID of the intended recipient of this RPC.
    #[prost(uint64, tag = "1")]
    pub target: u64,
    /// The leader's current term.
    #[prost(uint64, tag = "2")]
    pub term: u64,
    /// The leader's ID. Useful in redirecting clients.
    #[prost(uint64, tag = "3")]
    pub leader_id: u64,
    /// The snapshot replaces all log entries up through and including this index.
    #[prost(uint64, tag = "4")]
    pub last_included_index: u64,
    /// The term of the `last_included_index`.
    #[prost(uint64, tag = "5")]
    pub last_included_term: u64,
    /// The byte offset where chunk is positioned in the snapshot file.
    #[prost(uint64, tag = "6")]
    pub offset: u64,
    /// The raw Vec<u8> of the snapshot chunk, starting at `offset`.
    #[prost(bytes, tag = "7")]
    pub data: std::vec::Vec<u8>,
    //// Will be `true` if this is the last chunk in the snapshot.
    #[prost(bool, tag = "8")]
    pub done: bool,
}
/// An RPC response to an `RaftInstallSnapshotResponse` message.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftInstallSnapshotResponse {
    /// The receiving node's current term, for leader to update itself.
    #[prost(uint64, tag = "1")]
    pub term: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftVoteRequest {
    /// A non-standard field, this is the ID of the intended recipient of this RPC.
    #[prost(uint64, tag = "1")]
    pub target: u64,
    /// The candidate's current term.
    #[prost(uint64, tag = "2")]
    pub term: u64,
    /// The candidate's ID.
    #[prost(uint64, tag = "3")]
    pub candidate_id: u64,
    /// The index of the candidate’s last log entry (§5.4).
    #[prost(uint64, tag = "4")]
    pub last_log_index: u64,
    /// The term of the candidate’s last log entry (§5.4).
    #[prost(uint64, tag = "5")]
    pub last_log_term: u64,
}
/// An RPC response to an `RaftVoteResponse` message.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftVoteResponse {
    /// The current term of the responding node, for the candidate to update itself.
    #[prost(uint64, tag = "1")]
    pub term: u64,
    /// Will be true if the candidate received a vote from the responder.
    #[prost(bool, tag = "2")]
    pub vote_granted: bool,
    /// Will be true if the candidate is unknown to the responding node's config.
    ///
    /// If this field is true, and the sender's (the candidate's) index is greater than 0, then it
    /// should revert to the NonVoter state; if the sender's index is 0, then resume campaigning.
    #[prost(bool, tag = "3")]
    pub is_candidate_unknown: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChangeClusterMembershipResponse {
    #[prost(oneof = "change_cluster_membership_response::Response", tags = "1, 2")]
    pub response: ::std::option::Option<change_cluster_membership_response::Response>,
}
pub mod change_cluster_membership_response {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Response {
        #[prost(message, tag = "1")]
        Result(super::ClusterMembershipChange),
        #[prost(message, tag = "2")]
        Failure(super::Failure),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClusterMembershipChange {
    #[prost(message, optional, tag = "1")]
    pub node_id: ::std::option::Option<NodeId>,
    #[prost(enumeration = "MembershipAction", tag = "2")]
    pub action: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ClusterState {
    Initialized = 0,
    SingleNode = 1,
    Cluster = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MembershipAction {
    Added = 0,
    Removed = 1,
}
#[doc = r" Generated client implementations."]
pub mod router_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct RouterClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl RouterClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> RouterClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        pub async fn node_info(
            &mut self,
            request: impl tonic::IntoRequest<super::NodeIdMessage>,
        ) -> Result<tonic::Response<super::NodeInfoMessage>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/NodeInfo");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn cluster_nodes(
            &mut self,
            request: impl tonic::IntoRequest<super::ClusterNodesRequest>,
        ) -> Result<tonic::Response<super::ClusterNodesResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/ClusterNodes");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn cluster_state(
            &mut self,
            request: impl tonic::IntoRequest<super::ClusterStateRequest>,
        ) -> Result<tonic::Response<super::ClusterStateResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/ClusterState");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " An RPC invoked by the leader to replicate log entries (§5.3); also used as heartbeat (§5.2)."]
        pub async fn append_entries(
            &mut self,
            request: impl tonic::IntoRequest<super::RaftAppendEntriesRequest>,
        ) -> Result<tonic::Response<super::RaftAppendEntriesResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/AppendEntries");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " Invoked by the Raft leader to send chunks of a snapshot to a follower (§7)."]
        #[doc = ""]
        #[doc = " The result type of calling with this message type is RaftInstallSnapshotResponse. The Raft"]
        #[doc = " spec assigns no significance to failures during the handling or sending of RPCs and all"]
        #[doc = " RPCs are handled in an idempotent fashion, so Raft will almost always retry sending a"]
        #[doc = " failed RPC, depending on the state of the Raft."]
        pub async fn install_snapshot(
            &mut self,
            request: impl tonic::IntoRequest<super::RaftInstallSnapshotRequest>,
        ) -> Result<tonic::Response<super::RaftInstallSnapshotResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/InstallSnapshot");
            self.inner.unary(request.into_request(), path, codec).await
        }
        #[doc = " An RPC invoked by candidates to gather votes (§5.2)."]
        #[doc = ""]
        #[doc = " The result type of calling the Raft actor with this message type is `RaftVoteResponse`."]
        #[doc = " The Raft spec assigns no significance to failures during the handling or sending of RPCs and"]
        #[doc = " all RPCs are handled in an idempotent fashion, so Raft will almost always retry sending a"]
        #[doc = " failed RPC, depending on the state of the Raft."]
        pub async fn vote(
            &mut self,
            request: impl tonic::IntoRequest<super::RaftVoteRequest>,
        ) -> Result<tonic::Response<super::RaftVoteResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/Vote");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn join_cluster(
            &mut self,
            request: impl tonic::IntoRequest<super::NodeInfoMessage>,
        ) -> Result<tonic::Response<super::ChangeClusterMembershipResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/JoinCluster");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn leave_cluster(
            &mut self,
            request: impl tonic::IntoRequest<super::NodeIdMessage>,
        ) -> Result<tonic::Response<super::ChangeClusterMembershipResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/api.cluster.Router/LeaveCluster");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for RouterClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod router_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with RouterServer."]
    #[async_trait]
    pub trait Router: Send + Sync + 'static {
        async fn node_info(
            &self,
            request: tonic::Request<super::NodeIdMessage>,
        ) -> Result<tonic::Response<super::NodeInfoMessage>, tonic::Status>;
        async fn cluster_nodes(
            &self,
            request: tonic::Request<super::ClusterNodesRequest>,
        ) -> Result<tonic::Response<super::ClusterNodesResponse>, tonic::Status>;
        async fn cluster_state(
            &self,
            request: tonic::Request<super::ClusterStateRequest>,
        ) -> Result<tonic::Response<super::ClusterStateResponse>, tonic::Status>;
        #[doc = " An RPC invoked by the leader to replicate log entries (§5.3); also used as heartbeat (§5.2)."]
        async fn append_entries(
            &self,
            request: tonic::Request<super::RaftAppendEntriesRequest>,
        ) -> Result<tonic::Response<super::RaftAppendEntriesResponse>, tonic::Status>;
        #[doc = " Invoked by the Raft leader to send chunks of a snapshot to a follower (§7)."]
        #[doc = ""]
        #[doc = " The result type of calling with this message type is RaftInstallSnapshotResponse. The Raft"]
        #[doc = " spec assigns no significance to failures during the handling or sending of RPCs and all"]
        #[doc = " RPCs are handled in an idempotent fashion, so Raft will almost always retry sending a"]
        #[doc = " failed RPC, depending on the state of the Raft."]
        async fn install_snapshot(
            &self,
            request: tonic::Request<super::RaftInstallSnapshotRequest>,
        ) -> Result<tonic::Response<super::RaftInstallSnapshotResponse>, tonic::Status>;
        #[doc = " An RPC invoked by candidates to gather votes (§5.2)."]
        #[doc = ""]
        #[doc = " The result type of calling the Raft actor with this message type is `RaftVoteResponse`."]
        #[doc = " The Raft spec assigns no significance to failures during the handling or sending of RPCs and"]
        #[doc = " all RPCs are handled in an idempotent fashion, so Raft will almost always retry sending a"]
        #[doc = " failed RPC, depending on the state of the Raft."]
        async fn vote(
            &self,
            request: tonic::Request<super::RaftVoteRequest>,
        ) -> Result<tonic::Response<super::RaftVoteResponse>, tonic::Status>;
        async fn join_cluster(
            &self,
            request: tonic::Request<super::NodeInfoMessage>,
        ) -> Result<tonic::Response<super::ChangeClusterMembershipResponse>, tonic::Status>;
        async fn leave_cluster(
            &self,
            request: tonic::Request<super::NodeIdMessage>,
        ) -> Result<tonic::Response<super::ChangeClusterMembershipResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    #[doc(hidden)]
    pub struct RouterServer<T: Router> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: Router> RouterServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, None);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, Some(interceptor.into()));
            Self { inner }
        }
    }
    impl<T: Router> Service<http::Request<HyperBody>> for RouterServer<T> {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<HyperBody>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/api.cluster.Router/NodeInfo" => {
                    struct NodeInfoSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::NodeIdMessage> for NodeInfoSvc<T> {
                        type Response = super::NodeInfoMessage;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::NodeIdMessage>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.node_info(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = NodeInfoSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/api.cluster.Router/ClusterNodes" => {
                    struct ClusterNodesSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::ClusterNodesRequest> for ClusterNodesSvc<T> {
                        type Response = super::ClusterNodesResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ClusterNodesRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.cluster_nodes(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = ClusterNodesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/api.cluster.Router/ClusterState" => {
                    struct ClusterStateSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::ClusterStateRequest> for ClusterStateSvc<T> {
                        type Response = super::ClusterStateResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ClusterStateRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.cluster_state(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = ClusterStateSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/api.cluster.Router/AppendEntries" => {
                    struct AppendEntriesSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::RaftAppendEntriesRequest>
                        for AppendEntriesSvc<T>
                    {
                        type Response = super::RaftAppendEntriesResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RaftAppendEntriesRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.append_entries(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = AppendEntriesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/api.cluster.Router/InstallSnapshot" => {
                    struct InstallSnapshotSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::RaftInstallSnapshotRequest>
                        for InstallSnapshotSvc<T>
                    {
                        type Response = super::RaftInstallSnapshotResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RaftInstallSnapshotRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.install_snapshot(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = InstallSnapshotSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/api.cluster.Router/Vote" => {
                    struct VoteSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::RaftVoteRequest> for VoteSvc<T> {
                        type Response = super::RaftVoteResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RaftVoteRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.vote(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = VoteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/api.cluster.Router/JoinCluster" => {
                    struct JoinClusterSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::NodeInfoMessage> for JoinClusterSvc<T> {
                        type Response = super::ChangeClusterMembershipResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::NodeInfoMessage>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.join_cluster(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = JoinClusterSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/api.cluster.Router/LeaveCluster" => {
                    struct LeaveClusterSvc<T: Router>(pub Arc<T>);
                    impl<T: Router> tonic::server::UnaryService<super::NodeIdMessage> for LeaveClusterSvc<T> {
                        type Response = super::ChangeClusterMembershipResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::NodeIdMessage>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { inner.leave_cluster(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = LeaveClusterSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: Router> Clone for RouterServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: Router> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Router> tonic::transport::NamedService for RouterServer<T> {
        const NAME: &'static str = "api.cluster.Router";
    }
}
