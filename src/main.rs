use clap::{Parser, Subcommand};
use locci_kv::{Config, Server};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "locci-kv")]
#[command(author, version, about = "A distributed key-value store built on Raft", long_about = None)]
struct Cli {
    /// Path to config file (can be set via LOCCI_CONFIG env var)
    #[arg(short, long, env = "LOCCI_KV_CONFIG")]
    config: Option<String>,

    /// Server ID
    #[arg(long, env = "LOCCI_KV_SERVER_ID")]
    id: Option<u64>,

    /// Server bind address
    #[arg(long, env = "LOCCI_KV_BIND_ADDR")]
    bind_addr: Option<String>,

    /// Data directory
    #[arg(long, env = "LOCCI_KV_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "LOCCI_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Enable Raft consensus (default: false for Phase 1 compatibility)
    #[arg(long, env = "LOCCI_ENABLE_RAFT")]
    enable_raft: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Locci KV server
    Start {
        /// Bootstrap a new Raft cluster
        #[arg(long)]
        bootstrap: bool,
    },

    /// Run in standalone mode (single node, no Raft)
    Standalone,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing/logging
    let log_level = cli
        .log_level
        .parse::<tracing::Level>()
        .unwrap_or(tracing::Level::INFO);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("locci_kv={},tower_http=debug", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    // Load configuration
    let mut config = Config::load(cli.config)?;

    // Merge CLI overrides
    config.merge_overrides(cli.id, cli.bind_addr, cli.data_dir);

    match cli.command {
        Some(Commands::Start { bootstrap }) => {
            tracing::info!("Starting Locci KV server...");

            // Update bootstrap flag from CLI
            config.cluster.bootstrap = bootstrap;

            // Create server
            let mut server = Server::new(config)?;

            // Enable Raft if requested
            if cli.enable_raft {
                server = server.with_raft().await?;
            }

            server.start().await?;
        }
        Some(Commands::Standalone) | None => {
            tracing::info!("Starting Locci KV in standalone mode (no Raft)");
            let server = Server::new(config)?;
            server.start().await?;
        }
    }

    Ok(())
}
