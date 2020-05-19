mod fixtures;

use std::net::SocketAddr;
use std::collections::{HashMap, HashSet};
use actix::spawn;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tokio::timer::Delay;
use tracing::*;
use mockito::{mock, server_address, Matcher};
use lazy_static::lazy_static;
use ::config::Config;
use actix::prelude::*;
use actix_raft::*;
use actix_raft_scaffold::{
    utils,
    NodeInfo,
    ring::Ring,
    network::{Network, NetworkError},
    network::{GetClusterSummary, ConnectNode, GetNode},
    network::state::{Extent, Status},
    config::Configuration,
    ports::http::entities,
    raft::RaftSystemBuilder,
};

use actix_raft_scaffold::network::summary::{ClusterSummary, RaftState};
use crate::fixtures::memory_storage::{
    Data, Response, Error, Storage,
    MemoryStorageFactory,
};
use actix_raft_scaffold::raft::system::RaftSystem;


const LOCALHOST: &str = "127.0.0.1";
// const NODE_B_ADDRESS: &str = "127.0.0.1";
// const NODE_C_ADDRESS: &str = "127.0.0.1";

type TestNetwork = Network<Data, Response, Error, Storage>;

static PORT_OFFSET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn port_offset_get_and_increment() -> usize {
    PORT_OFFSET.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn make_node_from_address(name: &str, address: SocketAddr, offset: usize) -> NodeInfo {
    let ip = address.ip().to_string();

    NodeInfo {
        name: name.to_string(),
        cluster_address: address.to_string(),
        app_address: format!("{}:{}", ip, 9000 + offset),
        public_address: format!("{}:{}", ip, 10000 + offset),
    }
}

fn make_node_from_offset(name: &str, address: &str, offset: usize) -> NodeInfo {
    NodeInfo {
        name: name.to_string(),
        cluster_address: format!("{}:{}", address, 8000 + offset),
        app_address: format!("{}:{}", address, 9000 + offset),
        public_address: format!("{}:{}", address, 10000 + offset),
    }
}

// fn make_node_a(address: &str) -> NodeInfo {
//     let port = PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
//
//     NodeInfo {
//         name: "node_a".to_string(),
//         cluster_address: format!("{}:{}", address, port), //"127.0.0.1:8000".to_owned(),
//         app_address: "127.0.0.1:9000".to_owned(),
//         public_address: "127.0.0.1:8080".to_owned(),
//     }
// }

// fn make_node_b(address: SocketAddr) -> NodeInfo {
//     NodeInfo {
//         name: "node_b".to_string(),
//         cluster_address: address.to_string(), //"127.0.0.1:8001".to_owned(),
//         app_address: "127.0.0.1:9001".to_owned(),
//         public_address: "127.0.0.1:8081".to_owned(),
//     }
// }

// fn make_node_c(address: SocketAddr) -> NodeInfo {
//     NodeInfo {
//         name: "node_c".to_string(),
//         cluster_address: address.to_string(), //"127.0.0.1:8002".to_owned(),
//         app_address: "127.0.0.1:9002".to_owned(),
//         public_address: "127.0.0.1:8082".to_owned(),
//     }
// }

lazy_static! {
    static ref NAMES: Vec<&'static str> = vec![
        "node_a", "node_b", "node_c", "node_d", "node_e", "node_f", "node_g", "node_h", "node_i",
        "node_j", "node_k", "node_l",  "node_m", "node_n", "node_o", "node_p", "node_q", "node_r",
        "node_s", "node_t", "node_u",  "node_v", "node_w", "node_x", "node_y", "node_z",
    ];
}

fn make_all_nodes(offsets: &Vec<usize>) -> Vec<NodeInfo> {
    offsets.iter()
        .zip(NAMES.iter())
        .map(|(offset, name)| {
            make_node_from_offset(*name, LOCALHOST, *offset)
        })
        .collect()
    //todo maybe zip with names
    // vec![
    //     make_node_from_offset("node_a", LOCALHOST, *offsets.get(0).unwrap()),
    //     make_node_from_offset("node_b", LOCALHOST, *offsets.get(1).unwrap()),
    //     make_node_from_offset("node_c", LOCALHOST, *offsets.get(2).unwrap()),
    // ]
}

