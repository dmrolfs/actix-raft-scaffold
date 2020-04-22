use tracing_subscriber::fmt;

pub fn setup_logger() {
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap_or(());
}
