use actix::prelude::*;
use tonic::{
    Request, Response, Status,
    transport::{Server as TonicServer}
};
use tracing::*;
use crate::fib::{FibActor, Fibonacci};
use crate::cluster::ClusterService;
use futures::TryFutureExt;
use crate::api::cluster::{
    RaftAppendEntriesRequest, RaftAppendEntriesResponse,
    RaftInstallSnapshotRequest, RaftInstallSnapshotResponse,
    RaftVoteRequest, RaftVoteResponse,
    JoinClusterRequest, LeaveClusterRequest, ChangeClusterMembershipResponse,
    ClusterMembershipChange, MembershipAction,
    NodeId as ApiNodeId,
    router_server::{
        Router,
        RouterServer,
    },
};

pub struct Server {
    fib_addr: Addr<FibActor>,
}

impl Server {
    pub fn new() -> Self {
        let fib_arb = Arbiter::new();
        let fib_act = FibActor {};

        Server {
            fib_addr: FibActor::start_in_arbiter( &fib_arb, |_| fib_act )
        }
    }
}

impl Actor for Server {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = "[::1]:10000".parse().unwrap();
        info!("Raft gRPC endpoint listening on: {}", addr);

        let svc = ClusterService::make_service(self.fib_addr.clone());

        let _ = TonicServer::builder()
            .trace_fn( |_| tracing::info_span!("raft_server"))
            .add_service( svc )
            .serve(addr)
            .unwrap_or_else(|err| {
                error!("Failed to start Raft gRPC endpoint: {:?}", err);
                panic!(err);
            });
    }
}

#[tonic::async_trait]
impl Router for Server {
    // #[tracing::instrument]
    async fn append_entries(
        &self,
        request: Request<RaftAppendEntriesRequest>,
    ) -> Result<tonic::Response<RaftAppendEntriesResponse>, Status> {
        info!("appending to log: {:?}", request);
        unimplemented!()
    }

    // #[tracing::instrument]
    async fn install_snapshot(
        &self,
        request: Request<RaftInstallSnapshotRequest>,
    ) -> Result<Response<RaftInstallSnapshotResponse>, Status> {
        info!("install snapshot: {:?}", request);
        unimplemented!()
    }

    // #[tracing::instrument]
    async fn vote(
        &self,
        request: Request<RaftVoteRequest>,
    ) -> Result<Response<RaftVoteResponse>, Status> {
        info!("vote: {:?}", request);
        unimplemented!()
    }

    // #[tracing::instrument]
    async fn join_cluster(
        &self,
        request: Request<JoinClusterRequest>,
    ) -> Result<Response<ChangeClusterMembershipResponse>, Status> {
        use crate::api::cluster::{
            ClusterMembershipChange,
            MembershipAction,
            change_cluster_membership_response::Response as ApiResponse,
        };


        info!("join cluster: {:?}", request);

        let fib_ask = Fibonacci(7);
        info!("sending off Fib request: {:?}", fib_ask);
        let r = self.fib_addr.send( fib_ask);
        let r1 = r.await.unwrap();
        let r2: u64 = r1.unwrap();
        info!("fib response {:?} = {:?}", fib_ask, r2 );

        let id: u32 = (request.get_ref().node_id.as_ref().map(|nid| nid.id).unwrap_or(0 as u64)) as u32;

        Ok(Response::new( ChangeClusterMembershipResponse {
            response: Some(ApiResponse::Result(
                ClusterMembershipChange {
                    node_id: Some( ApiNodeId { id: 17 as u64 }),
                    action: MembershipAction::Removed as i32,
                }
            )),
        }))
    }

    // #[tracing::instrument]
    async fn leave_cluster(
        &self,
        request: Request<LeaveClusterRequest>,
    ) -> Result<Response<ChangeClusterMembershipResponse>, Status> {
        info!("leave cluster: {:?}", request);
        unimplemented!()
    }
}