fn make_expected_nodes(offsets: &Vec<usize>) -> BTreeMap<NodeId, NodeInfo> {
    let expected = make_all_nodes(offsets);
    let mut expected_nodes = BTreeMap::new();
    for n in expected.into_iter() {
        expected_nodes.insert(n.node_id(), n);
    }

    // let expected_a = expected.get(0).unwrap();
    // let expected_b = expected.get(1).unwrap();
    // let expected_c = expected.get(2).unwrap();
    // let expected_a = make_node_w_offset("node_a", LOCALHOST, 0);
    // let expected_b = make_node_w_offset("node_b", LOCALHOST, 1);
    // let
    //     NodeInfo {
    //     name: "node_b".to_owned(),
    //     cluster_address: "127.0.0.1:8001".to_owned(),
    //     app_address: "127.0.0.1:9001".to_owned(),
    //     public_address: "127.0.0.1:8081".to_owned(),
    // };
    //
    // let expected_c = NodeInfo {
    //     name: "node_c".to_owned(),
    //     cluster_address: "127.0.0.1:8002".to_owned(),
    //     app_address: "127.0.0.1:9002".to_owned(),
    //     public_address: "127.0.0.1:8082".to_owned(),
    // };

    // let mut expected_nodes = BTreeMap::new();
    // expected_nodes.insert(expected_a.node_id(), expected_a.clone());
    // expected_nodes.insert(expected_b.node_id(), expected_b.clone());
    // expected_nodes.insert(expected_c.node_id(), expected_c.clone());
    expected_nodes
}

fn make_test_network(node_info: &NodeInfo) -> TestNetwork {
    let node_id = node_info.node_id();
    let ring = Ring::new(10);
    let discovery = "127.0.0.1:8888".parse::<SocketAddr>().unwrap();
    Network::new(node_id, node_info, ring, discovery)
}

