use std::sync::Arc;
use futures::{Future, future};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use tracing::*;
use crate::ports::PortData;
use super::entities::*;
use crate::fib::Fibonacci;
use crate::network::{ConnectNode, GetClusterSummary, };
use crate::ports::http::entities::{ConnectionAcknowledged};

// NodeInfoMessage > ChangeClusterMembershipResponse
pub fn join_cluster_route(
    body: web::Json<NodeInfoMessage>,
    req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    //todo
    let nid = node_id_from_path(&req).expect("valid numerical node id");

    if body.node_id.is_some() && body.node_id.unwrap().id != nid {
        error!(
            "Join Cluster Request body node_id {} does not match path {}, which is used",
            body.node_id.unwrap().id, nid
        );
    }

    info!("received register node request for node {:?}:{:?}", nid, body);

    let register_node = ConnectNode {
        id: body.node_id.unwrap().into(),
        info: body.node_info.as_ref().unwrap().clone().into(),
    };


    srv.network
        .send(register_node)
        .map_err( Error::from)
        // .and_then(|res| {
        //     info!("join result = {:?}", res);
        //     res
        // })
        .and_then(move |_res| {
            info!("and now finding fibonacci...");

            srv.fib
                .send(Fibonacci( nid as u32 ))
                .map_err(Error::from)
                .and_then(move |res| {
                    info!("fibonacci response: {:?}", res);
                    let answer = res.unwrap().into();
                    let resp = ChangeClusterMembershipResponse {
                        response: Some(change_cluster_membership_response::Response::Result(
                            ConnectionAcknowledged { node_id: Some(answer), }
                        ))
                    };

                    Ok(HttpResponse::Ok().json(resp))
                })
        })
}

// NodeIdMessage > ChangeClusterMembershipResponse
pub fn leave_cluster_route(
    req: HttpRequest,
    _stream: web::Payload,
    _srv: web::Data<Arc<PortData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    //todo
    let nid = node_id_from_path(&req).expect("valid numerical node id");
    info!("leave cluster request {:?}", nid);
    future::ok(HttpResponse::Ok().json(()))
}

// NodeIdMessage > NodeInfoMessage
pub fn node_route(
    req: HttpRequest,
    _stream: web::Payload,
    _srv: web::Data<Arc<PortData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    //todo
    let nid = node_id_from_path(&req).expect("valid numerical node id");

    info!("get node info {:?}", nid);

    // srv.network
    //     .send( GetNode::new(nid.to_string))
    // srv.network
    //     .send(GetNode(uid.to_string()))
    //     .map_err(Error::from)
    //     .and_then(|res| Ok(HttpResponse::Ok().json(res)))
    //todo
    future::ok(HttpResponse::Ok().json(()))
}

// ClusterNodesRequest > ClusterNodesResponse
pub fn all_nodes_route(
    _req: HttpRequest,
    _stream: web::Payload,
    _srv: web::Data<Arc<PortData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    // srv.network
    //     .send(GetNodes)
    //     .map_err(Error::from)
    //     .and_then(|res| Ok(HttpResponse::Ok().json(res)))
    //todo
    info!("get all nodes");
    let resp = ClusterNodesResponse {
        nodes: std::collections::HashMap::<u64, NodeInfo>::new(),
    };

    future::ok(HttpResponse::Ok().json(resp))
}

// ClusterStateRequest > ClusterStateResponse
#[tracing::instrument(skip(_stream, srv))]
pub fn state_route(
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    srv.network
        .send(GetClusterSummary)
        .map_err(|err| actix_web::error::ErrorInternalServerError(err))
        .and_then(|res| match res {
            Ok(res) => Ok(HttpResponse::Ok().json(res)),
            Err(err) => {Err(
                actix_web::error::ErrorInternalServerError(err)
            )},
        })


            // Ok(HttpResponse::Ok().json(res)))
    // //todo
    // info!("get cluster state");
    // future::ok(HttpResponse::Ok().json(()))
}

// AppendEntries: RaftAppendEntriesRequest > RaftAppendEntriesResponse
pub fn append_entries_route(
    _body: web::Json<RaftAppendEntriesRequest>,
    _req: HttpRequest,
    _stream: web::Payload,
    _srv: web::Data<Arc<PortData>>,
) ->  impl Future<Item = HttpResponse, Error = Error> {
    info!("RAFT append entries");
    future::ok( HttpResponse::Ok().json(()))
}

// InstallSnaphot: RaftInstallSnapshotRequest > RaftInstallSnapshotResponse
pub fn install_snapshot_route(
    _body: web::Json<RaftInstallSnapshotRequest>,
    _req: HttpRequest,
    _stream: web::Payload,
    _srv: web::Data<Arc<PortData>>,
) ->  impl Future<Item = HttpResponse, Error = Error> {
    info!("RAFT install snapshot");
    future::ok( HttpResponse::Ok().json(()))
}

// Vote: RaftVoteRequest > RaftVoteResponse
pub fn vote_route(
    body: web::Json<RaftVoteRequest>,
    _req: HttpRequest,
    _stream: web::Payload,
    _srv: web::Data<Arc<PortData>>,
) ->  impl Future<Item = HttpResponse, Error = Error> {
    info!("RAFT vote");

    let vote_req = body.into_inner();

    let resp = RaftVoteResponse {
        term: vote_req.term,
        vote_granted: false,
        is_candidate_unknown: false,
    };

    future::ok( HttpResponse::Ok().json(resp))
}

fn node_id_from_path( req: &HttpRequest ) -> Result<u64, std::num::ParseIntError> {
    req.match_info()
        .get("uid")
        .unwrap_or("").trim()
        .parse::<u64>()
}
