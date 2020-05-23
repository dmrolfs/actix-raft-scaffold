use std::collections::HashMap;
use std::sync::Arc;
use futures::Future;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};
use actix_raft::messages as raft_protocol;
use tracing::*;
use crate::ports::PortData;
use super::entities;
use crate::fib::Fibonacci;
use crate::network::{GetConnectedNodes, ConnectNode, GetClusterSummary, messages::DisconnectNode, GetNode};

fn node_id_from_path( req: &HttpRequest ) -> Result<u64, std::num::ParseIntError> {
    req.match_info()
        .get("uid")
        .unwrap_or("").trim()
        .parse::<u64>()
}

// NodeInfoMessage > ChangeClusterMembershipResponse
#[tracing::instrument(skip(_stream, srv))]
pub fn connect_node_route<D, R, E, S>(
    body: web::Json<entities::NodeInfoMessage>,
    req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) -> impl Future<Item = HttpResponse, Error = Error>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    let nid = node_id_from_path(&req).expect("valid numerical node id");

    if body.node_id.is_some() && body.node_id.unwrap().id != nid {
        error!(
            "Join Cluster Request body node_id {} does not match path {}, which is used",
            body.node_id.unwrap().id, nid
        );
    }

    info!("received register node request for node {:?}:{:?}", nid, body);

    let connect_cmd = ConnectNode {
        id: body.node_id.unwrap().into(),
        info: body.node_info.as_ref().unwrap().clone().into(),
    };


    srv.network
        .send(connect_cmd)
        .map_err( Error::from)
        .and_then(move |_res| {
            info!("and now finding fibonacci...");

            srv.fib
                .send(Fibonacci( nid as u32 ))
                .map_err(Error::from)
                .and_then(move |res| {
                    info!("fibonacci response: {:?}", res);
                    let answer = res.unwrap().into();
                    let resp = entities::RaftProtocolResponse {
                        response: Some(entities::raft_protocol_command_response::Response::Result(
                            entities::ResponseResult::ConnectionAcknowledged {
                                node_id: Some(answer),
                            }
                        ))
                    };

                    Ok(HttpResponse::Ok().json(resp))
                })
        })
}

// NodeIdMessage > ChangeClusterMembershipResponse
#[tracing::instrument(skip(_stream, srv))]
pub fn disconnect_node_route<D, R, E, S>(
    req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) -> impl Future<Item = HttpResponse, Error = Error>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    let nid = node_id_from_path(&req).expect("valid numerical node id");
    info!("disconnect node from network {:?}", nid);

    srv.network
        .send(DisconnectNode(nid))
        .map_err(Error::from)
        .and_then(move |res| match res {
            Ok(_) => Ok(HttpResponse::Ok().finish()),
            Err(err) => Err(actix_web::error::ErrorInternalServerError(err)),
        })
}

// NodeIdMessage > NodeInfoMessage
#[tracing::instrument(skip(_stream, srv))]
pub fn node_route<D, R, E, S>(
    req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) -> impl Future<Item = HttpResponse, Error = Error>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    let nid = node_id_from_path(&req).expect("valid numerical node id");

    info!("get node info {:?}", nid);

    srv.network
        .send(GetNode::for_id(nid))
        .map_err(Error::from)
        .and_then(move |res| match res {
            Ok((node_id, node_info)) => {
                let mut body = HashMap::new();
                let info: Option<entities::NodeInfo> = node_info.map(|info| info.into());
                body.insert(node_id, info);
                Ok(HttpResponse::Ok().json(body))
            },
            Err(err) => Err(actix_web::error::ErrorInternalServerError(err)),
        })
}

// ClusterNodesRequest > ClusterNodesResponse
#[tracing::instrument(skip(_req, _stream, srv))]
pub fn all_nodes_route<D, R, E, S>(
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) -> impl Future<Item = HttpResponse, Error = Error>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    debug!("get all nodes");

    srv.network
        .send(GetConnectedNodes)
        .map_err(Error::from)
        .and_then(|res| match res {
            Ok(res) => Ok(HttpResponse::Ok().json(res)),
            Err(err) => {
                error!(error = ?err, "Failure in retrieving all connected nodes.");
                Err(actix_web::error::ErrorInternalServerError(err))
            }})
}

#[tracing::instrument(skip(_stream, srv))]
pub fn summary_route<D, R, E, S>(
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) -> impl Future<Item = HttpResponse, Error = Error>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    srv.network
        .send(GetClusterSummary)
        .map_err(|err| actix_web::error::ErrorInternalServerError(err))
        .and_then(|res| match res {
            Ok(res) => Ok(HttpResponse::Ok().json(res)),
            Err(err) => { Err(actix_web::error::ErrorInternalServerError(err))},
        })
}