#[tracing::instrument]
fn make_test_configuration<S>(host: S, host_offset: usize, nodes: Vec<&NodeInfo>) -> Configuration
where
    S: AsRef<str> + std::fmt::Debug,
{
    let mut c: Config = Config::default();
    c.set("discovery_host_address", format!("{}:{}", LOCALHOST, 8000 + host_offset)).unwrap();
    c.set("join_strategy", "static").unwrap();
    c.set("ring_replicas", 10).unwrap();
    c.set("max_discovery_timeout", 5).unwrap();
    c.set("max_raft_init_timeout", 5).unwrap();
    c.set("election_timeout_min", 200).unwrap();
    c.set("election_timeout_max", 300).unwrap();
    c.set("heartbeat_interval", 50).unwrap();
    c.set("max_payload_entries", 300).unwrap();
    c.set("metrics_rate", 10).unwrap();
    c.set("snapshot_dir", "data/snapshots/").unwrap();
    c.set("snapshot_max_chunk_size", 3_145_728).unwrap();

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
    info!("actual.id:{} expected.id:{}", actual.id, node_info.node_id() );
    assert_eq!(actual.id, node_info.node_id());
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
fn test_network_configure() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    let _ = span.enter();

    let offset_a = port_offset_get_and_increment();
    let offset_b = port_offset_get_and_increment();
    let offset_c = port_offset_get_and_increment();

    let node_info = make_node_from_offset("node_a", LOCALHOST, offset_a);
    let nodes = make_all_nodes(&vec![offset_a, offset_b, offset_c]);
    let nodes_ref = nodes.iter().by_ref().collect();
    let config = make_test_configuration(node_info.name.as_str(), offset_a, nodes_ref);

    let mut actual = make_test_network(&node_info);
    actual.configure_with(&config);

    info!("actual.id:{} expected.id:{}", actual.id, node_info.node_id() );
    assert_eq!(actual.id, node_info.node_id() );
    assert_eq!(actual.info, node_info);
    assert_eq!(actual.state.unwrap(), Status::Joining);
    assert_eq!(actual.state.extent(), Extent::SingleNode);
    assert_eq!(
        actual.discovery,
        "127.0.0.1:8888".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(actual.nodes.is_empty(), false);
    assert_eq!(actual.nodes.len(), 3);

    let expected_nodes = make_expected_nodes(&vec![offset_a, offset_b, offset_c]);

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

    let offset_a = port_offset_get_and_increment();
    let offset_b = port_offset_get_and_increment();
    let offset_c = port_offset_get_and_increment();

    let sys = System::builder().stop_on_panic(true).name("test").build();

    let node_info = make_node_from_offset("node_a", LOCALHOST, offset_a);
    let nodes = make_all_nodes(&vec![offset_a, offset_b, offset_c]);
    let nodes_ref = nodes.iter().by_ref().collect();
    let config = make_test_configuration(node_info.name.as_str(), offset_a, nodes_ref);
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
            assert_eq!(actual.id, utils::generate_node_id(nodes.get(0).unwrap().cluster_address.as_str()));

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
fn test_network_builder() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    let _ = span.enter();

    let offset_a = port_offset_get_and_increment();
    let offset_b = port_offset_get_and_increment();
    let offset_c = port_offset_get_and_increment();

    let sys = System::builder().stop_on_panic(true).name("test").build();

    let node_infos = make_all_nodes(&vec![offset_a, offset_b, offset_c]);
    let node_a = node_infos.get(0).unwrap().clone();
    let config = make_test_configuration("node_a", offset_a, node_infos.iter().by_ref().collect());
    let local_id = utils::generate_node_id(node_a.cluster_address.as_str());
    let seed_members = node_infos.iter()
        .map(|info| { info.node_id() })
        .collect();
    let storage_factory = MemoryStorageFactory::new()
        .with_members(seed_members)
        .with_configuration(&config)
        .collect();

    let system = RaftSystemBuilder::new(local_id)
        .with_configuration(&config)
        .with_storage_factory(storage_factory)
        .build()
        .unwrap();

    let node_info = make_node_from_offset("node_a", LOCALHOST, offset_a);

    let test = system.network.send(GetClusterSummary)
        .map_err(|err| {
            error!("error in get cluster summary: {:?}", err);
            panic!(err)
        })
        .and_then(move |res| {
            let actual = res.unwrap();
            info!("B: actual cluster summary:{:?}", actual);

            info!("actual.id: {:?}", actual.id);
            assert_eq!(actual.id, utils::generate_node_id(node_a.cluster_address.as_str()));

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
    spawn(test);

    assert!(sys.run().is_ok(), "error during test");

}


#[test]
fn test_network_bind() {
    fixtures::setup_logger();
    let span = span!( Level::INFO, "test_network_bind" );
    let _ = span.enter();

    let offset_a = port_offset_get_and_increment();
    let offset_b = port_offset_get_and_increment();
    // let offset_c = port_offset_get_and_increment();

    let sys = System::builder().stop_on_panic(true).name("test").build();

    let node_a = make_node_from_offset("node_a", LOCALHOST, offset_a);
    let node_a_2 = node_a.clone();
    let node_a_id = node_a.node_id();
    let node_b = make_node_from_address("node_b", server_address(), offset_b);
    // let node_infos = vec![node_a.clone(), node_b.clone()];
    let node_infos = vec![node_a.clone()];
    let seed_members: Vec<NodeId> = node_infos.iter().map(|info| info.node_id()).collect();
    // let expected_members: Vec<NodeId> = seed_members.clone();
    let config = make_test_configuration("node_a", offset_a, node_infos.iter().by_ref().collect());

    let node_b_id = node_b.node_id();
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
        .expect(0)
        .create();
    // let nb_mock = mock("POST", b_path.as_str()).expect(1).create();
    let raft_no_call_mock = mock("POST", "/api/cluster/admin").expect(0).create();

    let s = span!(Level::INFO, "build_network");
    let _ = s.enter();

    let storage_factory = MemoryStorageFactory::new()
        .with_members(seed_members)
        .with_configuration(&config)
        .collect();

    let _system = RaftSystemBuilder::new(node_a.node_id())
        .with_configuration(&config)
        .with_storage_factory(storage_factory)
        .build()
        .unwrap();

    // let network2 = system.network.clone();
    let route = format!("http://{}/{}", node_a_2.cluster_address, "api/cluster/nodes");

    let test =  Delay::new(Instant::now() + Duration::from_millis(50))
        .map_err(|err| { panic!(err) })
        .and_then(move |_| {
            let client = reqwest::Client::builder().build().unwrap();
            info!(%route, "GET all nodes in A");
            client.get(route.as_str()).send()
        })
        .map_err(|err| {
            error!(error = ?err, "error in GET all nodes send");
            panic!(err)
        })
        .and_then(move |mut resp| {
            info!(response = ?resp, "response from GET all nodes.");
            let actual = resp.json::<Vec::<NodeId>>().unwrap();
            assert_eq!(actual.len(), 1);
            assert_eq!(*actual.get(0).unwrap(), node_a_id);
            Ok(())
        })
        // .and_then(move |_| {
        //     network2.send(GetClusterSummary)
        //         .map_err(|err| {
        //             error!("error in get cluster summary: {:?}", err);
        //             panic!(err)
        //         })
        //         .and_then(move |res| {
        //             let s = span!(Level::INFO, "assert_results");
        //             let _ = s.enter();
        //
        //             let actual = res.unwrap();
        //             info!("B: actual cluster summary:{:?}", actual);
        //
        //             info!("actual.id: {:?}", actual.id);
        //             assert_eq!(actual.id, utils::generate_node_id("127.0.0.1:8000"));
        //
        //             info!("actual.info: {:?}", actual.info);
        //             assert_eq!(actual.info, node_a);
        //
        //             info!("actual.isolated_nodes: {:?}", actual.state.isolated_nodes());
        //             assert_eq!(actual.state.isolated_nodes().is_empty(), true);
        //
        //             // Still in Joining since move to WeaklyUp dependent on
        //             // RAFT cluster change msg from leader
        //             info!("actual.state: {:?}", actual.state);
        //             assert_eq!(actual.state.unwrap(), Status::Joining);
        //
        //             info!("actual.connected_nodes: {:?}", actual.state.connected_nodes());
        //             info!("actual.state.extent: {:?}", actual.state.extent());
        //             // assert_eq!(actual.state.extent, Extent::Initialized); //todo bad
        //             assert_eq!(actual.state.extent(), Extent::Cluster);
        //
        //             info!("[{:?}] actual.connected_nodes: {:?}", actual.id, actual.state.connected_nodes());
        //             // assert_eq!(actual.connected_nodes.len(), 0); //todo bad
        //             assert_eq!(actual.state.connected_nodes().len(), 2);
        //
        //             assert!(actual.metrics.is_some(), "raft metrics");
        //             let a_metrics = actual.metrics.unwrap();
        //             assert_eq!(a_metrics.id, actual.id);
        //             assert_eq!(a_metrics.last_log_index, 0);
        //             assert_eq!(a_metrics.current_term, 0);
        //             assert_eq!(a_metrics.state, RaftState::Follower);
        //             assert_eq!(a_metrics.current_leader, None);
        //             assert_eq!(a_metrics.last_applied, 0);
        //
        //             let e_membership = actix_raft::messages::MembershipConfig {
        //                 is_in_joint_consensus: false,
        //                 members: expected_members.clone(),
        //                 non_voters: Vec::new(),
        //                 removing: Vec::new(),
        //             };
        //             assert_eq!(a_metrics.membership_config, e_membership);
        //
        //             Ok(())
        //         })
                .then(|res| {
                    let s = span!(Level::INFO, "clean_up");
                    let _ = s.enter();

                    info!("test finished -- wrapping up");
                    actix::System::current().stop();
                    res
                })
        // });
    ;

    let s = span!(Level::INFO, "assert_mock");
    let _ = s.enter();
    info!("ASSERTING MOCK service: {:?}", node_connect_mock);
    // node_connect_mock.assert();
    raft_no_call_mock.assert();

    info!("#### BEFORE BLOCK...");
    spawn(test);
    assert!(sys.run().is_ok(), "error during test");
}

struct ConnectNodePrep<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    pub system: RaftSystem<D, R, E, S>,
    pub network_id: NodeId,
    pub members: HashMap<NodeId, NodeInfo>,
}

impl<D, R, E, S> ConnectNodePrep<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    pub fn network(&self) -> Addr<Network<D, R, E, S>> { self.system.network.clone() }
}

impl<D, R, E, S> std::clone::Clone for ConnectNodePrep<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
{
    fn clone(&self) -> Self {
        Self {
            system: self.system.clone(),
            network_id: self.network_id,
            members: self.members.clone(),
        }
    }
}

#[tracing::instrument]
fn create_and_bind_network(
    host_id: NodeId,
    members: HashMap<NodeId, NodeInfo>,
    config: &Configuration
) ->
    (
        ConnectNodePrep<Data, Response, Error, Storage>,
        impl Future<Item = ClusterSummary, Error = NetworkError>
    )
{
    let member_ids = members.keys().map(|k| *k).collect();
    let storage_factory = MemoryStorageFactory::new()
        .with_members(member_ids)
        .with_configuration(config)
        .collect();

    let system = RaftSystemBuilder::new(host_id)
        .with_configuration(config)
        .with_storage_factory(storage_factory)
        .build()
        .unwrap();

    debug!("getting summary...");
    let network = system.network.clone();

    let expected_host_info = members.get(&host_id).unwrap().clone();

    let task = Delay::new(Instant::now() + Duration::from_millis(50))
        .map_err(|err| {panic!(err)})
        .and_then(move |_| {
            network.send(GetClusterSummary)
                .from_err()
                .and_then(move |summary| {
                    info!("baseline summary:{:?}", summary);
                    let actual = summary.unwrap();
                    assert_eq!(actual.id, host_id);
                    assert_eq!(actual.info, expected_host_info);
                    assert_eq!(actual.state.isolated_nodes().is_empty(), true);
                    assert_eq!(actual.state.unwrap(), Status::Joining);
                    assert_eq!(actual.state.extent(), Extent::SingleNode);
                    assert_eq!(actual.state.connected_nodes().len(), 1);
                    debug!("actual.metrics = {:?}", actual.metrics);
                    assert!(actual.metrics.is_some(), "raft metrics");
                    let a_metrics = actual.metrics.as_ref().unwrap();
                    assert_eq!(a_metrics.id, actual.id);
                    assert_eq!(a_metrics.last_log_index, 0);
                    assert_eq!(a_metrics.current_term, 0);
                    assert_eq!(a_metrics.state, RaftState::NonVoter);
                    assert_eq!(a_metrics.current_leader, None);
                    assert_eq!(a_metrics.last_applied, 0);
                    debug!("wrapping it up...");
                    futures::future::ok::<ClusterSummary, NetworkError>(actual)
                })
        });

    let prep = ConnectNodePrep { system, network_id: host_id, members, };
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
fn test_network_cmd_connect_node_no_leader() {
    fixtures::setup_logger();
    let span = span!(Level::DEBUG, "test_cmd_connect_node_no_leader");
    let _ = span.enter();

    let offset_a = port_offset_get_and_increment();
    let offset_b = port_offset_get_and_increment();
    // let offset_c = port_offset_get_and_increment();

    let sys = System::builder().stop_on_panic(true).name("test-handle-connect").build();

    let node_a = make_node_from_offset("node_a", "[::1]", offset_a);
    // let node_a_1 = node_a.clone();
    let node_b = make_node_from_address("node_b", server_address(), offset_b);
    let config = make_test_configuration("node_a", offset_a, vec![&node_a, &node_b]);

    let node_a_id = node_a.node_id();
    let node_b_id = node_b.node_id();
    let mut members = HashMap::new();
    members.insert(node_a_id, node_a.clone());
    members.insert(node_b_id, node_b.clone());

    let b_path_exp = format!("/api/cluster/nodes/{}", node_b_id);
    info!("mock b path = {}", b_path_exp);

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

    let (prep, prep_task) = create_and_bind_network(node_a_id, members, &config);
    // let n2 = prep.network.clone();

    // let (node_b_id, node_b_info) = prep.members.iter().find(|id_info| {
    //     (*id_info).0 != &prep.network_id
    // })
    //     .map(|id_info|  (*id_info.0, id_info.1.clone()))
    //     .unwrap();

    let network_1 = prep.network();
    let network_2 = prep.network();
    let node_b_1 = node_b.clone();
    let task = prep_task
        .and_then(move |_summary| {
        debug!("ConnectNode#{} but there is no leader...", node_b_id);
        network_1.send(ConnectNode{id: node_b_id, info: node_b_1}).from_err()
    })
        .and_then(move |_ack| {
            network_2.send(GetClusterSummary).from_err()
        })
        // .from_err()
        .and_then(move |summary| {
            info!("AFTER Connect summary:{:?}", summary);
            // let foo = *prep.members.get(&prep.network_id).unwrap();
            let actual = summary.unwrap();
            assert_eq!(actual.id, utils::generate_node_id(node_a.cluster_address.as_str()));
            assert_eq!(&actual.info, prep.members.get(&prep.network_id).unwrap());
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);
            assert_eq!(actual.state.unwrap(), Status::Joining);
            assert_eq!(actual.state.extent(), Extent::Cluster);
            assert_eq!(actual.state.connected_nodes().len(), 2);
            assert!(actual.metrics.is_some(), "raft metrics");
            let a_metrics = actual.metrics.as_ref().unwrap();
            assert_eq!(a_metrics.id, actual.id);
            assert_eq!(a_metrics.last_log_index, 0);
            assert_eq!(a_metrics.current_term, 0);
            assert_eq!(a_metrics.state, RaftState::Follower);
            assert_eq!(a_metrics.current_leader, None);
            assert_eq!(a_metrics.last_applied, 0);
            debug!("wrapping it up...");
            futures::future::ok::<ClusterSummary, NetworkError>(actual)
        })
        .and_then(move |res| {
            nb_mock.assert();
            raft_mock.assert();
            futures::future::ok(res)
        })
        .then(|_| {
            info!("************* Testing Completed ***********");
            actix::System::current().stop();
            Ok(())
        });

    spawn(task);
    assert!(sys.run().is_ok(), "error during test");
}

#[test]
fn test_network_cmd_connect_node_change_info() {
    fixtures::setup_logger();
    let span = span!(Level::DEBUG, "test_cmd_connect_node_change_info");
    let _ = span.enter();

    let offset_a = port_offset_get_and_increment();
    let offset_b = port_offset_get_and_increment();
    // let offset_c = port_offset_get_and_increment();

    let sys = System::builder().stop_on_panic(true).name("test-handle-connect").build();

    let node_a = make_node_from_offset("node_a", "[::1]", offset_a);
    // let node_a_1 = node_a.clone();
    let node_b = make_node_from_address("node_b", server_address(), offset_b);
    let config = make_test_configuration("node_a", offset_a, vec![&node_a, &node_b]);

    let node_a_id = node_a.node_id();
    let node_b_id = node_b.node_id();
    let mut members = HashMap::new();
    members.insert(node_a_id, node_a.clone());
    members.insert(node_b_id, node_b.clone());

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

    let (prep, prep_task) = create_and_bind_network(node_a_id, members, &config);
    // let n2 = prep.network.clone();

    // let (node_b_id, node_b_info) = prep.members.iter().find(|id_info| {
    //     (*id_info).0 != &prep.network_id
    // })
    //     .map(|id_info|  (*id_info.0, id_info.1.clone()))
    //     .unwrap();

    let network_1 = prep.network();
    let network_2 = prep.network();
    let network_3 = prep.network();

    let mut changed_node_b = node_b.clone();
    changed_node_b.name = "new-node-b".to_string();
    let changed_node_b_expected = changed_node_b.clone();

    let task = prep_task.and_then(move |_summary| {
        // change node_b info on connect
        debug!("ConnectNode#{} will change node_b info, but there is no leader...", node_b_id);
        network_1.send(ConnectNode{id: node_b_id, info: changed_node_b}).from_err()
    })
        .and_then(move |_ack| {
            network_2.send(GetClusterSummary).from_err()
        })
        .and_then(move |summary| {
            info!("AFTER Connect summary:{:?}", summary);
            // let foo = *prep.members.get(&prep.network_id).unwrap();
            let actual = summary.unwrap();
            assert_eq!(actual.id, utils::generate_node_id(node_a.cluster_address.as_str()));
            assert_eq!(&actual.info, prep.members.get(&prep.network_id).unwrap());
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);
            assert_eq!(actual.state.unwrap(), Status::Joining);
            assert_eq!(actual.state.extent(), Extent::Cluster);
            assert_eq!(actual.state.connected_nodes().len(), 2);
            assert!(actual.metrics.is_some(), "raft metrics");
            let a_metrics = actual.metrics.as_ref().unwrap();
            assert_eq!(a_metrics.id, actual.id);
            assert_eq!(a_metrics.last_log_index, 0);
            assert_eq!(a_metrics.current_term, 0);
            assert_eq!(a_metrics.state, RaftState::Follower);
            assert_eq!(a_metrics.current_leader, None);
            assert_eq!(a_metrics.last_applied, 0);
            debug!("wrapping it up with node change verify...");

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
            info!("************* Testing Completed ***********");
            actix::System::current().stop();
            Ok(())
        });

    spawn(task);
    assert!(sys.run().is_ok(), "error during test");
}


