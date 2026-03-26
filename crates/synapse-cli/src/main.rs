use std::net::SocketAddr;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod doctor;

#[derive(Debug, Parser)]
#[command(name = "synapse", about = "AI code execution sandbox")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start the Synapse server
    Serve {
        /// Listen address
        #[arg(long, default_value = "127.0.0.1:8080")]
        listen: SocketAddr,
    },
    /// Runtime related commands (placeholder)
    Runtime,
    /// Check system requirements for the MVP sandbox
    Doctor,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Serve { listen } => synapse_api::server::serve(listen).await?,
        Commands::Runtime => {
            for runtime in synapse_core::RuntimeRegistry.list() {
                println!(
                    "{}\t{}\t{}",
                    runtime.language, runtime.resolved_version, runtime.command
                );
            }
        }
        Commands::Doctor => doctor::run()?,
    }
    Ok(())
}
