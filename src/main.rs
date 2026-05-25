use std::sync::Arc;

use clap::Parser;

mod agent;
mod config;
mod error;
mod persistence;
mod protocol;
mod server;

#[derive(Parser)]
#[command(name = "aintegrix", version, about = "Centralized ACP server for multi-agent coordination")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/aintegrix/aintegrix.yaml")]
    config: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("aintegrix=info").json().init();

    let cli = Cli::parse();
    tracing::info!(config_path = %cli.config, "starting aintegrix");

    let cfg = match config::loader::load(&cli.config) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!(error = %e, "failed to load configuration");
            std::process::exit(1);
        }
    };

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    tracing::info!(agents = cfg.agents.len(), %addr, "configuration loaded");

    let state = Arc::new(server::routes::AppState { config: cfg });
    let app = server::routes::create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, %addr, "failed to bind");
        std::process::exit(1);
    });

    tracing::info!(%addr, "server listening");
    axum::serve(listener, app).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "server error");
        std::process::exit(1);
    });
}
