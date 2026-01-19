//! Error types for tool execution.

use thiserror::Error;

/// Result type for tool operations
pub type ToolResult<T> = Result<T, ToolError>;

/// Errors that can occur during tool execution
#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Tool execution timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Tool execution was cancelled")]
    Cancelled,

    #[error("Network error: {0}")]
    Network(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation error in field '{field}': {reason}")]
    ValidationError { field: String, reason: String },

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Custom error: {0}")]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl ToolError {
    pub fn execution_failed(msg: impl Into<String>) -> Self {
        ToolError::ExecutionFailed(msg.into())
    }

    pub fn invalid_arguments(msg: impl Into<String>) -> Self {
        ToolError::InvalidArguments(msg.into())
    }

    pub fn network(msg: impl Into<String>) -> Self {
        ToolError::Network(msg.into())
    }

    pub fn validation(field: impl Into<String>, reason: impl Into<String>) -> Self {
        ToolError::ValidationError {
            field: field.into(),
            reason: reason.into(),
        }
    }

    pub fn resource_limit(msg: impl Into<String>) -> Self {
        ToolError::ResourceLimitExceeded(msg.into())
    }

    pub fn permission_denied(msg: impl Into<String>) -> Self {
        ToolError::PermissionDenied(msg.into())
    }

    pub fn tool_not_found(name: impl Into<String>) -> Self {
        ToolError::ToolNotFound(name.into())
    }

    pub fn custom(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        ToolError::Custom(Box::new(err))
    }

    pub fn kind(&self) -> ErrorKind {
        match self {
            ToolError::ExecutionFailed(_) => ErrorKind::Execution,
            ToolError::InvalidArguments(_) | ToolError::ValidationError { .. } => {
                ErrorKind::Validation
            }
            ToolError::Timeout(_) => ErrorKind::Timeout,
            ToolError::Network(_) => ErrorKind::Network,
            ToolError::PermissionDenied(_) => ErrorKind::Permission,
            ToolError::ResourceLimitExceeded(_) => ErrorKind::Resource,
            _ => ErrorKind::Other,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self.kind(),
            ErrorKind::Network | ErrorKind::Timeout | ErrorKind::Resource
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Execution,
    Validation,
    Network,
    Timeout,
    Permission,
    Resource,
    Other,
}
