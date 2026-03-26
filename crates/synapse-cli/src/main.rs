use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, Subcommand};
use synapse_core::{Providers, RuntimeInstallSource, RuntimeRegistry, SystemProviders};
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
    /// Initialize Synapse for first-run usage
    Init,
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
    /// Install a runtime bundle directory with manifest.json into the managed store
    InstallBundle {
        /// Path to the runtime bundle directory
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
        Commands::Init => init()?,
    }
    Ok(())
}

fn init() -> Result<(), Box<dyn std::error::Error>> {
    for line in execute_init(&SystemProviders, &RuntimeRegistry::default())? {
        println!("{line}");
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
        RuntimeCommand::InstallBundle { source, activate } => {
            let runtime = registry.install_bundle(&source)?;
            lines.push(format!(
                "installed\t{}\t{}\t{}\t{}",
                runtime.language,
                runtime.version,
                runtime.command,
                runtime.binary.display()
            ));
            if activate {
                let runtime = registry.activate(&runtime.language, &runtime.version)?;
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

fn execute_init(
    providers: &dyn Providers,
    registry: &RuntimeRegistry,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    execute_init_with_checks(providers, registry, doctor::collect_checks(providers))
}

fn execute_init_with_checks(
    providers: &dyn Providers,
    registry: &RuntimeRegistry,
    checks: Vec<doctor::DoctorCheck>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut lines = vec![format!("store\t{}", registry.root().display())];
    let mut blocking_failures = Vec::new();

    for check in checks {
        let status = if check.ok { "ok" } else { "fail" };
        lines.push(format!("check\t{status}\t{}\t{}", check.name, check.detail));
        if !check.ok {
            if let Some(remediation) = check.remediation {
                lines.push(format!("fix\t{}\t{}", check.name, remediation));
            }
            if check.name != "runtime" {
                blocking_failures.push(check.name);
            }
        }
    }

    if !blocking_failures.is_empty() {
        return Err(format!(
            "synapse init blocked by prerequisite checks: {}",
            blocking_failures.join(", ")
        )
        .into());
    }

    let bootstrapped = registry.ensure_default_runtime(providers)?;
    let runtime = registry.verify("python", None)?;
    lines.push(format!(
        "runtime\tready\t{}\t{}\t{}\t{}\t{}",
        runtime.language,
        runtime.version,
        runtime.command,
        runtime.binary.display(),
        runtime_source_label(&bootstrapped.source)
    ));
    if let Some(installed_from) = runtime.installed_from.as_deref() {
        lines.push(format!("runtime-source\t{installed_from}"));
    }
    lines.push("next\tsynapse serve --listen 127.0.0.1:8080".to_string());
    lines.push("next\tcurl http://127.0.0.1:8080/health".to_string());

    Ok(lines)
}

fn runtime_source_label(source: &RuntimeInstallSource) -> &'static str {
    match source {
        RuntimeInstallSource::Manual => "manual install",
        RuntimeInstallSource::Bundle => "bundle",
        RuntimeInstallSource::HostImport => "host import",
    }
}

#[cfg(test)]
mod tests {
    use super::{execute_init, execute_init_with_checks, execute_runtime_command, RuntimeCommand};
    use std::{
        collections::HashMap,
        env,
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
    };
    use synapse_core::{Providers, RuntimeInstallSource, RuntimeRegistry, SystemProviders};

    #[derive(Debug, Default)]
    struct FakeProviders {
        env: HashMap<String, OsString>,
        temp_dir: PathBuf,
        pid: u32,
        nanos: u128,
    }

    impl Providers for FakeProviders {
        fn env_var(&self, key: &str) -> Option<String> {
            self.env
                .get(key)
                .map(|value| value.to_string_lossy().into_owned())
        }

        fn env_var_os(&self, key: &str) -> Option<OsString> {
            self.env.get(key).cloned()
        }

        fn temp_dir(&self) -> PathBuf {
            self.temp_dir.clone()
        }

        fn process_id(&self) -> u32 {
            self.pid
        }

        fn now_unix_nanos(&self) -> u128 {
            self.nanos
        }
    }

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

    fn fake_providers_with_path(path: &Path) -> FakeProviders {
        let mut providers = FakeProviders {
            temp_dir: env::temp_dir(),
            pid: 7,
            nanos: 11,
            ..Default::default()
        };
        providers
            .env
            .insert("PATH".to_string(), path.as_os_str().to_os_string());
        providers
    }

    fn fake_runtime_bundle(source_root: &Path, version: &str) -> PathBuf {
        let bundle_source_root = source_root.join("bundle-source-store");
        let source_registry = RuntimeRegistry::from_root(&bundle_source_root);
        let binary = fake_runtime_binary(&source_root.join("bundle-bin"), "python3");
        source_registry.install("python", version, &binary).unwrap();
        bundle_source_root.join(format!("runtimes/python/{version}"))
    }

    fn passing_init_checks() -> Vec<crate::doctor::DoctorCheck> {
        vec![
            crate::doctor::DoctorCheck {
                name: "sandbox",
                ok: true,
                detail: "supported".to_string(),
                remediation: None,
            },
            crate::doctor::DoctorCheck {
                name: "strace",
                ok: true,
                detail: "present".to_string(),
                remediation: None,
            },
            crate::doctor::DoctorCheck {
                name: "cgroupv2",
                ok: true,
                detail: "present".to_string(),
                remediation: None,
            },
            crate::doctor::DoctorCheck {
                name: "runtime",
                ok: false,
                detail: "missing".to_string(),
                remediation: Some("bootstrap".to_string()),
            },
            crate::doctor::DoctorCheck {
                name: "audit",
                ok: true,
                detail: "writable".to_string(),
                remediation: None,
            },
            crate::doctor::DoctorCheck {
                name: "tempdir",
                ok: true,
                detail: "writable".to_string(),
                remediation: None,
            },
        ]
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

    #[test]
    fn import_host_command_can_activate_runtime() {
        let root = unique_root("synapse-cli-runtime-import-host");
        let registry = RuntimeRegistry::from_root(&root);
        let host_bin_dir = root.join("host-bin");
        fake_runtime_binary(&host_bin_dir, "python3");
        let providers = fake_providers_with_path(&host_bin_dir);

        let lines = execute_runtime_command(
            &registry,
            &providers,
            RuntimeCommand::ImportHost {
                language: "python".to_string(),
                version: "system".to_string(),
                command: "python3".to_string(),
                activate: true,
            },
        )
        .unwrap();

        assert!(lines[0].starts_with("store\t"));
        assert!(lines[1].contains("activated\tpython\tsystem\tpython3\t"));

        let runtime = registry.verify("python", None).unwrap();
        assert_eq!(runtime.version, "system");
        assert!(runtime.active);
        assert_eq!(runtime.command, "python3");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn install_bundle_command_can_activate_runtime() {
        let root = unique_root("synapse-cli-runtime-install-bundle");
        let registry = RuntimeRegistry::from_root(&root);
        let bundle = fake_runtime_bundle(&root, "3.12.6");

        let lines = execute_runtime_command(
            &registry,
            &SystemProviders,
            RuntimeCommand::InstallBundle {
                source: bundle,
                activate: true,
            },
        )
        .unwrap();

        assert!(lines[0].starts_with("store\t"));
        assert!(lines[1].contains("installed\tpython\t3.12.6\tpython3\t"));
        assert!(lines[2].contains("activated\tpython\t3.12.6\t"));

        let runtime = registry.verify("python", None).unwrap();
        assert_eq!(runtime.version, "3.12.6");
        assert!(runtime.active);
        assert_eq!(runtime.command, "python3");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn init_bootstraps_runtime_from_host_when_missing() {
        let root = unique_root("synapse-cli-init-host-bootstrap");
        let registry = RuntimeRegistry::from_root(&root);
        let host_bin_dir = root.join("host-bin");
        fake_runtime_binary(&host_bin_dir, "python3");
        let providers = fake_providers_with_path(&host_bin_dir);

        let lines = execute_init_with_checks(&providers, &registry, passing_init_checks()).unwrap();

        assert!(lines
            .iter()
            .any(|line| line.starts_with("runtime\tready\tpython\tsystem")));
        assert!(lines.iter().any(|line| line.ends_with("\thost import")));

        let runtime = registry.verify("python", None).unwrap();
        assert_eq!(runtime.install_source, RuntimeInstallSource::HostImport);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn init_is_idempotent_when_runtime_already_active() {
        let root = unique_root("synapse-cli-init-idempotent");
        let registry = RuntimeRegistry::from_root(&root);
        let host_bin_dir = root.join("host-bin");
        let binary = fake_runtime_binary(&host_bin_dir, "python3");
        registry.install("python", "3.12.8", &binary).unwrap();
        registry.activate("python", "3.12.8").unwrap();
        let providers = fake_providers_with_path(&host_bin_dir);

        let first = execute_init_with_checks(&providers, &registry, passing_init_checks()).unwrap();
        let second =
            execute_init_with_checks(&providers, &registry, passing_init_checks()).unwrap();

        assert!(first.iter().any(|line| line.contains("\tmanual install")));
        assert!(second.iter().any(|line| line.contains("\tmanual install")));
        let listed = registry.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].version, "3.12.8");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn init_reports_blocking_checks() {
        let root = unique_root("synapse-cli-init-blocked");
        let registry = RuntimeRegistry::from_root(&root);
        let providers = fake_providers_with_path(&root);

        let error = execute_init(&providers, &registry).unwrap_err();
        assert!(error.to_string().contains("blocked by prerequisite checks"));

        let _ = fs::remove_dir_all(root);
    }
}
