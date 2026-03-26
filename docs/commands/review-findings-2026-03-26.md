# Review Findings 2026-03-26

## Open Findings

### Security Issues (from initial audit)

1. High: `crates/synapse-core/src/scheduler.rs`
   `try_acquire_immediately` rejects any immediate admission when `queued_total > 0`, which can leave global execution capacity idle. If one tenant has queued work because of a per-tenant concurrency cap while the global scheduler still has spare slots, requests from other tenants are also forced into the queue and will not run until some active execution releases a permit.

2. Medium: `crates/synapse-api/src/server.rs`
   `/execute/stream` expects the full `ExecuteRequest` inside the WebSocket GET query string. That makes the streaming API subject to URL length limits in clients, proxies, and servers, so normal-sized code payloads can fail before reaching the handler.

3. Medium: `crates/synapse-api/src/server.rs`
   `GET /audits/:request_id` maps missing audit files to `SynapseError::Io`, and `status_for_error` turns that into HTTP 500. A nonexistent audit record should be reported as 404 rather than an internal server error.

4. High: `crates/synapse-api/src/server.rs`, `crates/synapse-core/src/audit.rs`
   `request_id` is accepted directly from the `x-synapse-request-id` header and then used as part of the audit filename without validation. A crafted value such as `../../tmp/owned` can escape the audit directory and overwrite files accessible to the server process.

5. Medium: `crates/synapse-api/src/metrics.rs`, `crates/synapse-api/src/server.rs`
   The metrics model exposes `synapse_execute_audit_failed_total`, but audit persistence failures are only logged. The counter is never incremented, so the exported metric will stay at zero even when audit writes fail.

6. High: `crates/synapse-api/src/server.rs`
   `GET /audits/:request_id` returns audit contents for any request id without any authentication or tenant ownership check. In a multi-tenant system this lets one caller read another caller's audit trail if they can guess or choose the request id.

7. High: `crates/synapse-core/src/audit.rs`
   `request_id` can be overwritten by callers. The same `request_id` value will overwrite existing audit files, allowing audit record tampering in addition to path traversal. An attacker who controls the header can overwrite another tenant's audit log.

8. Medium: `crates/synapse-api/src/server.rs`
   User-controlled `request_id` and `tenant_id` values are passed directly into tracing structured logging at server.rs:205. While tracing's structured fields generally avoid injection, downstream log formatters that output plain text may be vulnerable to log injection if values contain newlines or control characters.

9. Medium: `crates/synapse-core/src/runtime.rs`
   `ExecutionCgroup::try_create` returns `Ok(None)` when cgroups v2 is unavailable, and execution proceeds without cgroup-based resource limits. The code falls back to `setrlimit` for memory but has no CPU time accounting fallback, so CPU limits are silently ignored on systems without cgroups v2.

10. Low: `crates/synapse-core/src/error.rs`, `crates/synapse-api/src/server.rs`
    `SynapseError::Io` and `SynapseError::Execution` variants include the underlying error message in the response via `to_execute_error()`. This can leak internal paths (e.g., `/var/lib/synapse/...`) and system details to clients. The `Io` error maps to HTTP 500 with full error text, exposing implementation details.

### Security Issues (from git diff review)

11. Medium: `crates/synapse-core/src/runtimes.rs`
    `normalize_language` only converts to lowercase but does not block path separators. When writing the active version marker at `active_root().join(format!("{normalized}.txt"))`, a crafted language like `../other` could escape the active directory. The same issue exists in `runtime_dir` which uses the unvalidated language in path construction.

