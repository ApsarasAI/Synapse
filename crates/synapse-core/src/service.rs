use std::path::Path;

use tracing::{info, instrument};

use crate::{
    new_request_id, runtime, ExecuteRequest, ExecuteResponse, LimitSummary, RuntimeInfo,
    RuntimeRegistry, SynapseError, SystemProviders,
};

#[instrument(skip(request), fields(language = %request.language, tenant_id = request.tenant_id.as_deref().unwrap_or("default")))]
pub async fn execute(mut request: ExecuteRequest) -> Result<ExecuteResponse, SynapseError> {
    validate_request(&request)?;
    let request_id = request
        .request_id
        .clone()
        .unwrap_or_else(|| new_request_id(&SystemProviders));
    request.request_id = Some(request_id.clone());

    let sandbox = runtime::prepare_sandbox().await?;
    let result = execute_in_prepared(&sandbox, request).await;
    let _ = sandbox.destroy_blocking();
    info!(request_id, "execution finished in disposable sandbox");
    result
}

#[instrument(skip(sandbox, request), fields(language = %request.language, request_id = request.request_id.as_deref().unwrap_or("generated")))]
pub async fn execute_in_prepared(
    sandbox: &runtime::PreparedSandbox,
    mut request: ExecuteRequest,
) -> Result<ExecuteResponse, SynapseError> {
    validate_request(&request)?;
    let request_id = request
        .request_id
        .clone()
        .unwrap_or_else(|| new_request_id(&SystemProviders));
    request.request_id = Some(request_id.clone());
    sandbox.reset().await?;

    let runtime = resolve_runtime(&request)?;
    let result = execute_in_sandbox(sandbox.path(), &request, &runtime.info).await;
    match sandbox.reset().await {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

async fn execute_in_sandbox(
    sandbox_dir: &Path,
    request: &ExecuteRequest,
    runtime: &RuntimeInfo,
) -> Result<ExecuteResponse, SynapseError> {
    let binary = RuntimeRegistry
        .resolve(&request.language, request.runtime_version.as_deref())?
        .binary;
    let response = runtime::execute_binary(
        &binary,
        &request.code,
        sandbox_dir,
        request.timeout_ms,
        request.effective_cpu_time_limit_ms(),
        request.memory_limit_mb,
    )
    .await?;

    Ok(response.with_request_metadata(
        request
            .request_id
            .clone()
            .unwrap_or_else(|| new_request_id(&SystemProviders)),
        request.tenant_id.as_deref(),
        Some(runtime.clone()),
        LimitSummary {
            wall_time_limit_ms: request.timeout_ms,
            cpu_time_limit_ms: request.effective_cpu_time_limit_ms(),
            memory_limit_mb: request.memory_limit_mb,
        },
    ))
}

fn validate_request(request: &ExecuteRequest) -> Result<(), SynapseError> {
    if request.code.trim().is_empty() {
        return Err(SynapseError::InvalidInput(
            "code cannot be empty".to_string(),
        ));
    }

    if request.timeout_ms == 0 {
        return Err(SynapseError::InvalidInput(
            "timeout_ms must be greater than 0".to_string(),
        ));
    }

    if request.effective_cpu_time_limit_ms() == 0 {
        return Err(SynapseError::InvalidInput(
            "cpu_time_limit_ms must be greater than 0".to_string(),
        ));
    }

    if request.memory_limit_mb == 0 {
        return Err(SynapseError::InvalidInput(
            "memory_limit_mb must be greater than 0".to_string(),
        ));
    }

    Ok(())
}

fn resolve_runtime(request: &ExecuteRequest) -> Result<crate::ResolvedRuntime, SynapseError> {
    RuntimeRegistry.resolve(&request.language, request.runtime_version.as_deref())
}

#[cfg(test)]
mod tests {
    use super::{resolve_runtime, validate_request};
    use crate::{ExecuteRequest, SynapseError};

    fn request() -> ExecuteRequest {
        ExecuteRequest {
            language: "python".to_string(),
            code: "print('ok')\n".to_string(),
            timeout_ms: 5_000,
            cpu_time_limit_ms: Some(3_000),
            memory_limit_mb: 128,
            runtime_version: None,
            tenant_id: None,
            request_id: None,
        }
    }

    #[test]
    fn validate_request_rejects_empty_code() {
        let mut request = request();
        request.code = "   ".to_string();

        let error = validate_request(&request).unwrap_err();
        assert!(
            matches!(error, SynapseError::InvalidInput(message) if message == "code cannot be empty")
        );
    }

    #[test]
    fn validate_request_rejects_zero_timeout() {
        let mut request = request();
        request.timeout_ms = 0;

        let error = validate_request(&request).unwrap_err();
        assert!(
            matches!(error, SynapseError::InvalidInput(message) if message == "timeout_ms must be greater than 0")
        );
    }

    #[test]
    fn validate_request_rejects_zero_cpu_budget() {
        let mut request = request();
        request.cpu_time_limit_ms = Some(0);

        let error = validate_request(&request).unwrap_err();
        assert!(
            matches!(error, SynapseError::InvalidInput(message) if message == "cpu_time_limit_ms must be greater than 0")
        );
    }

    #[test]
    fn validate_request_rejects_zero_memory_limit() {
        let mut request = request();
        request.memory_limit_mb = 0;

        let error = validate_request(&request).unwrap_err();
        assert!(
            matches!(error, SynapseError::InvalidInput(message) if message == "memory_limit_mb must be greater than 0")
        );
    }

    #[test]
    fn resolve_runtime_accepts_python_aliases_case_insensitively() {
        let python = resolve_runtime(&request()).unwrap();
        let mut alias_request = request();
        alias_request.language = "  PyThOn3  ".to_string();
        let python3 = resolve_runtime(&alias_request).unwrap();

        assert_eq!(python.info.language, python3.info.language);
    }

    #[test]
    fn resolve_runtime_rejects_unknown_language() {
        let mut request = request();
        request.language = "ruby".to_string();
        let error = resolve_runtime(&request).unwrap_err();
        assert!(matches!(error, SynapseError::UnsupportedLanguage(language) if language == "ruby"));
    }
}
