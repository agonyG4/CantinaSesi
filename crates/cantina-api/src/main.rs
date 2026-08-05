//! Process startup: loads configuration, composes the application and
//! serves it with graceful shutdown.

use anyhow::Context;
use cantina_api::{app, state::AppState};
use cantina_infrastructure::config::Config;
use cantina_infrastructure::persistence::InMemoryRepositories;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env();
    let repositories = InMemoryRepositories::with_seed_data();
    let state = AppState::new(repositories);
    let router = app(state, &config);

    let address = config.bind_address().to_string();
    let listener = TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind to {address}"))?;
    tracing::info!(%address, "cantina-api listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut stream) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            stream.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
