use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use structopt::StructOpt;
use thiserror::Error;
use tracing::*;
use config::{Config, ConfigError};
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

// impl From<config::error::ConfigError> for ConfigurationError {
//     fn from(err: ConfigError) -> Self {
//         unimplemented!()
//     }
// }

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
    pub discovery_host: String,
    pub join_strategy: JoinStrategy,
    pub nodes: NodeList
}

#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    host: String,
    discovery_host: SocketAddr,
    join_strategy: JoinStrategy,
    nodes: HashMap<String, NodeInfo>,
}

impl Configuration {
    #[tracing::instrument]
    pub fn load() -> Result<Configuration, ConfigurationError> {
        let opt: Opt = Opt::from_args();
        info!("CLI options {:?}", opt);

        let ref_path = std::path::Path::new("config/reference.toml");
        let config_path = opt.configuration_path
            .as_ref()
            .map(|p| p.as_path());

        let mut config = Config::default();

        config_path.map(|p| {
            config.merge(config::File::from(p))
                .expect("could not find application configuration file")
        });

        config.merge(config::File::from(ref_path))
            .expect( "could not find config/reference.toml");

        config.merge( config::Environment::with_prefix("APP")).unwrap();

        config
            .try_into::<ConfigSchema>()
            .map_err(|err| err.into())
            .map(|schema| {
                info!("Actuator configuration: {:?}", schema);

                let discovery = schema.discovery_host.parse::<SocketAddr>()
                    .expect("failed to parse discovery host socket address");

                let nodes = schema.nodes.iter()
                    .map(|n| (n.name.clone(), n.clone()))
                    .into_iter()
                    .collect();

                Configuration {
                    host: opt.host.to_owned(),
                    discovery_host: discovery,
                    join_strategy: schema.join_strategy,
                    nodes,
                }
            })
    }
}