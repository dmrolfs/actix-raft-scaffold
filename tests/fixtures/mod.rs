use tracing_subscriber::fmt;

pub mod dev;
pub mod memory_storage;

pub fn setup_logger() {
    let _ = env_logger::try_init();

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap_or(());
}
