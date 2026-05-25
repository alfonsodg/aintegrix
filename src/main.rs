use clap::Parser;

mod agent;
mod config;
mod error;
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

    match config::loader::load(&cli.config) {
        Ok(cfg) => {
            tracing::info!(
                agents = cfg.agents.len(),
                host = %cfg.server.host,
                port = cfg.server.port,
                "configuration loaded"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to load configuration");
            std::process::exit(1);
        }
    }
}
