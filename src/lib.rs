//! # tool-useful
//!
//! **A high-performance, type-safe function calling and tool orchestration framework for Rust AI applications.**
//!
//! ## Performance Features
//! - ⚡ Parallel execution with configurable concurrency
//! - 🔄 Advanced retry policies with exponential backoff & jitter
//! - ⏱️  Circuit breakers to prevent cascading failures
//! - 📊 Built-in metrics and tracing
//! - 🚀 Zero-cost abstractions
//!
//! ## Security Features
//! - 🔒 Permission system (network, filesystem)
//! - 📏 Resource limits (memory, CPU time)
//! - 🛡️  Rate limiting
//! - 🔐 Sandboxing support
//!
//! ## Advanced Features
//! - 📡 Streaming tool execution
//! - 📦 Batch processing
//! - 🌐 Multi-provider schema export (OpenAI, Anthropic, Gemini)

pub mod error;
pub mod executor;
pub mod registry;
pub mod schema;
pub mod security;
pub mod streaming;
pub mod tool;
pub mod types;

// Re-export commonly used items
pub use error::{ErrorKind, ToolError, ToolResult};
pub use executor::{
    CircuitBreaker, ExecutionContext, Executor, ExecutorBuilder, ExecutorMetrics, RetryPolicy,
    RetryStrategy, ToolExecutor,
};
pub use registry::{Provider, ToolRegistry};
pub use schema::{ParameterSchema, ProviderSchema, ToolSchema};
pub use security::{
    FileSystemPermission, NetworkPermission, Permissions, PermissionsBuilder, RateLimiter,
    ResourceTracker,
};
pub use streaming::{collect_stream, StreamingToolExecutor};
pub use tool::{FromToolCall, Tool, ToolMetadata};
pub use types::{ToolCall, ToolOutput};

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::error::{ErrorKind, ToolError, ToolResult};
    pub use crate::executor::{
        ExecutionContext, Executor, ExecutorBuilder, ExecutorMetrics, RetryPolicy, RetryStrategy,
        ToolExecutor,
    };
    pub use crate::registry::{Provider, ToolRegistry};
    pub use crate::schema::{ParameterSchema, ProviderSchema, ToolSchema};
    pub use crate::security::{Permissions, PermissionsBuilder, RateLimiter, ResourceTracker};
    pub use crate::streaming::{collect_stream, StreamingToolExecutor};
    pub use crate::tool::{FromToolCall, Tool, ToolMetadata};
    pub use crate::types::{ToolCall, ToolOutput};
    pub use async_trait::async_trait;
    pub use serde::{Deserialize, Serialize};
}
