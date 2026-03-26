# Review Findings 2026-03-26

## Open Findings

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

## Reviewed Files (2026-03-26 continuation)

- `crates/synapse-api/src/lib.rs`
- `crates/synapse-api/Cargo.toml`
- `crates/synapse-core/Cargo.toml`
- `crates/synapse-cli/Cargo.toml`
- `TODO.md`
- `docs/session-snapshots/2026-03-25-exec-environment-workarounds.tmp`
- `docs/session-snapshots/2026-03-26-bwrap-wrapper-fix.tmp`
- `docs/architecture/tech-design.md`
- `docs/product/product.md`
- `docs/roadmaps/enterprise-sandbox-roadmap.md`
- `crates/synapse-core/src/cgroups.rs`
- `crates/synapse-core/src/runtime.rs`
- `crates/synapse-core/src/error.rs`
- `crates/synapse-core/src/types.rs`
- `crates/synapse-api/src/app.rs`

## Summary

- **High severity**: 4 findings (scheduler fairness, path traversal, unauthorized audit access, audit tampering)
- **Medium severity**: 4 findings (WebSocket URL limits, 404 vs 500, missing metric increment, log injection risk, silent cgroup fallback)
- **Low severity**: 1 finding (error message information disclosure)

The most critical issues center on the audit subsystem's lack of input validation and access control, which together enable path traversal, unauthorized cross-tenant reads, and audit record overwrites.
