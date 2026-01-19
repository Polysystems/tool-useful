//! Core tool trait and metadata.

use crate::{ToolCall, ToolResult, ToolSchema};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata about a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ToolMetadata {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            category: None,
            tags: Vec::new(),
            version: None,
            extra: HashMap::new(),
        }
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(|t| t.into()));
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

/// Core trait for all tools
pub trait Tool: Send + Sync {
    fn metadata(&self) -> ToolMetadata;
    fn schema(&self) -> ToolSchema;
    fn name(&self) -> &str {
        "unknown"
    }
}

/// Trait for tools that can be constructed from raw arguments
pub trait FromToolCall: Tool + Sized {
    fn from_tool_call(call: &ToolCall) -> ToolResult<Self>;
}
