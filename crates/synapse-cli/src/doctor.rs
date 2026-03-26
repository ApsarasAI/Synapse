use std::{fs, io};

use synapse_core::{
    find_command, temp_path, AuditLog, Providers, RuntimeInstallSource, RuntimeRegistry,
    SystemProviders,
};
#[cfg(target_os = "linux")]
use synapse_core::{probe_cgroup_v2_support, probe_linux_sandbox_support};

#[derive(Debug)]
pub struct DoctorCheck {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    pub remediation: Option<String>,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let providers = SystemProviders;
    let checks = collect_checks(&providers);

    for check in &checks {
        let status = if check.ok { "ok" } else { "fail" };
        println!("[{status}] {}: {}", check.name, check.detail);
        if !check.ok {
            if let Some(remediation) = &check.remediation {
                println!("  fix: {remediation}");
            }
        }
    }

    if checks.iter().all(|check| check.ok) {
        println!("Synapse doctor passed");
        Ok(())
    } else {
        Err(Box::new(io::Error::other(
            "Synapse doctor found one or more blocking issues",
        )))
    }
}

pub fn collect_checks(providers: &dyn Providers) -> Vec<DoctorCheck> {
    vec![
        sandbox_tool_check(providers),
        command_check(providers, "strace", "required for syscall audit capture"),
        cgroup_v2_check(providers),
        runtime_check(),
        audit_log_check(providers),
        temp_dir_check(providers),
    ]
}

fn command_check(
    providers: &dyn Providers,
    command: &'static str,
    detail: &'static str,
) -> DoctorCheck {
    match find_command(providers, command) {
        Some(path) => DoctorCheck {
            name: command,
            ok: true,
            detail: format!("{detail} ({})", path.display()),
            remediation: None,
        },
        None => DoctorCheck {
            name: command,
            ok: false,
            detail: format!("{detail}; command not found in PATH"),
            remediation: Some(format!(
                "install `{command}` and ensure it is available in PATH"
            )),
        },
    }
}

fn sandbox_tool_check(providers: &dyn Providers) -> DoctorCheck {
    if find_command(providers, "bwrap").is_none() {
        return DoctorCheck {
            name: "sandbox",
            ok: false,
            detail: "bubblewrap is required for secure Linux execution; command not found in PATH"
                .to_string(),
            remediation: Some("install `bwrap` (bubblewrap) on the host".to_string()),
        };
    }

    #[cfg(target_os = "linux")]
    {
        match probe_linux_sandbox_support() {
            Ok(detail) => DoctorCheck {
                name: "sandbox",
                ok: true,
                detail,
                remediation: None,
            },
            Err(error) => DoctorCheck {
                name: "sandbox",
                ok: false,
                detail: error.to_string(),
                remediation: Some(
                    "enable unprivileged user namespaces and verify bubblewrap can create a sandbox"
                        .to_string(),
                ),
            },
        }
    }

    #[cfg(not(target_os = "linux"))]
    DoctorCheck {
        name: "sandbox",
        ok: false,
        detail: "secure sandbox execution is only supported on Linux".to_string(),
        remediation: Some("run Synapse on a Linux host".to_string()),
    }
}

#[cfg(target_os = "linux")]
fn cgroup_v2_check(providers: &dyn Providers) -> DoctorCheck {
    match probe_cgroup_v2_support(providers) {
        Ok(support) => DoctorCheck {
            name: "cgroupv2",
            ok: true,
            detail: format!(
                "cgroups v2 available at {} with controllers {}",
                support.root.display(),
                support.controllers.join(",")
            ),
            remediation: None,
        },
        Err(error) => DoctorCheck {
            name: "cgroupv2",
            ok: false,
            detail: error.to_string(),
            remediation: Some(
                "mount a writable cgroup v2 hierarchy and expose cpu,memory,pids controllers"
                    .to_string(),
            ),
        },
    }
}

#[cfg(not(target_os = "linux"))]
fn cgroup_v2_check(_providers: &dyn Providers) -> DoctorCheck {
    DoctorCheck {
        name: "cgroupv2",
        ok: false,
        detail: "cgroups v2 checks are only supported on Linux".to_string(),
        remediation: Some("run Synapse on a Linux host".to_string()),
    }
}

fn temp_dir_check(providers: &dyn Providers) -> DoctorCheck {
    let temp_dir = temp_path(providers, "synapse-doctor");

    match fs::write(&temp_dir, b"ok") {
        Ok(()) => {
            let _ = fs::remove_file(&temp_dir);
            DoctorCheck {
                name: "tempdir",
                ok: true,
                detail: format!("temporary workspace writable ({})", temp_dir.display()),
                remediation: None,
            }
        }
        Err(error) => DoctorCheck {
            name: "tempdir",
            ok: false,
            detail: format!(
                "cannot write sandbox workspace in {}: {error}",
                temp_dir.display()
            ),
            remediation: Some(
                "check temporary directory permissions or set a writable TMPDIR".to_string(),
            ),
        },
    }
}

fn runtime_check() -> DoctorCheck {
    let registry = RuntimeRegistry::default();
    match registry.verify("python", None) {
        Ok(runtime) => DoctorCheck {
            name: "runtime",
            ok: true,
            detail: format!(
                "active runtime {}:{} ({}) via {}",
                runtime.language,
                runtime.version,
                runtime.binary.display(),
                runtime_source_label(&runtime.install_source)
            ),
            remediation: None,
        },
        Err(error) => DoctorCheck {
            name: "runtime",
            ok: false,
            detail: format!(
                "{}; import one explicitly with `synapse runtime import-host --activate --version system`",
                error
            ),
            remediation: Some(
                "run `synapse init` to bootstrap a default Python runtime or import one manually"
                    .to_string(),
            ),
        },
    }
}

fn audit_log_check(providers: &dyn Providers) -> DoctorCheck {
    let log = AuditLog::from_providers(providers);
    match fs::create_dir_all(log.root()) {
        Ok(()) => DoctorCheck {
            name: "audit",
            ok: true,
            detail: format!("audit log root writable ({})", log.root().display()),
            remediation: None,
        },
        Err(error) => DoctorCheck {
            name: "audit",
            ok: false,
            detail: format!(
                "cannot initialize audit log root {}: {error}",
                log.root().display()
            ),
            remediation: Some(
                "ensure the audit directory exists and is writable by the Synapse process"
                    .to_string(),
            ),
        },
    }
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
    use super::{collect_checks, command_check, temp_dir_check};
    use synapse_core::SystemProviders;

    #[test]
    fn temp_dir_check_passes_in_normal_env() {
        let check = temp_dir_check(&SystemProviders);
        assert!(check.ok, "{}", check.detail);
    }

    #[test]
    fn command_check_reports_missing_binary() {
        let check = command_check(&SystemProviders, "synapse-does-not-exist", "test binary");
        assert!(!check.ok);
        assert!(check.detail.contains("not found"));
    }

    #[test]
    fn find_command_locates_python_when_available() {
        if let Some(path) = synapse_core::find_command(&SystemProviders, "python3") {
            assert!(path.ends_with("python3") || path.to_string_lossy().contains("python3"));
        }
    }

    #[test]
    fn collect_checks_reports_expected_sections() {
        let checks = collect_checks(&SystemProviders);
        assert!(checks.iter().any(|check| check.name == "sandbox"));
        assert!(checks.iter().any(|check| check.name == "runtime"));
        assert!(checks.iter().any(|check| check.name == "audit"));
    }
}
