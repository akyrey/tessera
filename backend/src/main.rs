use std::process::ExitCode;
use std::sync::Arc;

use tessera::config::Config;
use tessera::{AppState, db, router};
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    init_tracing();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "tessera stopped");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let db = db::connect(&config.database_url).await?;
    let listener = TcpListener::bind(config.listen_addr).await?;

    tracing::info!(
        addr = %config.listen_addr,
        public_url = %config.public_url,
        rp_id = %config.webauthn_rp_id,
        "tessera listening"
    );

    let state = AppState {
        db: db.clone(),
        config: Arc::new(config),
    };
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    db.close().await;
    Ok(())
}

/// JSON logs in release builds (for the cluster), readable logs in development.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    if cfg!(debug_assertions) {
        builder.init();
    } else {
        builder.json().init();
    }
}

/// Kubernetes sends SIGTERM before killing the pod; finish in-flight requests first.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::error!(%error, "cannot listen for ctrl-c");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(error) => tracing::error!(%error, "cannot listen for SIGTERM"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
