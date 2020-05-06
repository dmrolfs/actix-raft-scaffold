mod fixtures;

use std::net::SocketAddr;
use std::collections::HashMap;
use actix::spawn;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tokio::timer::Delay;
use tracing::*;
use mockito::{mock, server_address, Matcher};

use ::config::Config;
use actix::prelude::*;
use actix_raft::*;
use actix_raft_scaffold::{utils, NodeInfo};
use actix_raft_scaffold::ring::Ring;
use actix_raft_scaffold::network::{Network, NetworkError};
use actix_raft_scaffold::network::{BindEndpoint, GetClusterSummary, ConnectNode, GetNode};
use actix_raft_scaffold::network::state::{Extent, Status};
use actix_raft_scaffold::config::Configuration;
use actix_raft_scaffold::fib::FibActor;
use actix_raft_scaffold::ports::{PortData, http::entities};
use actix_raft_scaffold::network::summary::ClusterSummary;


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
    assert_eq!(actual.nodes.is_empty(), false);
    assert_eq!(actual.nodes.len(), 1);
    assert_eq!(actual.nodes.get(&actual.id).unwrap().info.as_ref().unwrap(), &actual.info);
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

    let b_response = entities::RaftProtocolResponse {
        response: Some(entities::raft_protocol_command_response::Response::Result(
            entities::ResponseResult::ConnectionAcknowledged {
                node_id: Some(entities::NodeId { id: node_b_id })
            },
        ))
    };
    let b_connect_ack_json = serde_json::to_string(&b_response).unwrap();
    info!("mock b resp = |{}|", b_connect_ack_json);

    let node_connect_mock = mock("POST", Matcher::Regex(b_path_exp))
        .with_header("content-type", "application/json")
        .with_body(b_connect_ack_json)
        .expect(1)
        .create();
    // let nb_mock = mock("POST", b_path.as_str()).expect(1).create();
    let raft_mock = mock("POST", "/api/cluster/admin").expect(0).create();

    {
        let s = span!(Level::INFO, "build_network");
        let _ = s.enter();

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
                let s = span!(Level::INFO, "assert_results");
                let _ = s.enter();

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
                let s = span!(Level::INFO, "clean_up");
                let _ = s.enter();

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

    let s = span!(Level::INFO, "assert_mock");
    let _ = s.enter();
    info!("ASSERTING MOCK service: {:?}", node_connect_mock);
    node_connect_mock.assert();
    raft_mock.assert();
}

#[derive(Clone)]
struct ConnectNodePrep {
    pub network: Addr<Network>,
    pub network_id: NodeId,
    pub members: HashMap<NodeId, NodeInfo>,
}

#[tracing::instrument]
fn create_and_bind_network() -> (ConnectNodePrep, impl Future<Item = ClusterSummary, Error = NetworkError>) {
    let node_a = make_node_a("[::1]:8888".parse().unwrap());
    let node_a_1 = node_a.clone();
    let node_b = make_node_b(server_address());
    let config = make_test_configuration("node_a", vec![&node_a, &node_b]);

    let node_b_id = crate::utils::generate_node_id(node_b.cluster_address.clone());
    let b_path_exp = format!("/api/cluster/nodes/{}", node_b_id);
    info!("mock b path = {}", b_path_exp);

    let mut network = make_test_network(&node_a);
    network.configure_with(&config);
    let network_addr = network.start();
    let n1 = network_addr.clone();

    let fib_act = FibActor::new();
    let fib_addr = fib_act.start();

    let data = PortData {
        fib: fib_addr,
        network: network_addr.clone(),
    };

    debug!("binding networkk...");
    let task = network_addr.send( BindEndpoint::new(data))
        .map_err(|err| {
            error!("error in bind:{:?}", err);
            panic!(err)
        })
        .and_then( |_| {
            debug!("delaying for a sec...");
            Delay::new( Instant::now() + Duration::from_secs(1)).map_err(|err| panic!(err))
        })
        .and_then(move |_| {
            debug!("getting summary...");
            n1.send(GetClusterSummary).from_err()
        })
        // .from_err()
        .and_then(move |summary| {
            info!("baseline summary:{:?}", summary);
            let actual = summary.unwrap();
            assert_eq!(actual.id, utils::generate_node_id("[::1]:8888"));
            assert_eq!(actual.info, node_a_1);
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);
            assert_eq!(actual.state.unwrap(), Status::Joining);
            assert_eq!(actual.state.extent(), Extent::Cluster);
            assert_eq!(actual.state.connected_nodes().len(), 2);
            debug_assert!(actual.metrics.is_none(), "no raft metrics");
            futures::future::ok::<ClusterSummary, NetworkError>(actual)
        });

    let node_a_id = crate::utils::generate_node_id(node_a.cluster_address.clone());
    let node_b_id = crate::utils::generate_node_id(node_b.cluster_address.clone());
    let mut members = HashMap::new();
    members.insert(node_a_id, node_a);
    members.insert(node_b_id, node_b);

    let prep = ConnectNodePrep {
        network: network_addr,
        network_id: node_a_id,
        members,
    };

    (prep, task)
}

