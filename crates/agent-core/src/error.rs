use serde::{Deserialize, Serialize};

/// Stable, machine-readable failure classification. Details should be sanitized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorKind {
    Validation,
    UnsupportedCapability,
    Cancelled,
    Timeout,
    Provider,
    Tool,
    CorruptState,
    IncompatibleSchema,
}

/// Framework error. Callers must not put credentials or raw prompts in `message`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct AgentError {
    pub kind: ErrorKind,
    pub message: String,
}

impl AgentError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, message)
    }
}
