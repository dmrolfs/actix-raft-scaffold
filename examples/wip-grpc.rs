use actix::prelude::*;
// use tonic::transport::Server;
use tracing::*;
use tracing_subscriber::fmt;
// use actix_raft_grpc::cluster::ClusterService;
// use actix_raft_grpc::fib::FibActor;
use actix_raft_grpc::server::Server;

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
    let cluster_act = Server::new();
    let cluster_addr = cluster_act.start();
    system.run();

    // let fib_act = FibActor {};
    // let fib_arb = Arbiter::new();
    // let fib_addr = FibActor.start();
    // let fib_addr = FibActor::start_in_arbiter(&fib_arb, |_| fib_act );
    // let svc = ClusterService::make_service(fib_addr);

    // Server::builder()
    //     .trace_fn(|_| tracing::info_span!("wip_server"))
    //     .add_service(svc)
    //     .serve(addr)
    //     .await;

    // Ok(())
}