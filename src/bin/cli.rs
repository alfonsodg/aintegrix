use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aintegrix-cli", version, about = "CLI client for AIntegriX server")]
struct Cli {
    /// AIntegriX server URL
    #[arg(long, env = "AINTEGRIX_URL", default_value = "http://localhost:8050")]
    url: String,

    /// API token
    #[arg(long, env = "AINTEGRIX_API_KEY")]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show server status
    Status,
    /// List registered agents
    Agents,
    /// Session management
    Sessions {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Send a prompt to an agent
    Prompt {
        /// Agent name
        agent: String,
        /// Prompt message
        message: String,
    },
    /// Validate configuration file
    Config {
        /// Path to config file
        #[arg(default_value = "aintegrix.yaml")]
        path: String,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List active sessions
    List,
    /// Create a new session
    New {
        /// Agent name
        agent: String,
    },
    /// Close a session
    Close {
        /// Session ID
        id: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = reqwest::Client::new();

    let auth_header = cli.token.as_deref().map(|t| format!("Bearer {t}"));

    match cli.command {
        Commands::Status => {
            let resp = client.get(format!("{}/health", cli.url)).send().await;
            match resp {
                Ok(r) => println!("Status: {} ({})", r.status(), cli.url),
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        Commands::Agents => {
            let mut req = client.get(format!("{}/api/v1/agents", cli.url));
            if let Some(ref auth) = auth_header {
                req = req.header("authorization", auth);
            }
            match req.send().await {
                Ok(r) => {
                    let body = r.text().await.unwrap_or_default();
                    println!("{body}");
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        Commands::Sessions { action } => match action {
            SessionAction::List => {
                let mut req = client.get(format!("{}/api/v1/sessions", cli.url));
                if let Some(ref auth) = auth_header {
                    req = req.header("authorization", auth);
                }
                match req.send().await {
                    Ok(r) => println!("{}", r.text().await.unwrap_or_default()),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            SessionAction::New { agent } => {
                let mut req = client
                    .post(format!("{}/api/v1/sessions", cli.url))
                    .json(&serde_json::json!({"agent": agent}));
                if let Some(ref auth) = auth_header {
                    req = req.header("authorization", auth);
                }
                match req.send().await {
                    Ok(r) => println!("{}", r.text().await.unwrap_or_default()),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            SessionAction::Close { id } => {
                let mut req = client.delete(format!("{}/api/v1/sessions/{id}", cli.url));
                if let Some(ref auth) = auth_header {
                    req = req.header("authorization", auth);
                }
                match req.send().await {
                    Ok(r) => println!("Status: {}", r.status()),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
        },
        Commands::Prompt { agent, message } => {
            // Create session, send prompt
            let mut req = client
                .post(format!("{}/api/v1/sessions", cli.url))
                .json(&serde_json::json!({"agent": agent}));
            if let Some(ref auth) = auth_header {
                req = req.header("authorization", auth);
            }
            match req.send().await {
                Ok(r) => {
                    let body: serde_json::Value =
                        r.json().await.unwrap_or(serde_json::json!({}));
                    let session_id = body["id"].as_str().unwrap_or("unknown");
                    println!("Session: {session_id}");
                    println!("Prompt: {message}");
                    println!("(streaming not yet implemented in CLI)");
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        Commands::Config { path } => {
            match aintegrix::config::loader::load(&path) {
                Ok(cfg) => {
                    println!("Config valid: {} agents configured", cfg.agents.len());
                    for (name, agent) in &cfg.agents {
                        println!("  - {name}: {} ({})", agent.command, agent.mode);
                    }
                }
                Err(e) => eprintln!("Config error: {e}"),
            }
        }
    }
}
