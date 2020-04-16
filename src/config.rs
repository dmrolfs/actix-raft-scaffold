use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use serde::Deserialize;
use structopt::StructOpt;
use thiserror::Error;
use tracing::*;
use ::config::Config;
use crate::{NodeInfo, NodeList};

#[derive(Error, Debug)]
pub enum ConfigurationError {
    #[error("host, {0}, is missing from nodes list")]
    HostMissing(String),

    #[error("application misconfigured")]
    ConfigSource(#[from] config::ConfigError),

    #[error("unknown configuration error")]
    Unknown,
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
pub struct ConfigSchema {
    pub discovery_host_address: String,
    pub join_strategy: JoinStrategy,
    pub ring_replicas: isize,
    pub nodes: NodeList
}

#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    pub host: String,
    pub discovery_host: SocketAddr,
    pub join_strategy: JoinStrategy,
    pub ring_replicas: isize,
    pub nodes: HashMap<String, NodeInfo>,
}

impl Configuration {
    pub fn host_info(&self) -> NodeInfo {
        self.nodes
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

        let nodes = schema.nodes.iter()
            .map(|n| (n.name.clone(), n.clone()))
            .into_iter()
            .collect();

        Configuration {
            host: "".to_owned(),
            discovery_host: discovery,
            join_strategy: schema.join_strategy,
            ring_replicas: schema.ring_replicas,
            nodes,
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
            .map_err(|err| err.into())
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