// #[tracing::instrument]
// fn prep_for_connect_node_tests() -> (ConnectNodePrep, impl Future<Item = ClusterSummary, Error = NetworkError>) {
//     let nb_mock = mock( "POST", Matcher::Regex(b_path_exp))
//         .with_header("content-type", "application/json")
//         .with_body(b_connect_ack_json)
//         .expect(1)
//         .create();
//
//     let raft_mock = mock("POST", "/api/cluster/admin").expect(0).create();
//
//         ;
//
// }

#[test]
fn test_cmd_connect_node_no_leader() {
    fixtures::setup_logger();
    let span = span!(Level::DEBUG, "test_cmd_connect_node_no_leader");
    let _ = span.enter();

    let sys = System::builder().stop_on_panic(true).name("test-handle-connect").build();

    let (prep, prep_task) = create_and_bind_network();
    // let n2 = prep.network.clone();

    let (node_b_id, node_b_info) = prep.members.iter().find(|id_info| {
        (*id_info).0 != &prep.network_id
    })
        .map(|id_info|  (*id_info.0, id_info.1.clone()))
        .unwrap();

    let b_path_exp = format!("/api/cluster/nodes/{}", node_b_id);
    info!("mock b path = {}", b_path_exp);
    let b_connect_response = entities::RaftProtocolResponse {
        response: Some(entities::raft_protocol_command_response::Response::Result(
            entities::ResponseResult::ConnectionAcknowledged {
                node_id: Some(entities::NodeId { id: node_b_id })
            },
        ))
    };
    let b_connect_ack_json = serde_json::to_string(&b_connect_response).unwrap();
    info!("mock b resp = |{}|", b_connect_ack_json);

    let nb_mock = mock( "POST", Matcher::Regex(b_path_exp))
        .with_header("content-type", "application/json")
        .with_body(b_connect_ack_json)
        .expect(1)
        .create();

    let raft_mock = mock("POST", "/api/cluster/admin").expect(0).create();

    let network_1 = prep.network.clone();
    let network_2 = prep.network.clone();
    let task = prep_task.and_then(move |_summary| {
        network_1.send(ConnectNode{id: node_b_id, info: node_b_info.clone()}).from_err()
    })
        .and_then(move |_ack| {
            network_2.send(GetClusterSummary).from_err()
        })
        // .from_err()
        .and_then(move |summary| {
            info!("AFTER Connect summary:{:?}", summary);
            // let foo = *prep.members.get(&prep.network_id).unwrap();
            let actual = summary.unwrap();
            assert_eq!(actual.id, utils::generate_node_id("[::1]:8888"));
            assert_eq!(&actual.info, prep.members.get(&prep.network_id).unwrap());
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);
            assert_eq!(actual.state.unwrap(), Status::Joining);
            assert_eq!(actual.state.extent(), Extent::Cluster);
            assert_eq!(actual.state.connected_nodes().len(), 2);
            debug_assert!(actual.metrics.is_none(), "no raft metrics");
            futures::future::ok::<ClusterSummary, NetworkError>(actual)
        })
        .and_then(move |res| {
            nb_mock.assert();
            raft_mock.assert();
            futures::future::ok(res)
        })
        .then(|_| {
            actix::System::current().stop();
            Ok(())
        });

    spawn(task);
    assert!(sys.run().is_ok(), "error during test");
}

#[test]
fn test_cmd_connect_node_change_info() {
    fixtures::setup_logger();
    let span = span!(Level::DEBUG, "test_cmd_connect_node_change_info");
    let _ = span.enter();

    let sys = System::builder().stop_on_panic(true).name("test-handle-connect").build();

    let (prep, prep_task) = create_and_bind_network();
    // let n2 = prep.network.clone();

    let (node_b_id, node_b_info) = prep.members.iter().find(|id_info| {
        (*id_info).0 != &prep.network_id
    })
        .map(|id_info|  (*id_info.0, id_info.1.clone()))
        .unwrap();

    let b_path_exp = format!("/api/cluster/nodes/{}", node_b_id);
    info!("mock b path = {}", b_path_exp);
    let b_connect_response = entities::RaftProtocolResponse {
        response: Some(entities::raft_protocol_command_response::Response::Result(
            entities::ResponseResult::ConnectionAcknowledged {
                node_id: Some(entities::NodeId { id: node_b_id })
            },
        ))
    };
    let b_connect_ack_json = serde_json::to_string(&b_connect_response).unwrap();
    info!("mock b resp = |{}|", b_connect_ack_json);

    let nb_mock = mock( "POST", Matcher::Regex(b_path_exp))
        .with_header("content-type", "application/json")
        .with_body(b_connect_ack_json)
        .expect(2)
        .create();

    let raft_mock = mock("POST", "/api/cluster/admin").expect(0).create();

    let network_1 = prep.network.clone();
    let network_2 = prep.network.clone();
    let network_3 = prep.network.clone();

    let mut changed_node_b = node_b_info.clone();
    changed_node_b.name = "new-node-b".to_string();
    let changed_node_b_expected = changed_node_b.clone();

    let task = prep_task.and_then(move |_summary| {
        // change node_b info on connect
        network_1.send(ConnectNode{id: node_b_id, info: changed_node_b}).from_err()
    })
        .and_then(move |_ack| {
            network_2.send(GetClusterSummary).from_err()
        })
        .and_then(move |summary| {
            info!("AFTER Connect summary:{:?}", summary);
            // let foo = *prep.members.get(&prep.network_id).unwrap();
            let actual = summary.unwrap();
            assert_eq!(actual.id, utils::generate_node_id("[::1]:8888"));
            assert_eq!(&actual.info, prep.members.get(&prep.network_id).unwrap());
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);
            assert_eq!(actual.state.unwrap(), Status::Joining);
            assert_eq!(actual.state.extent(), Extent::Cluster);
            assert_eq!(actual.state.connected_nodes().len(), 2);
            debug_assert!(actual.metrics.is_none(), "no raft metrics");

            network_3.send( GetNode::for_id(node_b_id)).from_err()
        })
        .and_then(move |id_info| {
            let (b_id, b_info) = id_info.unwrap();
            info!("Verifying NodeB changed. GetNode NodeB#{} is: {:?}", b_id, b_info);
            assert_eq!(b_id, node_b_id);
            assert_eq!(b_info.unwrap(), changed_node_b_expected);
            futures::future::ok(())
        })
        .and_then(move |_| {
            nb_mock.assert();
            raft_mock.assert();
            futures::future::ok(())
        })
        .then(|_| {
            actix::System::current().stop();
            Ok(())
        });

    spawn(task);
    assert!(sys.run().is_ok(), "error during test");
}