#[test]
fn test_network_cmd_connect_node_leader() {
    fixtures::setup_logger();
    let span = span!(Level::DEBUG, "test_cmd_connect_node_leader");
    let _ = span.enter();

    let offset_a = port_offset_get_and_increment();
    let offset_b = port_offset_get_and_increment();
    // let offset_c = port_offset_get_and_increment();

    let sys = System::builder().stop_on_panic(true).name("test-handle-connect").build();

    let node_a = make_node_from_offset("node_a", "[::1]", offset_a);
    // let node_a_1 = node_a.clone();
    let node_b = make_node_from_address("node_b", server_address(), offset_b);
    // let config = make_test_configuration("node_a", vec![&node_a, &node_b]);
    let config = make_test_configuration("node_a", offset_a, vec![&node_a]);

    let node_a_id = node_a.node_id();
    let node_b_id = node_b.node_id();
    let mut members = HashMap::new();
    members.insert(node_a_id, node_a.clone());
    // members.insert(node_b_id, node_b.clone());

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

    let connect_to_b_mock = mock("POST", Matcher::Regex(b_path_exp))
        .with_header("content-type", "application/json")
        .with_body(b_connect_ack_json)
        .expect(1)
        .create();

    let b_vote_resp = entities::RaftVoteResponse {
        term: 1,
        vote_granted: true,
        is_candidate_unknown: false,
    };
    let b_vote_resp_json = serde_json::to_string(&b_vote_resp).unwrap();
    let raft_vote_req_mock = mock("POST", "/api/cluster/vote")
        .with_header("content-type", "application/json")
        .with_body(b_vote_resp_json)
        .expect(1)
        .create();

    let b_append_entries_resp = entities::RaftAppendEntriesResponse {
        term: 1,
        success: true,
        conflict: None,
    };
    let b_append_entries_resp_json = serde_json::to_string(&b_append_entries_resp).unwrap();
    let raft_append_entries_mock = mock("POST", "/api/cluster/entries")
        .with_header("content-type", "application/json")
        .with_body(b_append_entries_resp_json)
        .expect_at_least(1)
        .create();

    let raft_gen_exclude_mock = mock("POST", "/api/cluster/admin").expect(0).create();

    let (prep, prep_task) = create_and_bind_network(node_a_id, members, &config);

    // let network_1 = prep.network();
    let network_2 = prep.network();
    let network_3 = prep.network();
    let prep_1 = prep.clone();
    let node_b_1 = node_b.clone();

    let task = prep_task.and_then(move |_summary| {
        // let members = prep_1.members.keys().map(|k| *k).collect();
        info!("Starting RaftSystem...");
        let system_arb = Arbiter::new();
        let _ = RaftSystem::start_in_arbiter(&system_arb, |_| prep_1.system);
        futures::future::ok(())
    })
        .and_then(move |_| {
            debug!("ConnectNode#{} will change node_b info, and node_a is LEADER!!!...", node_b_id);
            network_2.send(ConnectNode{id: node_b_id, info: node_b_1}).from_err()
        })
        .and_then(|ack| {
            Delay::new(Instant::now() + Duration::from_millis(250))
                .map_err(move |err| panic!("Delay failed: {:?} after ack", err))
                .and_then(move |_| ack)
        })
        .and_then(move |_ack| {
            network_3.send(GetClusterSummary).from_err()
        })
        .and_then(move |summary| {
            info!("AFTER Connect summary:{:?}", summary);
            let actual = summary.unwrap();
            assert_eq!(actual.id, node_a_id);
            assert_eq!(actual.info, node_a);
            assert_eq!(actual.state.isolated_nodes().is_empty(), true);
            assert_eq!(actual.state.extent(), Extent::Cluster);
            assert_eq!(actual.state.connected_nodes().len(), 2);
            assert_eq!(actual.state.unwrap(), Status::Up);
            assert!(actual.metrics.is_some(), "raft metrics");
            let a_metrics = actual.metrics.as_ref().unwrap();
            assert_eq!(a_metrics.id, actual.id);
            assert_eq!(a_metrics.last_log_index, 1);
            assert_eq!(a_metrics.current_term, 1);
            assert_eq!(a_metrics.state, RaftState::Leader);

            let mut actual_members = HashSet::new();
            for a in a_metrics.membership_config.members.iter() {
                actual_members.insert(*a);
            }

            let mut expected_members = HashSet::new();
            expected_members.insert(node_a_id);
            expected_members.insert(node_b_id);
            assert_eq!(actual_members, expected_members);
            assert!(a_metrics.membership_config.non_voters.is_empty());
            assert!(a_metrics.membership_config.removing.is_empty());
            assert_eq!(a_metrics.current_leader, Some(node_a_id));
            assert_eq!(a_metrics.last_applied, 1);

            futures::future::ok::<ClusterSummary, NetworkError>(actual)
        })
        .and_then(move |res| {
            connect_to_b_mock.assert();
            raft_vote_req_mock.assert();
            raft_append_entries_mock.assert();
            raft_gen_exclude_mock.assert();
            futures::future::ok(res)
        })
        .then(|_| {
            info!("************* Testing Completed ***********");
            actix::System::current().stop();
            Ok(())
        });

    spawn(task);
    assert!(sys.run().is_ok(), "error during test");
}