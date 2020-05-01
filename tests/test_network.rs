mod fixtures;

use std::net::SocketAddr;
use actix::spawn;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tokio::timer::Delay;
use tracing::*;
use mockito::{mock, server_address, Matcher};

use ::config::Config;
use actix::prelude::*;
use actix_raft::NodeId;
use actix_raft_grpc::{utils, NodeInfo};
use actix_raft_grpc::ring::Ring;
use actix_raft_grpc::network::{Network, BindEndpoint, GetClusterSummary};
use actix_raft_grpc::network::state::{Extent, Status};
use actix_raft_grpc::config::Configuration;
use actix_raft_grpc::fib::FibActor;
use actix_raft_grpc::ports::{PortData, http::entities};


const NODE_A_ADDRESS: &str = "127.0.0.1:8000";
const NODE_B_ADDRESS: &str = "127.0.0.1:8001";
const NODE_C_ADDRESS: &str = "127.0.0.1:8002";

fn make_node_a(address: SocketAddr) -> NodeInfo {
    NodeInfo {
        name: "node_a".to_string(),
        cluster_address: address.to_string(), //"127.0.0.1:8000".to_owned(),
        app_address: "127.0.0.1:9000".to_owned(),
        public_address: "127.0.0.1:8080".to_owned(),
    }
}

fn make_node_b(address: SocketAddr) -> NodeInfo {
    NodeInfo {
        name: "node_b".to_string(),
        cluster_address: address.to_string(), //"127.0.0.1:8001".to_owned(),
        app_address: "127.0.0.1:9001".to_owned(),
        public_address: "127.0.0.1:8081".to_owned(),
    }
}

fn make_node_c(address: SocketAddr) -> NodeInfo {
    NodeInfo {
        name: "node_c".to_string(),
        cluster_address: address.to_string(), //"127.0.0.1:8002".to_owned(),
        app_address: "127.0.0.1:9002".to_owned(),
        public_address: "127.0.0.1:8082".to_owned(),
    }
}

fn make_all_nodes() -> Vec<NodeInfo> {
    vec![
        make_node_a(NODE_A_ADDRESS.parse().unwrap()),
        make_node_b(NODE_B_ADDRESS.parse().unwrap()),
        make_node_c(NODE_C_ADDRESS.parse().unwrap()),
    ]
}

fn make_expected_nodes() -> BTreeMap<NodeId, NodeInfo> {
    let expected_a = NodeInfo {
        name: "node_a".to_owned(),
        cluster_address: "127.0.0.1:8000".to_owned(),
        app_address: "127.0.0.1:9000".to_owned(),
        public_address: "127.0.0.1:8080".to_owned(),
    };

    let expected_b = NodeInfo {
        name: "node_b".to_owned(),
        cluster_address: "127.0.0.1:8001".to_owned(),
        app_address: "127.0.0.1:9001".to_owned(),
        public_address: "127.0.0.1:8081".to_owned(),
    };

    let expected_c = NodeInfo {
        name: "node_c".to_owned(),
        cluster_address: "127.0.0.1:8002".to_owned(),
        app_address: "127.0.0.1:9002".to_owned(),
        public_address: "127.0.0.1:8082".to_owned(),
    };

    let mut expected_nodes = BTreeMap::new();
    expected_nodes.insert(utils::generate_node_id(expected_a.clone().cluster_address), expected_a);
    expected_nodes.insert(utils::generate_node_id(expected_b.clone().cluster_address), expected_b);
    expected_nodes.insert(utils::generate_node_id(expected_c.clone().cluster_address), expected_c);
    expected_nodes
}

fn make_test_network(node_info: &NodeInfo) -> Network {
    let node_id = utils::generate_node_id(node_info.cluster_address.as_str());
    let ring = Ring::new(10);
    let discovery = "127.0.0.1:8888".parse::<SocketAddr>().unwrap();
    Network::new(node_id, node_info, ring, discovery)
}

fn make_test_configuration<S>(host: S, nodes: Vec<&NodeInfo>) -> Configuration
where
    S: AsRef<str> + std::fmt::Debug,
{
    let mut c: Config = Config::default();
    c.set("discovery_host_address", "127.0.0.1:8080").unwrap();
    c.set("join_strategy", "static").unwrap();
    c.set("ring_replicas", 10).unwrap();

    c.set::<Vec<std::collections::HashMap<String, String>>>(
        "nodes",
        nodes.iter().map(|n| (*n).clone().into()).collect()
    ).unwrap();

    Configuration::load_from_config(host, c).unwrap()
}

