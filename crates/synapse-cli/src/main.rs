use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, Subcommand};
use synapse_core::RuntimeRegistry;
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
    /// Runtime related commands
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    /// Check system requirements for the MVP sandbox
    Doctor,
}

#[derive(Debug, Subcommand)]
enum RuntimeCommand {
    /// List installed runtimes
    List,
    /// Install a runtime binary into the managed store
    Install {
        /// Runtime language
        #[arg(long, default_value = "python")]
        language: String,
        /// Runtime version label
        #[arg(long)]
        version: String,
        /// Path to the runtime binary to import
        #[arg(long)]
        source: PathBuf,
        /// Activate the runtime after installation
        #[arg(long, default_value_t = false)]
        activate: bool,
    },
    /// Activate an installed runtime version
    Activate {
        /// Runtime language
        #[arg(long, default_value = "python")]
        language: String,
        /// Installed runtime version label
        #[arg(long)]
        version: String,
    },
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
        Commands::Runtime { command } => {
            handle_runtime_command(command)?;
        }
        Commands::Doctor => doctor::run()?,
    }
    Ok(())
}

fn handle_runtime_command(command: RuntimeCommand) -> Result<(), Box<dyn std::error::Error>> {
    let registry = RuntimeRegistry::default();

    match command {
        RuntimeCommand::List => {
            println!("store\t{}", registry.root().display());
            for runtime in registry.list() {
                let status = if runtime.active {
                    "active"
                } else {
                    "installed"
                };
                let health = if runtime.healthy { "ok" } else { "corrupt" };
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    runtime.language,
                    runtime.version,
                    runtime.command,
                    status,
                    health,
                    runtime.binary.display()
                );
            }
        }
        RuntimeCommand::Install {
            language,
            version,
            source,
            activate,
        } => {
            let runtime = registry.install(&language, &version, &source)?;
            println!(
                "installed\t{}\t{}\t{}\t{}",
                runtime.language,
                runtime.version,
                runtime.command,
                runtime.binary.display()
            );
            if activate {
                let runtime = registry.activate(&language, &version)?;
                println!(
                    "activated\t{}\t{}\t{}",
                    runtime.language,
                    runtime.version,
                    runtime.binary.display()
                );
            }
        }
        RuntimeCommand::Activate { language, version } => {
            let runtime = registry.activate(&language, &version)?;
            println!(
                "activated\t{}\t{}\t{}",
                runtime.language,
                runtime.version,
                runtime.binary.display()
            );
        }
    }

    Ok(())
}
