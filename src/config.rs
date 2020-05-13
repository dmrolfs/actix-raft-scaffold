use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use chrono::Duration;
use serde::Deserialize;
use structopt::StructOpt;
use thiserror::Error;
use tracing::*;
use ::config::Config;
use crate::{NodeInfo, NodeList};
use actix_raft::SnapshotPolicy;


#[derive(Error, Debug)]
pub enum ConfigurationError {
    #[error("host, {0}, is missing from nodes list")]
    HostMissing(String),

    #[error("application misconfigured")]
    ConfigSource(#[from] config::ConfigError),

    #[error("Configuration was net set before building: {0:?}")]
    NoConfiguration(std::option::NoneError),

    #[error("unknown configuration error: {0}")]
    Unknown(String),
}

impl From<std::option::NoneError> for ConfigurationError {
    fn from(that: std::option::NoneError) -> Self { Self::NoConfiguration(that) }
}


#[derive(StructOpt, Debug)]
#[structopt(name="actix-raft-seed")]
pub struct Opt {
    /// node ref in configuration for this node
    #[structopt(short, long)]
    pub host: String,

    /// application configuration
    #[structopt(long = "config", parse(from_os_str))]
    pub configuration_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum JoinStrategy {
    Static,
    Dynamic,
}

#[derive(Deserialize, Debug, Clone)]
struct ConfigSchema {
    pub discovery_host_address: String,
    pub join_strategy: JoinStrategy,
    pub ring_replicas: isize,
    pub nodes: NodeList,
    pub max_discovery_timeout: i64,
    pub max_raft_init_timeout: i64,
    pub election_timeout_min: i64,
    pub election_timeout_max: i64,
    pub heartbeat_interval: i64,
    pub max_payload_entries: usize,
    pub metrics_rate: i64,
    pub snapshot_dir: PathBuf,
    pub snapshot_max_chunk_size: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    pub host: String,
    pub discovery_host: SocketAddr,
    pub join_strategy: JoinStrategy,
    pub ring_replicas: isize,
    pub seed_nodes: HashMap<String, NodeInfo>,
    pub max_discovery_timeout: Duration,
    pub max_raft_init_timeout: Duration,
    pub election_timeout_min: Duration,
    pub election_timeout_max: Duration,
    pub heartbeat_interval: Duration,
    pub max_payload_entries: usize,
    pub metrics_rate: Duration,
    pub snapshot_policy: SnapshotPolicy,
    pub snapshot_dir: PathBuf,
    pub snapshot_max_chunk_size: u64,
}

impl Configuration {
    pub fn host_info(&self) -> NodeInfo {
        self.seed_nodes
            .get(&self.host)
            .expect("host node not configured")
            .clone()
    }
}

impl From<ConfigSchema> for Configuration {
    #[tracing::instrument]
    fn from(schema: ConfigSchema) -> Self {
        info!("Actuator configuration: {:?}", schema);

        let discovery = schema.discovery_host_address.parse::<SocketAddr>()
            .expect("failed to parse discovery host socket address");

        let seed_nodes = schema.nodes.iter()
            .map(|n| (n.name.clone(), n.clone()))
            .into_iter()
            .collect();

        Configuration {
            host: "".to_owned(),
            discovery_host: discovery,
            join_strategy: schema.join_strategy,
            ring_replicas: schema.ring_replicas,
            seed_nodes,
            max_discovery_timeout: Duration::seconds(schema.max_raft_init_timeout),   //   Duration::from_std(schema.max_discovery_timeout).unwrap(),
            max_raft_init_timeout: Duration::seconds(schema.max_raft_init_timeout),
            election_timeout_min: Duration::milliseconds(schema.election_timeout_min),
            election_timeout_max: Duration::milliseconds(schema.election_timeout_max),
            heartbeat_interval: Duration::milliseconds(schema.heartbeat_interval),
            max_payload_entries: schema.max_payload_entries,
            metrics_rate: Duration::milliseconds(schema.metrics_rate),
            snapshot_policy: Default::default(),
            snapshot_dir: schema.snapshot_dir,
            snapshot_max_chunk_size: schema.snapshot_max_chunk_size,
        }
    }
}

impl Configuration {
    #[tracing::instrument]
    pub fn load_from_config<S>(host: S, config: Config) -> Result<Configuration, ConfigurationError>
    where
        S: AsRef<str> + std::fmt::Debug,
    {
        config
            .try_into::<ConfigSchema>()
            .map_err(|err| {
                error!(error = ?err, "error parsing config into schema.");
                err.into()
            })
            .map(|schema| {
                Configuration {
                    host: host.as_ref().to_owned(),
                    ..schema.into()
                }
            })
    }

    #[tracing::instrument]
    pub fn load_from_opt(opt: Opt, reference: &Path) -> Result<Configuration, ConfigurationError> {
        let config_path = opt.configuration_path
            .as_ref()
            .map(|p| p.as_path());

        let mut config = Config::default();

        config_path.map(|p| {
            config.merge(config::File::from(p))
                .expect("could not find application configuration file")
        });

        config.merge(config::File::from(reference))
            .expect( "could not find config/reference.toml");

        config.merge( config::Environment::with_prefix("APP")).unwrap();
        Self::load_from_config(opt.host, config)
    }

    #[tracing::instrument]
    pub fn load() -> Result<Configuration, ConfigurationError> {
        let opt: Opt = Opt::from_args();
        info!("CLI options {:?}", opt);
        let ref_path = std::path::Path::new("config/reference.toml");
        Self::load_from_opt(opt, ref_path)
    }
}

impl Into<::actix_raft::Config> for Configuration {
    fn into(self) -> ::actix_raft::Config {
        //todo: provide better feedback to snapshot_dir; e.g., does not exist
        debug!("config snapshot_dir={:?}", self.snapshot_dir);
        debug!("config election_timeout_min={:?}", self.election_timeout_min);
        debug!("config election_timeout_min millis ={:?}", self.election_timeout_min.num_milliseconds());
        debug!("config election_timeout_min millis as u16={:?}", self.election_timeout_min.num_milliseconds() as u16);
        debug!("config election_timeout_max={:?}", self.election_timeout_max);
        debug!("config election_timeout_max millis ={:?}", self.election_timeout_max.num_milliseconds());
        debug!("config election_timeout_max millis as u16={:?}", self.election_timeout_max.num_milliseconds() as u16);

        ::actix_raft::Config::build(self.snapshot_dir.to_string_lossy().to_string())
            .election_timeout_min(self.election_timeout_min.num_milliseconds() as u16)
            .election_timeout_max(self.election_timeout_max.num_milliseconds() as u16)
            .heartbeat_interval(self.heartbeat_interval.num_milliseconds() as u16)
            .metrics_rate(
                self.metrics_rate.to_std().expect("Duration parsing")
            )
            .snapshot_policy(self.snapshot_policy)
            .snapshot_max_chunk_size(self.snapshot_max_chunk_size)
            .validate()
            .expect("Raft config to be create without error.")
    }
}