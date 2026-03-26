use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{audit_event, AuditEvent, AuditEventKind};

pub fn collect_trace_audit_events(
    request_id: &str,
    tenant_id: Option<&str>,
    trace_prefix: &Path,
) -> Vec<AuditEvent> {
    let Some(parent) = trace_prefix.parent() else {
        return Vec::new();
    };
    let Some(prefix_name) = trace_prefix.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };

    let mut paths: Vec<PathBuf> = match fs::read_dir(parent) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name == prefix_name || name.starts_with(&format!("{prefix_name}.")))
                    .unwrap_or(false)
            })
            .collect(),
        Err(_) => return Vec::new(),
    };
    paths.sort();

    let mut events = Vec::new();
    for path in paths {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        for line in content.lines() {
            if let Some(event) = parse_strace_line(request_id, tenant_id, line) {
                events.push(event);
            }
        }
        let _ = fs::remove_file(path);
    }
    events
}

fn parse_strace_line(request_id: &str, tenant_id: Option<&str>, line: &str) -> Option<AuditEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if starts_with_any(trimmed, &["open(", "openat(", "access(", "stat(", "lstat("]) {
        let path = first_quoted(trimmed)?;
        let action = if trimmed.contains("O_WRONLY") || trimmed.contains("O_RDWR") {
            "write"
        } else {
            "read"
        };
        return Some(syscall_event(
            request_id,
            tenant_id,
            AuditEventKind::FileAccess,
            format!("sandbox file {action}"),
            &[
                ("syscall", syscall_name(trimmed)),
                ("action", action.to_string()),
                ("path", path),
            ],
        ));
    }

    if starts_with_any(trimmed, &["socket(", "connect(", "sendto("]) {
        let target = extract_network_target(trimmed);
        return Some(syscall_event(
            request_id,
            tenant_id,
            AuditEventKind::NetworkAttempt,
            "sandbox network attempt".to_string(),
            &[("syscall", syscall_name(trimmed)), ("target", target)],
        ));
    }

    if starts_with_any(
        trimmed,
        &["execve(", "clone(", "clone3(", "fork(", "vfork("],
    ) {
        let mut fields = vec![("syscall", syscall_name(trimmed))];
        if let Some(path) = first_quoted(trimmed) {
            fields.push(("path", path));
        }
        return Some(syscall_event(
            request_id,
            tenant_id,
            AuditEventKind::ProcessSpawn,
            "sandbox process spawn attempt".to_string(),
            &fields,
        ));
    }

    None
}

fn syscall_event(
    request_id: &str,
    tenant_id: Option<&str>,
    kind: AuditEventKind,
    message: String,
    fields: &[(&str, String)],
) -> AuditEvent {
    let mut event = audit_event(request_id.to_string(), tenant_id, kind, message);
    for (key, value) in fields {
        if !value.is_empty() {
            event.fields.insert((*key).to_string(), value.clone());
        }
    }
    event
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}

fn syscall_name(line: &str) -> String {
    line.split('(').next().unwrap_or_default().to_string()
}

fn first_quoted(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_network_target(line: &str) -> String {
    if let Some(ip_start) = line.find("inet_addr(\"") {
        let rest = &line[ip_start + "inet_addr(\"".len()..];
        if let Some(ip_end) = rest.find('"') {
            let ip = &rest[..ip_end];
            if let Some(port_start) = line.find("htons(") {
                let port_rest = &line[port_start + "htons(".len()..];
                if let Some(port_end) = port_rest.find(')') {
                    return format!("{ip}:{}", &port_rest[..port_end]);
                }
            }
            return ip.to_string();
        }
    }

    if let Some(path) = first_quoted(line) {
        return path;
    }

    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_strace_line;
    use crate::AuditEventKind;

    #[test]
    fn parses_file_access_lines() {
        let event = parse_strace_line(
            "req-1",
            Some("tenant-a"),
            "openat(AT_FDCWD, \"/workspace/main.py\", O_RDONLY|O_CLOEXEC) = 3",
        )
        .unwrap();
        assert_eq!(event.kind, AuditEventKind::FileAccess);
        assert_eq!(event.fields["path"], "/workspace/main.py");
        assert_eq!(event.fields["action"], "read");
    }

    #[test]
    fn parses_network_lines() {
        let event = parse_strace_line(
            "req-1",
            Some("tenant-a"),
            "connect(3, {sa_family=AF_INET, sin_port=htons(80), sin_addr=inet_addr(\"1.1.1.1\")}, 16) = -1 EPERM (Operation not permitted)",
        )
        .unwrap();
        assert_eq!(event.kind, AuditEventKind::NetworkAttempt);
        assert_eq!(event.fields["target"], "1.1.1.1:80");
    }

    #[test]
    fn parses_exec_lines() {
        let event = parse_strace_line(
            "req-1",
            Some("tenant-a"),
            "execve(\"/bin/sh\", [\"sh\"], 0x0) = -1 EPERM (Operation not permitted)",
        )
        .unwrap();
        assert_eq!(event.kind, AuditEventKind::ProcessSpawn);
        assert_eq!(event.fields["path"], "/bin/sh");
    }
}
