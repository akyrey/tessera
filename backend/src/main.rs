use std::net::SocketAddr;
use std::process::ExitCode;

use clap::Parser;
use tessera::accounts::service;
use tessera::cli::{Cli, Command, run_admin};
use tessera::clock::now_unix;
use tessera::config::Config;
use tessera::{AppState, brands, db, janitor, router};
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing();
    match run(cli.command.unwrap_or(Command::Serve)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "tessera stopped");
            ExitCode::FAILURE
        }
    }
}

async fn run(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let pool = db::connect(&config.database_url).await?;
    match command {
        Command::Serve => serve(config, pool).await,
        Command::Admin(action) => {
            let result = run_admin(action, &config, &pool, &mut std::io::stdout()).await;
            pool.close().await;
            Ok(result?)
        }
    }
}

async fn serve(config: Config, pool: sqlx::SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = config
        .bootstrap_token
        .as_ref()
        .map(|t| t.expose().to_owned());
    if let Some(invite) =
        service::ensure_bootstrap_invite(&pool, bootstrap.as_deref(), now_unix()).await?
    {
        if bootstrap.is_some() {
            tracing::warn!(
                "no users yet: open /join#<TESSERA_BOOTSTRAP_TOKEN> to create the first admin"
            );
        } else {
            // Single use, valid 24h, and only while no account exists.
            tracing::warn!(
                link = %format!("{}/join#{}", config.public_origin(), invite.token.value),
                "no users yet: open this link to create the first admin"
            );
        }
    }

    let listener = TcpListener::bind(config.listen_addr).await?;
    tracing::info!(
        addr = %config.listen_addr,
        public_url = %config.public_url,
        rp_id = %config.webauthn_rp_id,
        "tessera listening"
    );

    let state = AppState::new(config, pool.clone())?;
    let cleanup = janitor::spawn(state.clone());
    let store_refresh = brands::refresh::spawn(state.clone());
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    cleanup.abort();
    store_refresh.abort();
    pool.close().await;
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
