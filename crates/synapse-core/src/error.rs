use thiserror::Error;

#[derive(Debug, Error)]
pub enum SynapseError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
