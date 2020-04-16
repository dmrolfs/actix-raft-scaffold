use tracing::*;
use tracing_subscriber::fmt;
use anyhow::{Result, Context};
// pub mod controller;

pub fn setup_logger() {
    // env_logger::init();
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        // .with_env_filter( "wip=trace" )
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("setting default subscriber failed")
        .unwrap();

    // let logger = env_logger::Builder::from_default_env().build();
    // async_log::Logger::wrap(logger, || 12)
    //     .start(log::LevelFilter::Trace)
    //     .expect("Expected to be able to start async logger.");
}
