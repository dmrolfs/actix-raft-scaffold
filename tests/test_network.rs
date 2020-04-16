mod fixtures;

use std::net::SocketAddr;
// use rayon::prelude::*;
use tracing::*;
use ::config::Config;
use actix::prelude::*;
use actix_raft_grpc::{utils, NodeInfo};
use actix_raft_grpc::ring::Ring;
use actix_raft_grpc::network::{Network, NetworkError, NetworkState, BindEndpoint, GetClusterSummary};
use actix_raft_grpc::config::{Configuration, ConfigSchema};
use actix_raft_grpc::fib::FibActor;
use actix_raft_grpc::ports::PortData;
use actix::spawn;

fn make_test_network(node_info: &NodeInfo) -> Network {
    let node_id = utils::generate_node_id(node_info.cluster_address.as_str());
    let ring = Ring::new(10);
    let discovery = "127.0.0.1:8888".parse::<SocketAddr>().unwrap();
    Network::new(node_id, node_info, ring, discovery)
}

fn test_configuration<S>(host: S) -> Configuration
where
    S: AsRef<str> + std::fmt::Debug,
{
    let mut c: Config = Config::default();
    c.set("discovery_host_address", "127.0.0.1:8080");
    c.set("join_strategy", "static");
    c.set("ring_replicas", 10);
    c.set::<Vec<std::collections::HashMap<String, String>>>(
        "nodes",
        vec![
            NodeInfo {
                name: "node_a".to_owned(),
                cluster_address: "127.0.0.1:8000".to_owned(),
                app_address: "127.0.0.1:9000".to_owned(),
                public_address: "127.0.0.1:8080".to_owned(),
            }.into(),
            NodeInfo {
                name: "node_b".to_owned(),
                cluster_address: "127.0.0.1:8001".to_owned(),
                app_address: "127.0.0.1:9001".to_owned(),
                public_address: "127.0.0.1:8081".to_owned(),
            }.into(),
            NodeInfo {
                name: "node_c".to_owned(),
                cluster_address: "127.0.0.1:8002".to_owned(),
                app_address: "127.0.0.1:9002".to_owned(),
                public_address: "127.0.0.1:8082".to_owned(),
            }.into(),
        ]);

    Configuration::load_from_config(host, c).unwrap()
}

#[test]
fn test_network_create() {
    let node_info = NodeInfo {
        name: "test_node".to_owned(),
        cluster_address: "127.0.0.1:8080".to_owned(),
        app_address: "127.0.0.1:9000".to_owned(),
        public_address: "127.0.0.1:80".to_owned(),
    };

    let actual = make_test_network(&node_info);
    assert_eq!(actual.id, utils::generate_node_id("127.0.0.1:8080"));
    assert_eq!(actual.info, node_info);
    assert_eq!(actual.state, NetworkState::Initialized);
    assert_eq!(
        actual.discovery,
        "127.0.0.1:8888".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(actual.nodes.is_empty(), true);
    assert_eq!(actual.nodes_connected.is_empty(), true);
    assert_eq!(actual.isolated_nodes.is_empty(), true);
    assert_eq!(actual.metrics.is_none(), true);
    assert_eq!(actual.server.is_none(), true);
}

#[test]
fn test_configure_network() {
    let node_info = NodeInfo {
        name: "node_a".to_owned(),
        cluster_address: "127.0.0.1:8080".to_owned(),
        app_address: "127.0.0.1:9000".to_owned(),
        public_address: "127.0.0.1:80".to_owned(),
    };

    let config = test_configuration("node_a");

    let mut actual = make_test_network(&node_info);
    actual.configure_with(&config);

    assert_eq!(actual.id, utils::generate_node_id("127.0.0.1:8080"));
    assert_eq!(actual.info, node_info);
    assert_eq!(actual.state, NetworkState::Initialized);
    assert_eq!(
        actual.discovery,
        "127.0.0.1:8888".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(actual.nodes.is_empty(), false);
    assert_eq!(actual.nodes.len(), 3);
    assert_eq!(actual.nodes_connected.is_empty(), true);
    assert_eq!(actual.isolated_nodes.is_empty(), true);
    assert_eq!(actual.metrics.is_none(), true);
    assert_eq!(actual.server.is_none(), true);
}

#[test]
fn test_network_start() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    span.enter();

    let sys = System::builder().stop_on_panic(true).name("test").build();

    let node_info = NodeInfo {
        name: "node_a".to_owned(),
        cluster_address: "127.0.0.1:8080".to_owned(),
        app_address: "127.0.0.1:9000".to_owned(),
        public_address: "127.0.0.1:80".to_owned(),
    };

    let config = test_configuration("node_a");

    let mut network = make_test_network(&node_info);
    network.configure_with(&config);
    let network_addr = network.start();

    // let foo = network.nodes.values().par_iter();
    // assert_eq!(network.nodes.len(), 3);
}

#[test]
fn test_network_bind() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    span.enter();

    // actix::System::run( || {
    //
    // })

    let mut sys = System::builder().stop_on_panic(true).name("test").build();

    let node_info = NodeInfo {
        name: "node_a".to_owned(),
        cluster_address: "127.0.0.1:8080".to_owned(),
        app_address: "127.0.0.1:9000".to_owned(),
        public_address: "127.0.0.1:80".to_owned(),
    };

    let config = test_configuration("node_a");

    let mut network = make_test_network(&node_info);
    network.configure_with(&config);
    let network_addr = network.start();

    let fib_act = FibActor::new();
    let fib_addr = fib_act.start();

    let data = PortData {
        fib: fib_addr,
        network: network_addr.clone(),
    };

    let network2 = network_addr.clone();
    let test = network_addr.send(BindEndpoint::new(data))
        .map_err(|err| {
            error!("error in bind: {:?}", err);
            panic!(err)
        })
        .and_then( move |_| {
            network2.send(GetClusterSummary)
        })
        .map_err(|err| {
            error!("error in get cluster summary: {:?}", err);
            panic!(err)
        })
        .and_then(move |res| {
            let actual = res.unwrap();
            error!("B: actual cluster summary:{:?}", actual);

            assert_eq!(actual.id, utils::generate_node_id("127.0.0.1:8080"));
            warn!("actual.id: {:?}", actual.id);

            assert_eq!(actual.info, node_info);
            warn!("actual.info: {:?}", actual.info);

            assert_eq!(actual.isolated_nodes.is_empty(), true);
            warn!("actual.isolated_nodes: {:?}", actual.isolated_nodes);

            assert_eq!(actual.state, NetworkState::Cluster);
            warn!("actual.state: {:?}", actual.state);

            assert_eq!(actual.connected_nodes.is_empty(), false);
            warn!("actual.connected_nodes: {:?}", actual.connected_nodes);

            Ok(())
        })
        .then(|res| {
            warn!("test finished -- wrapping up");
            actix::System::current().stop();
            res
        });
// ;
    warn!("#### BEFORE BLOCK...");
    // System::current().stop_on_panic();
    // sys.block_on(test);
    // warn!("#### ... AFTER BLOCK");
    spawn(test);

    assert!(sys.run.is_ok(), "error during test");
}