#[test]
fn test_network_create() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    let _ = span.enter();

    let node_info = NodeInfo {
        name: "test_node".to_owned(),
        cluster_address: "127.0.0.1:8080".to_owned(),
        app_address: "127.0.0.1:9090".to_owned(),
        public_address: "127.0.0.1:90".to_owned(),
    };

    let actual = make_test_network(&node_info);
    info!("actual.id:{} expected.id:{}", actual.id, utils::generate_node_id(node_info.clone().cluster_address) );
    assert_eq!(actual.id, utils::generate_node_id(node_info.clone().cluster_address.as_str()));
    assert_eq!(actual.info, node_info);
    assert_eq!(actual.state.unwrap(), Status::Joining);
    assert_eq!(actual.state.extent(), Extent::SingleNode);
    assert_eq!(
        actual.discovery,
        "127.0.0.1:8888".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(actual.nodes.is_empty(), true);
    assert_eq!(actual.state.connected_nodes().is_empty(), true);
    assert_eq!(actual.state.isolated_nodes().is_empty(), true);
    assert_eq!(actual.metrics.is_none(), true);
    assert_eq!(actual.server.is_none(), true);
}

#[test]
fn test_configure_network() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    let _ = span.enter();

    let node_info = make_node_a(NODE_A_ADDRESS.parse().unwrap());
    let nodes = make_all_nodes();
    let nodes_ref = nodes.iter().by_ref().collect();
    let config = make_test_configuration(node_info.clone().name, nodes_ref);

    let mut actual = make_test_network(&node_info);
    actual.configure_with(&config);

    info!("actual.id:{} expected.id:{}", actual.id, utils::generate_node_id(node_info.clone().cluster_address) );
    assert_eq!(actual.id, utils::generate_node_id(node_info.clone().cluster_address) );
    assert_eq!(actual.info, node_info);
    assert_eq!(actual.state.unwrap(), Status::Joining);
    assert_eq!(actual.state.extent(), Extent::SingleNode);
    assert_eq!(
        actual.discovery,
        "127.0.0.1:8888".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(actual.nodes.is_empty(), false);
    assert_eq!(actual.nodes.len(), 3);

    let expected_nodes = make_expected_nodes();

    let actual_nodes = actual.nodes.iter()
        .map( |kv| (*kv.0, kv.1.info.as_ref().unwrap().clone()))
        .collect::<BTreeMap<NodeId, NodeInfo>>();

    assert_eq!(actual_nodes, expected_nodes);

    assert_eq!(actual.state.connected_nodes().is_empty(), true);
    assert_eq!(actual.state.isolated_nodes().is_empty(), true);
    assert_eq!(actual.metrics.is_none(), true);
    assert_eq!(actual.server.is_none(), true);
}

#[test]
fn test_network_start() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    let _ = span.enter();

    let sys = System::builder().stop_on_panic(true).name("test").build();

    let node_info = make_node_a(NODE_A_ADDRESS.parse().unwrap());
    let nodes = make_all_nodes();
    let nodes_ref = nodes.iter().by_ref().collect();
    let config = make_test_configuration("node_a", nodes_ref);
    let mut network = make_test_network(&node_info);
    network.configure_with(&config);
    let network_addr = network.start();

    let test = network_addr.send(GetClusterSummary)
        .map_err(|err| {
            error!("error in get cluster summary: {:?}", err);
            panic!(err)
        })
        .and_then(move |res| {
            let actual = res.unwrap();
            info!("B: actual cluster summary:{:?}", actual);

            info!("actual.id: {:?}", actual.id);
            assert_eq!(actual.id, utils::generate_node_id("127.0.0.1:8000"));

            info!("actual.info: {:?}", actual.info);
            assert_eq!(actual.info, node_info);

            info!("actual.isolated_nodes: {:?}", actual.state.isolated_nodes());
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);

            info!("actual.state: {:?}", actual.state);
            assert_eq!(actual.state.unwrap(), Status::Joining);
            info!("actual.state.extent: {:?}", actual.state.extent());
            assert_eq!(actual.state.extent(), Extent::SingleNode);

            info!("[{:?}] actual.connected_nodes: {:?}", actual.id , actual.state.connected_nodes());
            assert_eq!(actual.state.connected_nodes().is_empty(), true);

            debug_assert!(actual.metrics.is_none(), "no raft metrics");

            // debug_assert!(false, "force failure");
            Ok(())
        })
        .then(|res| {
            info!("test finished -- wrapping up");
            actix::System::current().stop();
            res
        });

    info!("#### BEFORE BLOCK...");
    // System::current().stop_on_panic();
    // sys.block_on(test);
    // info!("#### ... AFTER BLOCK");
    spawn(test);

    assert!(sys.run().is_ok(), "error during test");

}

