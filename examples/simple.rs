//! Simple example showing basic tool usage

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tool_useful::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Calculator {
    a: f64,
    b: f64,
}

impl Tool for Calculator {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::new("add", "Add two numbers")
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new("add", "Add two numbers")
            .with_parameter(
                ParameterSchema::new("a", "number")
                    .with_description("First number")
                    .required(),
            )
            .with_parameter(
                ParameterSchema::new("b", "number")
                    .with_description("Second number")
                    .required(),
            )
    }

    fn name(&self) -> &str {
        "add"
    }
}

impl FromToolCall for Calculator {
    fn from_tool_call(call: &ToolCall) -> ToolResult<Self> {
        serde_json::from_value(call.arguments.clone())
            .map_err(|e| ToolError::invalid_arguments(e.to_string()))
    }
}

#[async_trait]
impl ToolExecutor for Calculator {
    type Output = f64;
    type Error = std::convert::Infallible;

    async fn execute(&self, _ctx: &ExecutionContext) -> Result<f64, Self::Error> {
        Ok(self.a + self.b)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple Calculator Example ===\n");

    // Create a calculator
    let calc = Calculator { a: 10.0, b: 5.0 };

    // Execute it
    let executor = Executor::new();
    let result = executor.execute(&calc).await?;
    println!("Result: {} + {} = {}\n", calc.a, calc.b, result);

    // Show the schema
    println!("OpenAI Schema:");
    println!(
        "{}\n",
        serde_json::to_string_pretty(&calc.schema().to_openai_schema())?
    );

    println!("Anthropic Schema:");
    println!(
        "{}",
        serde_json::to_string_pretty(&calc.schema().to_anthropic_schema())?
    );

    Ok(())
}