#[test]
fn test_cmd_connect_node_leader() {
    fixtures::setup_logger();
    let span = span!(Level::DEBUG, "test_cmd_connect_node_leader");
    let _ = span.enter();

    let sys = System::builder().stop_on_panic(true).name("test-handle-connect").build();

    let (prep, prep_task) = create_and_bind_network();

    let (node_b_id, node_b_info) = prep.members.iter().find(|id_info| {
        (*id_info).0 != &prep.network_id
    })
        .map(|id_info|  (*id_info.0, id_info.1.clone()))
        .unwrap();

    let node_a_info = prep.members.get(&prep.network_id).unwrap().clone();
    let b_path_exp = format!("/api/cluster/nodes/{}", node_b_id);
    info!("mock b path = {}", b_path_exp);
    let b_connect_response = entities::RaftProtocolResponse {
        response: Some(entities::raft_protocol_command_response::Response::Result(
            entities::ResponseResult::ConnectionAcknowledged {
                node_id: Some(entities::NodeId { id: node_b_id })
            },
        ))
    };
    let b_connect_ack_json = serde_json::to_string(&b_connect_response).unwrap();
    info!("mock b resp = |{}|", b_connect_ack_json);

    let nb_mock = mock( "POST", Matcher::Regex(b_path_exp))
        .with_header("content-type", "application/json")
        .with_body(b_connect_ack_json)
        .expect(1)
        .create();

    let raft_mock = mock("POST", "/api/cluster/admin").expect(0).create();

    let network_1 = prep.network.clone();
    let network_2 = prep.network.clone();
    let network_3 = prep.network.clone();
    let prep_1 = prep.clone();

    let task = prep_task.and_then(move |_summary| {
        let members = prep_1.members.keys().map(|k| *k).collect();

        let change_leader = RaftMetrics {
            id: prep_1.network_id,
            state: actix_raft::metrics::State::Leader,
            current_term: 1,
            last_log_index: 1,
            last_applied: 1,
            current_leader: Some(prep_1.network_id),
            membership_config: actix_raft::messages::MembershipConfig {
                is_in_joint_consensus: true,
                members,
                non_voters: Vec::new(),
                removing: Vec::new(),
            },
        };

        info!("send change in leader to network...");
        network_1.send(change_leader).from_err()
    })
        .and_then(move |_summary| {
            network_2.send(ConnectNode{id: node_b_id, info: node_b_info}).from_err()
        })
        .and_then(move |_ack| {
            network_3.send(GetClusterSummary).from_err()
        })
        .and_then(move |summary| {
            info!("AFTER Connect summary:{:?}", summary);
            let actual = summary.unwrap();
            assert_eq!(actual.id, utils::generate_node_id("[::1]:8888"));
            assert_eq!(actual.info, node_a_info);
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);
            assert_eq!(actual.state.unwrap(), Status::Joining);
            assert_eq!(actual.state.extent(), Extent::Cluster);
            assert_eq!(actual.state.connected_nodes().len(), 2);
            debug_assert!(actual.metrics.is_none(), "no raft metrics");
            futures::future::ok::<ClusterSummary, NetworkError>(actual)
        })
        .and_then(move |res| {
            nb_mock.assert();
            raft_mock.assert();
            futures::future::ok(res)
        })
        .then(|_| {
            actix::System::current().stop();
            Ok(())
        });

    spawn(task);
    assert!(sys.run().is_ok(), "error during test");
}