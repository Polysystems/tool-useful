//! Common types used throughout the framework.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A tool call from an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub arguments: Value,
}

impl ToolCall {
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            id: None,
            name: name.into(),
            arguments,
        }
    }

    pub fn with_id(id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            id: Some(id.into()),
            name: name.into(),
            arguments,
        }
    }
}

/// Output from a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub tool_name: String,
    pub content: Value,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolOutput {
    pub fn success(tool_name: impl Into<String>, content: Value) -> Self {
        Self {
            tool_call_id: None,
            tool_name: tool_name.into(),
            content,
            success: true,
            error: None,
        }
    }

    pub fn failure(tool_name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            tool_call_id: None,
            tool_name: tool_name.into(),
            content: Value::Null,
            success: false,
            error: Some(error.into()),
        }
    }
}
