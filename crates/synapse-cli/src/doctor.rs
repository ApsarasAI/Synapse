use std::{fs, io};

use synapse_core::{
    find_command, temp_path, AuditLog, Providers, RuntimeRegistry, SystemProviders,
};
#[cfg(target_os = "linux")]
use synapse_core::{probe_cgroup_v2_support, probe_linux_sandbox_support};

#[derive(Debug)]
struct DoctorCheck {
    name: &'static str,
    ok: bool,
    detail: String,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let providers = SystemProviders;
    let checks = vec![
        sandbox_tool_check(&providers),
        command_check(&providers, "strace", "required for syscall audit capture"),
        cgroup_v2_check(&providers),
        runtime_check(),
        audit_log_check(&providers),
        temp_dir_check(&providers),
    ];

    for check in &checks {
        let status = if check.ok { "ok" } else { "fail" };
        println!("[{status}] {}: {}", check.name, check.detail);
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
        },
        None => DoctorCheck {
            name: command,
            ok: false,
            detail: format!("{detail}; command not found in PATH"),
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
        };
    }

    #[cfg(target_os = "linux")]
    {
        match probe_linux_sandbox_support() {
            Ok(detail) => DoctorCheck {
                name: "sandbox",
                ok: true,
                detail,
            },
            Err(error) => DoctorCheck {
                name: "sandbox",
                ok: false,
                detail: error.to_string(),
            },
        }
    }

    #[cfg(not(target_os = "linux"))]
    DoctorCheck {
        name: "sandbox",
        ok: false,
        detail: "secure sandbox execution is only supported on Linux".to_string(),
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
        },
        Err(error) => DoctorCheck {
            name: "cgroupv2",
            ok: false,
            detail: error.to_string(),
        },
    }
}

#[cfg(not(target_os = "linux"))]
fn cgroup_v2_check(_providers: &dyn Providers) -> DoctorCheck {
    DoctorCheck {
        name: "cgroupv2",
        ok: false,
        detail: "cgroups v2 checks are only supported on Linux".to_string(),
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
            }
        }
        Err(error) => DoctorCheck {
            name: "tempdir",
            ok: false,
            detail: format!(
                "cannot write sandbox workspace in {}: {error}",
                temp_dir.display()
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
                "active runtime {}:{} ({})",
                runtime.language,
                runtime.version,
                runtime.binary.display()
            ),
        },
        Err(error) => DoctorCheck {
            name: "runtime",
            ok: false,
            detail: format!(
                "{}; import one explicitly with `synapse runtime import-host --activate --version system`",
                error
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
        },
        Err(error) => DoctorCheck {
            name: "audit",
            ok: false,
            detail: format!(
                "cannot initialize audit log root {}: {error}",
                log.root().display()
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{command_check, temp_dir_check};
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
}
