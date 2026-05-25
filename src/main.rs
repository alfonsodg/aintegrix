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
}