12. Medium: `crates/synapse-core/src/runtimes.rs`
    `normalized_version` trims whitespace but does not block path separators like `/` or `\`. A version string like `../other` could escape the intended runtime directory when constructing paths in `runtime_dir(language, version)`.

13. Low: `crates/synapse-core/src/runtimes.rs`
    `sha256_file` opens files without `O_NOFOLLOW` on Unix, so a symbolic link could be substituted after the path check but before hashing. An attacker with write access to the runtime store could replace a binary with a symlink to cause hash validation to pass against a different file.

14. Medium: `crates/synapse-core/src/runtimes.rs`, `crates/synapse-core/src/service.rs`
    `RuntimeRegistry::default()` is instantiated multiple times across the codebase (in `service.rs:resolve_runtime`, `server.rs:prepare_execution_audit_events`, `app.rs:default_state`, CLI commands). Each instantiation re-reads the environment variable for the store root and re-scans directories. This is inefficient and could lead to inconsistent behavior if `SYNAPSE_RUNTIME_STORE_DIR` changes mid-request.

15. Low: `crates/synapse-core/src/runtimes.rs`
    `RuntimeManifest.binary_path` is stored as a string and resolved at runtime. If this path is relative, it's joined to the runtime directory; if absolute, it's used as-is. A manifest with an absolute path pointing outside the runtime directory (e.g., `/usr/bin/python3`) would be accepted, bypassing the runtime isolation model.

16. Low: `crates/synapse-cli/src/main.rs`
    `synapse runtime install` accepts a `--source` path from the command line without validating that it's within expected bounds. A user could install a binary from any location, including world-writable directories like `/tmp`, which could be replaced by another user after installation.

## Reviewed Files (2026-03-26 continuation)

### Initial audit
- `crates/synapse-api/src/app.rs`
- `crates/synapse-api/src/server.rs`
- `crates/synapse-api/src/metrics.rs`
- `crates/synapse-api/tests/api_endpoints.rs`
- `crates/synapse-api/tests/security_execute.rs`
- `crates/synapse-core/src/audit.rs`
- `crates/synapse-core/src/cgroups.rs`
- `crates/synapse-core/src/config.rs`
- `crates/synapse-core/src/error.rs`
- `crates/synapse-core/src/lib.rs`
- `crates/synapse-core/src/pool.rs`
- `crates/synapse-core/src/providers.rs`
- `crates/synapse-core/src/runtime.rs`
- `crates/synapse-core/src/scheduler.rs`
- `crates/synapse-core/src/service.rs`
- `crates/synapse-core/src/tenancy.rs`
- `crates/synapse-core/src/types.rs`
- `crates/synapse-cli/src/doctor.rs`
- `crates/synapse-cli/src/main.rs`

### Git diff review
- `crates/synapse-core/src/runtimes.rs` (major changes: +487 lines)
- `crates/synapse-cli/src/main.rs` (runtime CLI commands)
- `crates/synapse-cli/src/doctor.rs` (runtime check update)
- `crates/synapse-core/src/lib.rs` (new exports)
- `crates/synapse-core/src/service.rs` (RuntimeRegistry usage)
- `crates/synapse-api/src/app.rs` (bootstrap_system_defaults call)
- `crates/synapse-api/src/server.rs` (RuntimeRegistry::default() usage)
- `crates/synapse-core/Cargo.toml` (added sha2 dependency)
- `TODO.md` (status update)
- `docs/roadmaps/enterprise-sandbox-roadmap.md` (progress update)

## Summary

- **High severity**: 4 findings (scheduler fairness, path traversal, unauthorized audit access, audit tampering)
- **Medium severity**: 7 findings (WebSocket URL limits, 404 vs 500, missing metric increment, log injection risk, silent cgroup fallback, language/version path traversal x2, RuntimeRegistry duplication)
- **Low severity**: 5 findings (error message disclosure, symlink race, manifest path escape, install source validation, version string validation)

The most critical issues center on the audit subsystem's lack of input validation and access control, which together enable path traversal, unauthorized cross-tenant reads, and audit record overwrites.

The new runtime management feature adds useful capabilities (managed runtimes, SHA-256 integrity, CLI commands) but introduces new input validation gaps in `language` and `version` parameters that should be fixed before release.
