use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{find_command, SynapseError, SystemProviders};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeInfo {
    pub language: String,
    pub requested_version: Option<String>,
    pub resolved_version: String,
    pub command: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedRuntime {
    pub binary: PathBuf,
    pub info: RuntimeInfo,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRegistry;

impl RuntimeRegistry {
    pub fn resolve(
        &self,
        language: &str,
        requested_version: Option<&str>,
    ) -> Result<ResolvedRuntime, SynapseError> {
        let normalized = language.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "python" | "python3" => {
                let binary = resolve_binary("python3")?;
                Ok(ResolvedRuntime {
                    binary,
                    info: RuntimeInfo {
                        language: "python".to_string(),
                        requested_version: requested_version.map(str::to_string),
                        resolved_version: requested_version.unwrap_or("system").to_string(),
                        command: "python3".to_string(),
                    },
                })
            }
            other => Err(SynapseError::UnsupportedLanguage(other.to_string())),
        }
    }

    pub fn list(&self) -> Vec<RuntimeInfo> {
        let available = resolve_binary("python3").ok().is_some();
        vec![RuntimeInfo {
            language: "python".to_string(),
            requested_version: None,
            resolved_version: if available {
                "system".to_string()
            } else {
                "unavailable".to_string()
            },
            command: "python3".to_string(),
        }]
    }
}

fn resolve_binary(binary: &str) -> Result<PathBuf, SynapseError> {
    let binary_path = Path::new(binary);
    if binary_path.is_absolute() && binary_path.exists() {
        return canonicalize_binary(binary_path);
    }

    let providers = SystemProviders;
    let Some(path) = find_command(&providers, binary) else {
        return Err(SynapseError::RuntimeUnavailable(format!(
            "{binary} is not available in PATH"
        )));
    };

    canonicalize_binary(&path)
}

fn canonicalize_binary(path: &Path) -> Result<PathBuf, SynapseError> {
    std::fs::canonicalize(path).or_else(|_| Ok(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::RuntimeRegistry;
    use crate::SynapseError;

    #[test]
    fn registry_lists_python_runtime() {
        let items = RuntimeRegistry.list();
        assert_eq!(items[0].language, "python");
    }

    #[test]
    fn registry_rejects_unknown_runtime() {
        let error = RuntimeRegistry.resolve("ruby", None).unwrap_err();
        assert!(matches!(error, SynapseError::UnsupportedLanguage(language) if language == "ruby"));
    }
}
