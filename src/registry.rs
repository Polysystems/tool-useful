//! Tool registry and discovery system.

use crate::{
    ExecutionContext, FromToolCall, ProviderSchema, Tool, ToolCall, ToolError, ToolExecutor,
    ToolMetadata, ToolResult, ToolSchema,
};
use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

/// Registry for managing and discovering tools
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn DynamicTool>>,
    tags: DashMap<String, HashSet<String>>,
    categories: DashMap<String, HashSet<String>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
            tags: DashMap::new(),
            categories: DashMap::new(),
        }
    }

    pub fn register<T>(&self, tool: T) -> ToolResult<()>
    where
        T: Tool + FromToolCall + ToolExecutor + 'static,
        T::Output: serde::Serialize,
    {
        let wrapper = DynamicToolWrapper::new(tool);
        let metadata = wrapper.metadata();
        let name = metadata.name.clone();

        if self.tools.contains_key(&name) {
            return Err(ToolError::execution_failed(format!(
                "Tool '{}' already registered",
                name
            )));
        }

        for tag in &metadata.tags {
            self.tags
                .entry(tag.clone())
                .or_insert_with(HashSet::new)
                .insert(name.clone());
        }

        if let Some(category) = &metadata.category {
            self.categories
                .entry(category.clone())
                .or_insert_with(HashSet::new)
                .insert(name.clone());
        }

        self.tools.insert(name, Arc::new(wrapper));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn DynamicTool>> {
        self.tools.get(name).map(|entry| Arc::clone(entry.value()))
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tools.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn list_metadata(&self) -> Vec<ToolMetadata> {
        self.tools
            .iter()
            .map(|entry| entry.value().metadata())
            .collect()
    }

    pub fn find_by_tag(&self, tag: &str) -> Vec<String> {
        self.tags
            .get(tag)
            .map(|entry| entry.value().iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn find_by_category(&self, category: &str) -> Vec<String> {
        self.categories
            .get(category)
            .map(|entry| entry.value().iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn export_schemas(&self, provider: Provider) -> Vec<Value> {
        self.tools
            .iter()
            .map(|entry| {
                let schema = entry.value().schema();
                match provider {
                    Provider::OpenAI => schema.to_openai_schema(),
                    Provider::Anthropic => schema.to_anthropic_schema(),
                    Provider::Gemini => schema.to_gemini_schema(),
                    Provider::Generic => schema.to_json_schema(),
                }
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported LLM providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Gemini,
    Generic,
}

/// Type-erased tool that can be executed
pub trait DynamicTool: Send + Sync {
    fn metadata(&self) -> ToolMetadata;
    fn schema(&self) -> ToolSchema;
    fn execute_dynamic<'a>(
        &'a self,
        args: Value,
        ctx: &'a ExecutionContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResult<Value>> + Send + 'a>>;
}

/// Wrapper to make any Tool + ToolExecutor into a DynamicTool
struct DynamicToolWrapper<T> {
    tool: Arc<T>,
}

impl<T> DynamicToolWrapper<T> {
    fn new(tool: T) -> Self {
        Self {
            tool: Arc::new(tool),
        }
    }
}

impl<T> DynamicTool for DynamicToolWrapper<T>
where
    T: Tool + FromToolCall + ToolExecutor + Send + Sync + 'static,
    T::Output: serde::Serialize,
{
    fn metadata(&self) -> ToolMetadata {
        self.tool.metadata()
    }

    fn schema(&self) -> ToolSchema {
        self.tool.schema()
    }

    fn execute_dynamic<'a>(
        &'a self,
        args: Value,
        ctx: &'a ExecutionContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResult<Value>> + Send + 'a>> {
        Box::pin(async move {
            let tool_call = ToolCall::new(self.tool.name(), args);
            let instance = T::from_tool_call(&tool_call)?;
            let result = instance.execute_tool(ctx).await?;
            serde_json::to_value(&result).map_err(|e| ToolError::Serialization(e))
        })
    }
}
