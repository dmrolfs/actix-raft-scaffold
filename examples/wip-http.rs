use std::sync::Arc;
use actix::prelude::*;
use actix_cors::Cors;
use actix_files as fs;
use actix_web::{
    http::header, middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use tracing::*;
use tracing_subscriber::fmt;
use anyhow::{Result, Context};
use actix_raft_grpc::{
    NodeInfo,
    fib::FibActor,
    ring::Ring,
    network::Network,
    ports::PortData,
    ports::http::routes::*,
    ports::http::entities::*,
    raft_system::*,
    utils,
};

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
#[actix_rt::main]
async fn main() -> Result<()> {
    env_logger::init();
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        // .with_env_filter( "wip=trace" )
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("setting default subscriber failed");

    let span = span!( Level::INFO, "wip" );
    let _guard = span.enter();

    let system = System::new("raft");

    let raft = RaftSystem::new()?;

    let fib_arb = Arbiter::new();
    let fib_act = FibActor::new();
    let fib_addr = FibActor::start_in_arbiter(&fib_arb, |_| fib_act);

    let state = PortData {
        fib: fib_addr,
        network: raft.network.clone(),
    };

    raft.start(state)?;

    let _ = system.run();
    Ok(())
}
