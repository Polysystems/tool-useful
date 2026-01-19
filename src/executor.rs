//! High-performance tool execution engine with retry, timeout, and resource management.

use crate::{ErrorKind, ToolError, ToolResult};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, Semaphore};
use tracing::{debug, instrument, warn};

/// Context provided during tool execution
#[derive(Clone)]
pub struct ExecutionContext {
    cancellation: Arc<watch::Receiver<bool>>,
    pub timeout: Option<Duration>,
    pub max_memory: Option<usize>,
    pub metadata: Arc<RwLock<ExecutionMetadata>>,
    started_at: Instant,
}

impl ExecutionContext {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        std::mem::drop(tx);
        Self {
            cancellation: Arc::new(rx),
            timeout: None,
            max_memory: None,
            metadata: Arc::new(RwLock::new(ExecutionMetadata::default())),
            started_at: Instant::now(),
        }
    }

    pub fn with_cancellation(cancellation: watch::Receiver<bool>) -> Self {
        Self {
            cancellation: Arc::new(cancellation),
            timeout: None,
            max_memory: None,
            metadata: Arc::new(RwLock::new(ExecutionMetadata::default())),
            started_at: Instant::now(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancellation.borrow()
    }

    pub fn check_cancelled(&self) -> ToolResult<()> {
        if self.is_cancelled() {
            Err(ToolError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub fn set_metadata<V: serde::Serialize>(&self, key: impl Into<String>, value: V) {
        if let Ok(v) = serde_json::to_value(value) {
            self.metadata.write().fields.insert(key.into(), v);
        }
    }

    pub fn get_metadata<T: for<'de> serde::Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.metadata
            .read()
            .fields
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct ExecutionMetadata {
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

/// Trait for types that can execute as tools
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    type Output: serde::Serialize + Send;
    type Error: std::error::Error + Send + Sync + 'static;

    async fn execute(&self, ctx: &ExecutionContext) -> Result<Self::Output, Self::Error>;

    async fn execute_tool(&self, ctx: &ExecutionContext) -> ToolResult<Self::Output> {
        self.execute(ctx).await.map_err(|e| ToolError::custom(e))
    }
}

/// Retry policy with circuit breaker support
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub strategy: RetryStrategy,
    pub retryable_errors: Vec<ErrorKind>,
    pub jitter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryStrategy {
    Fixed,
    Exponential,
    Linear,
}

impl RetryPolicy {
    pub fn exponential(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            strategy: RetryStrategy::Exponential,
            retryable_errors: vec![ErrorKind::Network, ErrorKind::Timeout, ErrorKind::Resource],
            jitter: true,
        }
    }

    pub fn fixed(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay: delay,
            max_delay: delay,
            strategy: RetryStrategy::Fixed,
            retryable_errors: vec![ErrorKind::Network, ErrorKind::Timeout, ErrorKind::Resource],
            jitter: false,
        }
    }

    pub fn with_backoff(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    pub fn should_retry(&self, error: &ToolError) -> bool {
        self.retryable_errors.contains(&error.kind())
    }

    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let delay = match self.strategy {
            RetryStrategy::Fixed => self.base_delay,
            RetryStrategy::Exponential => {
                let multiplier = 2u32.pow(attempt.saturating_sub(1));
                self.base_delay.saturating_mul(multiplier)
            }
            RetryStrategy::Linear => self.base_delay.saturating_mul(attempt),
        };

        let delay = delay.min(self.max_delay);

        if self.jitter {
            // Add jitter: random value between 0.5 and 1.5 times the delay
            let jitter_factor = 0.5 + (rand::random::<f64>() * 1.0);
            Duration::from_secs_f64(delay.as_secs_f64() * jitter_factor)
        } else {
            delay
        }
    }
}

/// Circuit breaker to prevent cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    state: Arc<RwLock<CircuitBreakerState>>,
    failures: Arc<AtomicU64>,
    successes: Arc<AtomicU64>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            failure_threshold,
            success_threshold: 2,
            timeout,
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failures: Arc::new(AtomicU64::new(0)),
            successes: Arc::new(AtomicU64::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
        }
    }

    pub fn call<F, Fut, T>(&self, f: F) -> impl std::future::Future<Output = ToolResult<T>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ToolResult<T>>,
    {
        let state = *self.state.read();
        let should_attempt = match state {
            CircuitBreakerState::Open => {
                if let Some(last_failure) = *self.last_failure_time.read() {
                    last_failure.elapsed() > self.timeout
                } else {
                    false
                }
            }
            _ => true,
        };

        let failures = self.failures.clone();
        let successes = self.successes.clone();
        let state_arc = self.state.clone();
        let last_failure = self.last_failure_time.clone();
        let failure_threshold = self.failure_threshold;
        let success_threshold = self.success_threshold;

        async move {
            if !should_attempt {
                return Err(ToolError::execution_failed("Circuit breaker is open"));
            }

            match f().await {
                Ok(result) => {
                    successes.fetch_add(1, Ordering::Relaxed);
                    let success_count = successes.load(Ordering::Relaxed);

                    if success_count >= success_threshold as u64 {
                        *state_arc.write() = CircuitBreakerState::Closed;
                        failures.store(0, Ordering::Relaxed);
                        successes.store(0, Ordering::Relaxed);
                    }

                    Ok(result)
                }
                Err(err) => {
                    failures.fetch_add(1, Ordering::Relaxed);
                    *last_failure.write() = Some(Instant::now());

                    if failures.load(Ordering::Relaxed) >= failure_threshold as u64 {
                        *state_arc.write() = CircuitBreakerState::Open;
                    }

                    Err(err)
                }
            }
        }
    }
}

/// High-performance executor with advanced features
#[derive(Clone)]
pub struct Executor {
    config: Arc<ExecutorConfig>,
    semaphore: Arc<Semaphore>,
    metrics: Arc<ExecutorMetrics>,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
}

#[derive(Debug)]
struct ExecutorConfig {
    default_timeout: Option<Duration>,
    max_concurrent: usize,
    retry_policy: Option<RetryPolicy>,
    enable_tracing: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            default_timeout: Some(Duration::from_secs(30)),
            max_concurrent: 100,
            retry_policy: None,
            enable_tracing: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct ExecutorMetrics {
    pub total_executions: AtomicUsize,
    pub successful_executions: AtomicUsize,
    pub failed_executions: AtomicUsize,
    pub total_duration_ms: AtomicU64,
}

impl ExecutorMetrics {
    pub fn success_rate(&self) -> f64 {
        let total = self.total_executions.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let successful = self.successful_executions.load(Ordering::Relaxed);
        (successful as f64 / total as f64) * 100.0
    }

    pub fn avg_duration_ms(&self) -> f64 {
        let total = self.total_executions.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let duration = self.total_duration_ms.load(Ordering::Relaxed);
        duration as f64 / total as f64
    }
}

impl Executor {
    pub fn new() -> Self {
        let config = ExecutorConfig::default();
        let max_concurrent = config.max_concurrent;
        Self {
            config: Arc::new(config),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            metrics: Arc::new(ExecutorMetrics::default()),
            circuit_breaker: None,
        }
    }

    pub fn builder() -> ExecutorBuilder {
        ExecutorBuilder::new()
    }

    pub fn metrics(&self) -> &ExecutorMetrics {
        &self.metrics
    }

    #[instrument(skip(self, tool))]
    pub async fn execute<T>(&self, tool: &T) -> ToolResult<T::Output>
    where
        T: ToolExecutor,
    {
        let ctx = ExecutionContext::new();
        self.execute_with_context(tool, &ctx).await
    }

    pub async fn execute_with_context<T>(
        &self,
        tool: &T,
        ctx: &ExecutionContext,
    ) -> ToolResult<T::Output>
    where
        T: ToolExecutor,
    {
        // Acquire semaphore permit for concurrency control
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| ToolError::execution_failed("Failed to acquire execution permit"))?;

        let start = Instant::now();
        self.metrics
            .total_executions
            .fetch_add(1, Ordering::Relaxed);

        let result = if let Some(ref cb) = self.circuit_breaker {
            cb.call(|| self.execute_internal(tool, ctx)).await
        } else {
            self.execute_internal(tool, ctx).await
        };

        let duration = start.elapsed();
        self.metrics
            .total_duration_ms
            .fetch_add(duration.as_millis() as u64, Ordering::Relaxed);

        match &result {
            Ok(_) => {
                self.metrics
                    .successful_executions
                    .fetch_add(1, Ordering::Relaxed);
                debug!("Tool execution succeeded in {:?}", duration);
            }
            Err(e) => {
                self.metrics
                    .failed_executions
                    .fetch_add(1, Ordering::Relaxed);
                warn!("Tool execution failed: {} (duration: {:?})", e, duration);
            }
        }

        result
    }

    async fn execute_internal<T>(&self, tool: &T, ctx: &ExecutionContext) -> ToolResult<T::Output>
    where
        T: ToolExecutor,
    {
        let timeout = ctx.timeout.or(self.config.default_timeout);

        if let Some(ref retry_policy) = self.config.retry_policy {
            self.execute_with_retry(tool, ctx, retry_policy, timeout)
                .await
        } else if let Some(timeout_duration) = timeout {
            self.execute_with_timeout(tool, ctx, timeout_duration).await
        } else {
            tool.execute_tool(ctx).await
        }
    }

    async fn execute_with_timeout<T>(
        &self,
        tool: &T,
        ctx: &ExecutionContext,
        timeout: Duration,
    ) -> ToolResult<T::Output>
    where
        T: ToolExecutor,
    {
        tokio::time::timeout(timeout, tool.execute_tool(ctx))
            .await
            .map_err(|_| ToolError::Timeout(timeout))?
    }

    async fn execute_with_retry<T>(
        &self,
        tool: &T,
        ctx: &ExecutionContext,
        policy: &RetryPolicy,
        timeout: Option<Duration>,
    ) -> ToolResult<T::Output>
    where
        T: ToolExecutor,
    {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts <= policy.max_attempts {
            let result = if let Some(timeout_duration) = timeout {
                self.execute_with_timeout(tool, ctx, timeout_duration).await
            } else {
                tool.execute_tool(ctx).await
            };

            match result {
                Ok(output) => return Ok(output),
                Err(err) => {
                    attempts += 1;
                    if !policy.should_retry(&err) || attempts > policy.max_attempts {
                        return Err(err);
                    }
                    last_error = Some(err);
                    let delay = policy.calculate_backoff(attempts);
                    debug!("Retrying after {:?} (attempt {})", delay, attempts);
                    tokio::time::sleep(delay).await;
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| ToolError::execution_failed("Max retry attempts exceeded")))
    }

    /// Execute multiple tools in parallel
    pub async fn execute_batch<T>(&self, tools: Vec<T>) -> Vec<ToolResult<T::Output>>
    where
        T: ToolExecutor + Clone,
    {
        stream::iter(tools)
            .map(|tool| async move { self.execute(&tool).await })
            .buffer_unordered(self.config.max_concurrent)
            .collect()
            .await
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating configured executors
#[derive(Default)]
pub struct ExecutorBuilder {
    config: ExecutorConfig,
    circuit_breaker: Option<CircuitBreaker>,
}

impl ExecutorBuilder {
    pub fn new() -> Self {
        Self {
            config: ExecutorConfig::default(),
            circuit_breaker: None,
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.default_timeout = Some(timeout);
        self
    }

    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.config.max_concurrent = max;
        self
    }

    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.config.retry_policy = Some(policy);
        self
    }

    pub fn circuit_breaker(mut self, failure_threshold: u32, timeout: Duration) -> Self {
        self.circuit_breaker = Some(CircuitBreaker::new(failure_threshold, timeout));
        self
    }

    pub fn enable_tracing(mut self, enable: bool) -> Self {
        self.config.enable_tracing = enable;
        self
    }

    pub fn build(self) -> Executor {
        let max_concurrent = self.config.max_concurrent;
        Executor {
            config: Arc::new(self.config),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            metrics: Arc::new(ExecutorMetrics::default()),
            circuit_breaker: self.circuit_breaker.map(Arc::new),
        }
    }
}

// Add random number generation for jitter
mod rand {
    use std::cell::Cell;

    thread_local! {
        static RNG: Cell<u64> = Cell::new(0x4d595df4d0f33173);
    }

    pub fn random<T: SampleUniform>() -> T {
        T::sample_uniform()
    }

    pub trait SampleUniform: Sized {
        fn sample_uniform() -> Self;
    }

    impl SampleUniform for f64 {
        fn sample_uniform() -> Self {
            RNG.with(|rng| {
                let mut x = rng.get();
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                rng.set(x);
                (x as f64) / (u64::MAX as f64)
            })
        }
    }
}
