//! `dobukasu` binary: compose the registry, start the HTTP server.

use std::net::SocketAddr;
use std::time::Duration;

use dobukasu_core::api::{AppState, router};
use dobukasu_core::config::Config;
use dobukasu_core::registry::Registry;
use tracing_subscriber::EnvFilter;

// Every bundled engine plug-in is linked in here; the core crate does not know
// any of them.
use dobukasu_engine_bing as _;
use dobukasu_engine_duckduckgo as _;
use dobukasu_engine_google as _;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("dobukasu=info")),
        )
        .init();

    let config = Config::load(None)?;
    let registry = Registry::from_plugins(&config)?;
    let engines = registry.ids().to_vec();

    let http = reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()?;

    let addr: SocketAddr = config.server.bind.parse()?;
    let app = router(AppState::new(registry, config, http));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, ?engines, "dobukasu listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "cannot listen for ctrl-c");
    }
}
