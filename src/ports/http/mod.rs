use std::sync::Arc;
use actix_server::Server;
use actix_cors::Cors;
use actix_web::{http::header, middleware::Logger, web, App, HttpServer};
use actix_raft::{AppData, AppDataResponse, AppError, RaftStorage};
use super::{PortData, PortError};
use self::routes::*;

pub mod routes;
pub mod entities;

pub fn start_server<S, D, R, E, S0>(
    address: S,
    data: PortData<D, R, E, S0>
) -> Result<Server, PortError>
where
    S: AsRef<str>,
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S0: RaftStorage<D, R, E>,
{
    let server = HttpServer::new( move || {
        App::new()
            .wrap(
                Cors::new()
                    .allowed_methods(vec!["GET", "POST", "PUT"])
                    .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                    .allowed_header(header::CONTENT_TYPE)
                    .max_age(3600),
            )
            .wrap(Logger::default())
            .data( Arc::new(data.clone()) )
        // .service( web::resource("/").route(web::get().to( || {
        //     HttpResponse::Found()
        //         .header( "LOCATION", "/static/index.html")
        //         .finish()
        // })))
            .service(
                web::scope("/api/cluster")
                    // .service( web::resource("/echo").to_async(echo))
                    .service(web::resource("/nodes").to_async(all_nodes_route::<D, R, E, S0>))
                    .service(web::resource("/admin").route(web::post().to_async(raft_protocol_route::<D, R, E, S0>)))
                    .service(
                        web::resource("/nodes/{uid}")
                            .route(web::get().to_async(node_route::<D, R, E, S0>))
                            .route(web::post().to_async(connect_node_route::<D, R, E, S0>))
                            .route(web::delete().to_async(disconnect_node_route::<D, R, E, S0>)),
                    )
                    .service(web::resource("/state").route(web::get().to_async(state_route::<D, R, E, S0>)))
                    .service(web::resource("/entries").route(web::post().to_async(append_entries_route::<D, R, E, S0>)))
                    .service(web::resource("/snapshots").route(web::post().to_async(install_snapshot_route::<D, R, E, S0>)))
                    .service(web::resource("/vote").route(web::post().to_async(vote_route::<D, R, E, S0>)))
            )
        // static resources
        // .service( fs::Files::new("/static/", "static/"))
    })
        .bind( address.as_ref() )?
        .start();

    Ok(server)
}