use std::sync::Arc;
use std::fmt;
use futures::{Future, future};
use actix::prelude::*;
use actix::Response;
use actix_cors::Cors;
use actix_files as fs;
use actix_web::{
    http::header, middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use actix_raft::NodeId;
use tracing::*;
use crate::{fib::FibActor, server::ServerData};
use super::entities::*;


// NodeInfoMessage > ChangeClusterMembershipResponse
pub fn join_cluster_route(
    join_req: web::Json<NodeInfoMessage>,
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<ServerData>>,
) ->  HttpResponse {
    //todo
    info!("join cluster request:{:?}", join_req);
    // info!("got join request with id {:#?}", node_id);
    // srv.raft.do_send(ChangeRaftClusterConfig(vec![*node_id], vec![]));
    let resp = ChangeClusterMembershipResponse {
        response: Some(change_cluster_membership_response::Response::Result(
            ClusterMembershipChange {
                node_id: join_req.node_id.clone(),
                action: MembershipAction::Added,
            }
        ))
    };

    info!("join cluster resp:{:?}", resp);

    HttpResponse::Ok().json( resp )
}

// NodeIdMessage > ChangeClusterMembershipResponse
pub fn leave_cluster_route(
    req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<ServerData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    //todo
    future::ok(HttpResponse::Ok().json(()))
}

// NodeIdMessage > NodeInfoMessage
pub fn node_route(
    req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<ServerData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    //todo
    let nid = req.match_info().get("uid").unwrap_or("");

    // srv.network
    //     .send(GetNode(uid.to_string()))
    //     .map_err(Error::from)
    //     .and_then(|res| Ok(HttpResponse::Ok().json(res)))
    //todo
    future::ok(HttpResponse::Ok().json(()))
}

// ClusterNodesRequest > ClusterNodesResponse
pub fn nodes_route(
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<ServerData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    // srv.network
    //     .send(GetNodes)
    //     .map_err(Error::from)
    //     .and_then(|res| Ok(HttpResponse::Ok().json(res)))
    //todo
    future::ok(HttpResponse::Ok().json(()))
}

// ClusterStateRequest > ClusterStateResponse
pub fn state_route(
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<ServerData>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    // srv.network
    //     .send(GetClusterState)
    //     .map_err(Error::from)
    //     .and_then(|res| Ok(HttpResponse::Ok().json(res)))
    //todo
    future::ok(HttpResponse::Ok().json(()))
}

// AppendEntries: RaftAppendEntriesRequest > RaftAppendEntriesResponse
// InstallSnaphot: RaftInstallSnapshotRequest > RaftInstallSnapshotResponse
// Note: RaftVoteRequest > RaftVoteResponse
