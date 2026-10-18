use clap::Parser;
use hyper_edge::config::EdgeConfig;
use hyper_edge::error::Result;
use hyper_edge::start_edge_server;
use std::path::PathBuf;
use tokio::signal;
use tokio::sync::watch;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(
    name = "hyper-edge",
    author = "koushik4475 <koushik4475@gmail.com>",
    version = "0.1.0",
    about = "Global Anycast Edge Distributed Cache & Reverse Proxy"
)]
struct Args {
    /// Path to YAML or JSON configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Override listen port
    #[arg(short, long)]
    port: Option<u16>,

    /// Override local edge PoP code (e.g. IAD, SFO, LHR, NRT)
    #[arg(long)]
    pop: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hyper_edge=info,hyper=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let mut config = if let Some(config_path) = args.config {
        let content = std::fs::read_to_string(&config_path)
            .unwrap_or_else(|e| panic!("Failed to read config file {:?}: {}", config_path, e));
        serde_json::from_str::<EdgeConfig>(&content)
            .unwrap_or_else(|e| panic!("Failed to parse config file: {}", e))
    } else {
        info!("No configuration file provided; using production defaults");
        EdgeConfig::default()
    };

    if let Some(port) = args.port {
        config.server.port = port;
    }
    if let Some(pop) = args.pop {
        config.geo.local_pop_code = pop;
    }

    info!(
        pop = config.geo.local_pop_code.as_str(),
        port = config.server.port,
        "Initializing HyperEdge daemon"
    );

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Ctrl-C received, initiating shutdown");
                let _ = shutdown_tx.send(true);
            }
            Err(err) => {
                error!("Unable to listen for shutdown signal: {}", err);
            }
        }
    });

    start_edge_server(config, shutdown_rx).await
}