#[test]
fn test_network_bind() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    let _ = span.enter();

    let sys = System::builder().stop_on_panic(true).name("test").build();

    let node_a = make_node_a(NODE_A_ADDRESS.parse().unwrap());
    let node_b = make_node_b(server_address());

    let config = make_test_configuration("node_a", vec![&node_a, &node_b]);

    let node_b_id = crate::utils::generate_node_id(node_b.cluster_address);
    let b_path_exp = format!("/api/cluster/nodes/{}", node_b_id );
    info!("mock b path = {}", b_path_exp);

    let e_1 = entities::ChangeClusterMembershipResponse {
        response: Some(entities::change_cluster_membership_response::Response::Result(
            entities::ConnectionAcknowledged {
                node_id: Some(entities::NodeId { id: node_b_id }),
                // action: entities::MembershipAction::Added,
            }
        ))
    };
    info!("mock b e_1:{:?} = json:{:?}", e_1, serde_json::to_string(&e_1));

    let b_resp = format!(
        "{}{}{}",
        r#"{"response":{"result":{"nodeId":{"id":"#,
        node_b_id,
        r#"},"action":"Added"}}}"#
    );
    info!("mock b resp = |{}|", b_resp);

    let nb_mock = mock( "POST", Matcher::Regex(b_path_exp))
        .with_header("content-type", "application/json")
        .with_body(b_resp)
        .expect(1)
        .create();
    // let nb_mock = mock("POST", b_path.as_str()).expect(1).create();
    let _nb_bad_mock = mock("POST", "/api/cluster").expect(0).create();

    {
        let mut network = make_test_network(&node_a);
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
            .and_then( |_| {
                Delay::new(Instant::now() + Duration::from_secs(1))
                    .map_err(|err| {panic!(err) })
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
                info!("B: actual cluster summary:{:?}", actual);

                info!("actual.id: {:?}", actual.id);
                assert_eq!(actual.id, utils::generate_node_id("127.0.0.1:8000"));

                info!("actual.info: {:?}", actual.info);
                assert_eq!(actual.info, node_a);

                info!("actual.isolated_nodes: {:?}", actual.state.isolated_nodes());
                assert_eq!(actual.state.isolated_nodes().is_empty(), true);

                // Still in Joining since move to WeaklyUp dependent on
                // RAFT cluster change msg from leader
                info!("actual.state: {:?}", actual.state);
                assert_eq!(actual.state.unwrap(), Status::Joining);

                info!("actual.connected_nodes: {:?}", actual.state.connected_nodes());
                info!("actual.state.extent: {:?}", actual.state.extent());
                // assert_eq!(actual.state.extent, Extent::Initialized); //todo bad
                assert_eq!(actual.state.extent(), Extent::Cluster);

                info!("[{:?}] actual.connected_nodes: {:?}", actual.id ,actual.state.connected_nodes());
                // assert_eq!(actual.connected_nodes.len(), 0); //todo bad
                assert_eq!(actual.state.connected_nodes().len(), 2);

                debug_assert!(actual.metrics.is_none(), "no raft metrics");

                Ok(())
            })
            .then(|res| {
                info!("test finished -- wrapping up");
                actix::System::current().stop();
                res
            });
// ;
        info!("#### BEFORE BLOCK...");
        // System::current().stop_on_panic();
        // sys.block_on(test);
        // info!("#### ... AFTER BLOCK");
        spawn(test);

        assert!(sys.run().is_ok(), "error during test");
    }

    info!("ASSERTING MOCK service: {:?}", nb_mock);
    nb_mock.assert();
}

#[test]
fn test_handle_cmd_connect_node() {
    unimplemented!()
}