//! Advanced example showing performance and security features

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tool_useful::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataProcessor {
    items: Vec<String>,
}

impl Tool for DataProcessor {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::new("process_data", "Process a batch of data items")
            .with_category("processing")
            .with_tag("batch")
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema::new("process_data", "Process a batch of data items").with_parameter(
            ParameterSchema::new("items", "array")
                .with_description("Items to process")
                .required(),
        )
    }

    fn name(&self) -> &str {
        "process_data"
    }
}

impl FromToolCall for DataProcessor {
    fn from_tool_call(call: &ToolCall) -> ToolResult<Self> {
        serde_json::from_value(call.arguments.clone())
            .map_err(|e| ToolError::invalid_arguments(e.to_string()))
    }
}

#[async_trait]
impl ToolExecutor for DataProcessor {
    type Output = Vec<String>;
    type Error = std::io::Error;

    async fn execute(&self, ctx: &ExecutionContext) -> Result<Vec<String>, std::io::Error> {
        let mut results = Vec::new();

        for (i, item) in self.items.iter().enumerate() {
            // Check for cancellation
            ctx.check_cancelled().ok();

            // Simulate processing
            tokio::time::sleep(Duration::from_millis(10)).await;

            results.push(format!("Processed: {}", item));

            // Track progress in metadata
            ctx.set_metadata("progress", format!("{}/{}", i + 1, self.items.len()));
        }

        Ok(results)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Advanced Features Example ===\n");

    // 1. Basic execution with metrics
    println!("1. Execution with Metrics:");
    let executor = Executor::builder()
        .timeout(Duration::from_secs(5))
        .max_concurrent(50)
        .build();

    let tool = DataProcessor {
        items: vec![
            "item1".to_string(),
            "item2".to_string(),
            "item3".to_string(),
        ],
    };

    let result = executor.execute(&tool).await?;
    println!("   Processed {} items", result.len());
    println!("   Success rate: {:.2}%", executor.metrics().success_rate());
    println!(
        "   Avg duration: {:.2}ms\n",
        executor.metrics().avg_duration_ms()
    );

    // 2. Retry with exponential backoff
    println!("2. Retry Policy with Exponential Backoff:");
    let retry_executor = Executor::builder()
        .retry_policy(
            RetryPolicy::exponential(3)
                .with_backoff(Duration::from_millis(100))
                .with_max_delay(Duration::from_secs(5))
                .with_jitter(true),
        )
        .build();

    let _ = retry_executor.execute(&tool).await?;
    println!("   Executed with retry support\n");

    // 3. Circuit breaker
    println!("3. Circuit Breaker:");
    let cb_executor = Executor::builder()
        .circuit_breaker(5, Duration::from_secs(10))
        .build();

    let _ = cb_executor.execute(&tool).await?;
    println!("   Executed with circuit breaker\n");

    // 4. Parallel execution
    println!("4. Parallel Batch Execution:");
    let tools = vec![
        DataProcessor {
            items: vec!["a1".to_string(), "a2".to_string()],
        },
        DataProcessor {
            items: vec!["b1".to_string(), "b2".to_string()],
        },
        DataProcessor {
            items: vec!["c1".to_string(), "c2".to_string()],
        },
    ];

    let batch_results = executor.execute_batch(tools).await;
    println!("   Executed {} tools in parallel", batch_results.len());
    println!(
        "   Successful: {}",
        batch_results.iter().filter(|r| r.is_ok()).count()
    );
    println!();

    // 5. Permissions and security
    println!("5. Permission System:");
    let permissions = Permissions::builder()
        .deny_network()
        .readonly_filesystem(vec!["/tmp".into()])
        .max_memory(50_000_000) // 50MB
        .max_cpu_time(Duration::from_secs(30))
        .build();

    match permissions.check_network_access("api.example.com") {
        Ok(_) => println!("   Network access: Allowed"),
        Err(_) => println!("   Network access: Denied (as expected)"),
    }

    match permissions.check_file_access(std::path::Path::new("/tmp/file.txt")) {
        Ok(_) => println!("   File access to /tmp: Allowed"),
        Err(_) => println!("   File access to /tmp: Denied"),
    }
    println!();

    // 6. Rate limiting
    println!("6. Rate Limiting:");
    let limiter = RateLimiter::per_second(5);

    for i in 1..=3 {
        limiter.acquire().await?;
        println!("   Request {} allowed", i);
    }
    println!();

    // 7. Resource tracking
    println!("7. Resource Tracking:");
    let tracker = ResourceTracker::new(Permissions::builder().max_memory(1_000_000).build());

    tracker.track_memory_allocation(500_000)?;
    println!("   Memory used: {} bytes", tracker.memory_usage());
    println!("   Elapsed time: {:?}", tracker.elapsed_time());
    println!();

    println!("=== All Features Demonstrated ===");

    Ok(())
}
