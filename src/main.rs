mod assets;
mod auth;
mod config;
mod geo;
mod limits;
mod listener;
mod proxy;
mod routes;
mod transfer;

use std::{error::Error, net::SocketAddr, process::ExitCode};

use clap::Parser;
use tokio::net::TcpListener;

use crate::{
    config::Config,
    listener::{BoundedListener, PeerAddress},
};

#[tokio::main]
async fn main() -> ExitCode {
    let config = Config::parse();
    tracing_subscriber::fmt()
        .with_max_level(config.log_level)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    match serve(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rate: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve(config: Config) -> Result<(), Box<dyn Error>> {
    let app = routes::build(&config)?;
    let listener = TcpListener::bind(SocketAddr::new(config.host, config.port)).await?;
    tracing::info!(address = %listener.local_addr()?, base_path = %config.base_path, "Listening");
    axum::serve(
        BoundedListener::new(listener),
        app.into_make_service_with_connect_info::<PeerAddress>(),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        // Docker sends SIGTERM, while interactive shells use Ctrl+C.
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }

    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(%error, "Unable to listen for Ctrl+C");
        std::future::pending::<()>().await;
    }
}
