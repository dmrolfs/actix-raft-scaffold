use std::sync::Arc;
use actix::prelude::*;
use actix_cors::Cors;
use actix_files as fs;
use actix_web::{
    http::header, middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use tracing::*;
use tracing_subscriber::fmt;
use actix_raft_grpc::{
    fib::FibActor,
    server::ServerData,
    server::http::routes::*,
    server::http::entities::*,
};


// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
#[actix_rt::main]
async fn main() {
    // env_logger::init();
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        // .with_env_filter( "wip=trace" )
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let span = span!( Level::INFO, "wip" );
    let _guard = span.enter();

    // let addr = "[::1]:10000".parse().unwrap();
    // info!("RouteGuideServer listening on: {}", addr);

    let system = System::new("raft");
    let fib_arb = Arbiter::new();
    let fib_act = FibActor::new();
    let fib_addr = FibActor::start_in_arbiter(&fib_arb, |_| fib_act);
    let state = Arc::new( ServerData {
        fib: fib_addr,
    });

    let endpoint_address = "127.0.0.1:8080";

    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::new()
                    .allowed_methods(vec!["GET", "POST", "PUT"])
                    .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                    .allowed_header(header::CONTENT_TYPE)
                    .max_age(3600),
            )
            .wrap(Logger::default())
            .data( state.clone() )
            .service( web::resource("/").route(web::get().to( || {
                HttpResponse::Found()
                    .header( "LOCATION", "/static/index.html")
                    .finish()
            })))
            .service(
                web::scope("/api/cluster")
                    .service(web::resource("/nodes").to_async(all_nodes_route))
                    .service(
                        web::resource("/nodes/{uid}")
                            .route(web::get().to_async(node_route))
                            .route(web::post().to_async(join_cluster_route))
                            .route(web::delete().to_async(leave_cluster_route)),
                    )
                    .service(web::resource("/state").route(web::get().to_async(state_route)))
                    .service(web::resource("/entries").route(web::post().to_async(append_entries_route)))
                    .service(web::resource("/snapshots").route(web::post().to_async(install_snapshot_route)))
                    .service(web::resource("/vote").route(web::post().to_async(vote_route)))
            )
            // static resources
            .service( fs::Files::new("/static/", "static/"))
    })
        .bind(endpoint_address)
        .unwrap()
        .start();


    let _ = system.run();
}