// AppendEntries: RaftAppendEntriesRequest > RaftAppendEntriesResponse
#[tracing::instrument(skip(_stream, srv))]
pub fn append_entries_route<D, R, E, S>(
    body: web::Json<entities::RaftAppendEntriesRequest>,
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) ->  impl Future<Item = HttpResponse, Error = Error>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    info!("RAFT append entries");

    let append_req: raft_protocol::AppendEntriesRequest<D> = body.into_inner().into();

    srv.network
        .send(append_req)
        .map_err(Error::from)
        .and_then(|res| match res {
            Ok(resp) => {
                let payload: entities::RaftAppendEntriesResponse = resp.into();
                HttpResponse::Ok().json(payload)
            },

            Err(_) => HttpResponse::Ok().finish(),
        })

}

// InstallSnaphot: RaftInstallSnapshotRequest > RaftInstallSnapshotResponse
#[tracing::instrument(skip(_stream, srv))]
pub fn install_snapshot_route<D, R, E, S>(
    body: web::Json<entities::RaftInstallSnapshotRequest>,
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) ->  impl Future<Item = HttpResponse, Error = Error>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    info!("RAFT install snapshot");

    let install_req: raft_protocol::InstallSnapshotRequest = body.into_inner().into();

    srv.network
        .send(install_req)
        .map_err(Error::from)
        .and_then(|res| match res {
            Ok(resp) => {
                let payload: entities::RaftInstallSnapshotResponse = resp.into();
                HttpResponse::Ok().json(payload)
            },

            Err(_) => HttpResponse::Ok().finish(),
        })
}

// Vote: RaftVoteRequest > RaftVoteResponse
#[tracing::instrument(skip(_stream, srv))]
pub fn vote_route<D, R, E, S>(
    body: web::Json<entities::RaftVoteRequest>,
    _req: HttpRequest,
    _stream: web::Payload,
    srv: web::Data<Arc<PortData<D, R, E, S>>>,
) ->  impl Future<Item = HttpResponse, Error = Error>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    info!("RAFT vote");

    let vote_req: raft_protocol::VoteRequest = body.into_inner().into();

    srv.network
        .send(vote_req)
        .map_err(Error::from)
        .and_then(|res| match res {
            Ok(resp) => {
                let payload: entities::RaftVoteResponse = resp.into();
                HttpResponse::Ok().json(payload)
            },

            Err(_) => HttpResponse::Ok().finish(),
        })
}

//todo not used since raft admin commands are handled in routes explicitly
//
// #[tracing::instrument(skip(_stream, _srv))]
// pub fn raft_protocol_route<D, R, E, S>(
//     body: web::Json<entities::RaftProtocolCommand>,
//     _req: HttpRequest,
//     _stream: web::Payload,
//     _srv: web::Data<Arc<PortData<D, R, E, S>>>,
// ) -> impl Future<Item = HttpResponse, Error = Error>
//     where
//         D: AppData,
//         R: AppDataResponse,
//         E: AppError,
//         S: RaftStorage<D, R, E>,
// {
//     let command = body.into_inner();
//     info!("RAFT protocol:{:?}", command);
//
//     route_raft_command(command)
//         .map_err(|err| Error::from(err))
//         .map(|_| { entities::raft_protocol_command_response::Response::Result(
//             entities::ResponseResult::Acknowledged
//         )})
//         .map(|resp| HttpResponse::Ok().finish() )
// }
//
// #[tracing::instrument]
// fn route_raft_command(
//     command: entities::RaftProtocolCommand
// ) -> impl Future<Item = (), Error = messages::RaftProtocolError> {
//     error!(raft_command = ?command, "RECEIVED RAFT MESSAGE");
//     match command {
//         entities::RaftProtocolCommand::ProposeConfigChange {add_members, remove_members} => {
//             debug!("routing to ProposeConfigChange...");
//             handle_raft_propose_config_change(add_members, remove_members)
//         },
//     }
// }
//
// #[tracing::instrument]
// fn handle_raft_propose_config_change(
//     add_members: Vec<entities::NodeId>,
//     remove_members: Vec<entities::NodeId>
// ) -> impl Future<Item = (), Error = messages::RaftProtocolError> {
//     error!("RECEIVED RAFT MESSAGE");
//     future::ok(())
// }
