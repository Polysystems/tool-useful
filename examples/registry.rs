//! Example showing tool registry and discovery

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tool_useful::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WeatherTool {
    location: String,
}

impl Tool for WeatherTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::new("get_weather", "Get weather for a location")
            .with_category("data")
            .with_tag("weather")
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new("get_weather", "Get weather for a location")
            .with_parameter(ParameterSchema::new("location", "string").required())
    }

    fn name(&self) -> &str {
        "get_weather"
    }
}

impl FromToolCall for WeatherTool {
    fn from_tool_call(call: &ToolCall) -> ToolResult<Self> {
        serde_json::from_value(call.arguments.clone())
            .map_err(|e| ToolError::invalid_arguments(e.to_string()))
    }
}

#[async_trait]
impl ToolExecutor for WeatherTool {
    type Output = String;
    type Error = std::io::Error;

    async fn execute(&self, _ctx: &ExecutionContext) -> Result<String, std::io::Error> {
        Ok(format!("Weather in {}: Sunny, 22°C", self.location))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimeTool {
    timezone: String,
}

impl Tool for TimeTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::new("get_time", "Get current time")
            .with_category("data")
            .with_tag("time")
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new("get_time", "Get current time")
            .with_parameter(ParameterSchema::new("timezone", "string").required())
    }

    fn name(&self) -> &str {
        "get_time"
    }
}

impl FromToolCall for TimeTool {
    fn from_tool_call(call: &ToolCall) -> ToolResult<Self> {
        serde_json::from_value(call.arguments.clone())
            .map_err(|e| ToolError::invalid_arguments(e.to_string()))
    }
}

#[async_trait]
impl ToolExecutor for TimeTool {
    type Output = String;
    type Error = std::io::Error;

    async fn execute(&self, _ctx: &ExecutionContext) -> Result<String, std::io::Error> {
        Ok(format!("Time in {}: 14:30:00", self.timezone))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tool Registry Example ===\n");

    // Create registry
    let registry = ToolRegistry::new();

    // Register tools
    registry.register(WeatherTool {
        location: "".to_string(),
    })?;
    registry.register(TimeTool {
        timezone: "".to_string(),
    })?;

    println!("Registered {} tools\n", registry.len());

    // List tools
    println!("Available tools:");
    for name in registry.list_tools() {
        println!("  - {}", name);
    }
    println!();

    // Find by tag
    println!("Weather tools:");
    for name in registry.find_by_tag("weather") {
        println!("  - {}", name);
    }
    println!();

    // Export schemas
    println!("Anthropic Schemas:");
    let schemas = registry.export_schemas(Provider::Anthropic);
    println!("{}", serde_json::to_string_pretty(&schemas)?);

    Ok(())
}
