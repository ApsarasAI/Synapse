use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, Subcommand};
use synapse_core::{Providers, RuntimeRegistry, SystemProviders};
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
    /// Verify an installed runtime and its active pointer
    Verify {
        /// Runtime language
        #[arg(long, default_value = "python")]
        language: String,
        /// Installed runtime version label. Uses the active version when omitted.
        #[arg(long)]
        version: Option<String>,
    },
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
    /// Import a runtime binary from the host PATH into the managed store
    ImportHost {
        /// Runtime language
        #[arg(long, default_value = "python")]
        language: String,
        /// Runtime version label to store
        #[arg(long, default_value = "system")]
        version: String,
        /// Host command to import from PATH
        #[arg(long, default_value = "python3")]
        command: String,
        /// Activate the runtime after import
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
    let providers = SystemProviders;

    for line in execute_runtime_command(&registry, &providers, command)? {
        println!("{line}");
    }

    Ok(())
}

fn execute_runtime_command(
    registry: &RuntimeRegistry,
    providers: &dyn Providers,
    command: RuntimeCommand,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut lines = vec![format!("store\t{}", registry.root().display())];

    match command {
        RuntimeCommand::List => {
            for runtime in registry.list() {
                let status = if runtime.active {
                    "active"
                } else {
                    "installed"
                };
                let health = if runtime.healthy { "ok" } else { "corrupt" };
                lines.push(format!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    runtime.language,
                    runtime.version,
                    runtime.command,
                    status,
                    health,
                    runtime.binary.display()
                ));
            }
        }
        RuntimeCommand::Verify { language, version } => {
            let runtime = registry.verify(&language, version.as_deref())?;
            lines.push(format!(
                "verified\t{}\t{}\t{}\t{}\t{}",
                runtime.language,
                runtime.version,
                runtime.command,
                if runtime.active { "active" } else { "inactive" },
                runtime.binary.display()
            ));
        }
        RuntimeCommand::Install {
            language,
            version,
            source,
            activate,
        } => {
            let runtime = registry.install(&language, &version, &source)?;
            lines.push(format!(
                "installed\t{}\t{}\t{}\t{}",
                runtime.language,
                runtime.version,
                runtime.command,
                runtime.binary.display()
            ));
            if activate {
                let runtime = registry.activate(&language, &version)?;
                lines.push(format!(
                    "activated\t{}\t{}\t{}",
                    runtime.language,
                    runtime.version,
                    runtime.binary.display()
                ));
            }
        }
        RuntimeCommand::ImportHost {
            language,
            version,
            command,
            activate,
        } => {
            let runtime =
                registry.import_host_runtime(providers, &language, &version, &command, activate)?;
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                if runtime.active {
                    "activated"
                } else {
                    "imported"
                },
                runtime.language,
                runtime.version,
                runtime.command,
                runtime.binary.display()
            ));
        }
        RuntimeCommand::Activate { language, version } => {
            let runtime = registry.activate(&language, &version)?;
            lines.push(format!(
                "activated\t{}\t{}\t{}",
                runtime.language,
                runtime.version,
                runtime.binary.display()
            ));
        }
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::{execute_runtime_command, RuntimeCommand};
    use std::{env, fs, path::PathBuf};
    use synapse_core::{RuntimeRegistry, SystemProviders};

    fn unique_root(prefix: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "{prefix}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn fake_runtime_binary(root: &PathBuf, name: &str) -> PathBuf {
        fs::create_dir_all(root).unwrap();
        let path = root.join(name);
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
        }
        path
    }

    #[test]
    fn verify_command_reports_active_runtime() {
        let root = unique_root("synapse-cli-runtime-verify");
        let registry = RuntimeRegistry::from_root(&root);
        let binary = fake_runtime_binary(&root.join("src"), "python3");
        registry.install("python", "3.12.4", &binary).unwrap();
        registry.activate("python", "3.12.4").unwrap();

        let lines = execute_runtime_command(
            &registry,
            &SystemProviders,
            RuntimeCommand::Verify {
                language: "python".to_string(),
                version: None,
            },
        )
        .unwrap();

        assert!(lines[0].starts_with("store\t"));
        assert!(lines[1].contains("verified\tpython\t3.12.4\tpython3\tactive"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verify_command_fails_without_active_runtime() {
        let root = unique_root("synapse-cli-runtime-verify-missing");
        let registry = RuntimeRegistry::from_root(&root);

        let error = execute_runtime_command(
            &registry,
            &SystemProviders,
            RuntimeCommand::Verify {
                language: "python".to_string(),
                version: None,
            },
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("no active runtime configured for python"));

        let _ = fs::remove_dir_all(root);
    }
}
