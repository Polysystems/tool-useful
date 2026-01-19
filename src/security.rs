//! Security primitives: permissions, resource limits, and sandboxing.

use crate::{ToolError, ToolResult};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Permission system for tools
#[derive(Debug, Clone)]
pub struct Permissions {
    pub network: NetworkPermission,
    pub filesystem: FileSystemPermission,
    pub max_memory_bytes: Option<usize>,
    pub max_cpu_time: Option<Duration>,
    pub allowed_syscalls: Option<HashSet<String>>,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            network: NetworkPermission::Deny,
            filesystem: FileSystemPermission::Deny,
            max_memory_bytes: Some(100_000_000), // 100MB default
            max_cpu_time: Some(Duration::from_secs(30)),
            allowed_syscalls: None,
        }
    }
}

impl Permissions {
    pub fn unrestricted() -> Self {
        Self {
            network: NetworkPermission::Allow,
            filesystem: FileSystemPermission::Allow,
            max_memory_bytes: None,
            max_cpu_time: None,
            allowed_syscalls: None,
        }
    }

    pub fn builder() -> PermissionsBuilder {
        PermissionsBuilder::new()
    }

    pub fn check_network_access(&self, host: &str) -> ToolResult<()> {
        match &self.network {
            NetworkPermission::Allow => Ok(()),
            NetworkPermission::Deny => Err(ToolError::permission_denied("Network access denied")),
            NetworkPermission::AllowList(allowed) => {
                if allowed.iter().any(|pattern| matches_pattern(host, pattern)) {
                    Ok(())
                } else {
                    Err(ToolError::permission_denied(format!(
                        "Network access to {} not allowed",
                        host
                    )))
                }
            }
            NetworkPermission::DenyList(denied) => {
                if denied.iter().any(|pattern| matches_pattern(host, pattern)) {
                    Err(ToolError::permission_denied(format!(
                        "Network access to {} explicitly denied",
                        host
                    )))
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn check_file_access(&self, path: &std::path::Path) -> ToolResult<()> {
        match &self.filesystem {
            FileSystemPermission::Allow => Ok(()),
            FileSystemPermission::Deny => {
                Err(ToolError::permission_denied("Filesystem access denied"))
            }
            FileSystemPermission::ReadOnly(paths) => {
                if paths.iter().any(|allowed| path.starts_with(allowed)) {
                    Ok(())
                } else {
                    Err(ToolError::permission_denied(format!(
                        "Filesystem access to {:?} not allowed",
                        path
                    )))
                }
            }
            FileSystemPermission::AllowList(paths) => {
                if paths.iter().any(|allowed| path.starts_with(allowed)) {
                    Ok(())
                } else {
                    Err(ToolError::permission_denied(format!(
                        "Filesystem access to {:?} not allowed",
                        path
                    )))
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum NetworkPermission {
    Allow,
    Deny,
    AllowList(Vec<String>),
    DenyList(Vec<String>),
}

#[derive(Debug, Clone)]
pub enum FileSystemPermission {
    Allow,
    Deny,
    ReadOnly(Vec<std::path::PathBuf>),
    AllowList(Vec<std::path::PathBuf>),
}

pub struct PermissionsBuilder {
    permissions: Permissions,
}

impl PermissionsBuilder {
    pub fn new() -> Self {
        Self {
            permissions: Permissions::default(),
        }
    }

    pub fn allow_network(mut self) -> Self {
        self.permissions.network = NetworkPermission::Allow;
        self
    }

    pub fn allow_network_hosts(mut self, hosts: Vec<String>) -> Self {
        self.permissions.network = NetworkPermission::AllowList(hosts);
        self
    }

    pub fn deny_network(mut self) -> Self {
        self.permissions.network = NetworkPermission::Deny;
        self
    }

    pub fn allow_filesystem(mut self) -> Self {
        self.permissions.filesystem = FileSystemPermission::Allow;
        self
    }

    pub fn allow_filesystem_paths(mut self, paths: Vec<std::path::PathBuf>) -> Self {
        self.permissions.filesystem = FileSystemPermission::AllowList(paths);
        self
    }

    pub fn readonly_filesystem(mut self, paths: Vec<std::path::PathBuf>) -> Self {
        self.permissions.filesystem = FileSystemPermission::ReadOnly(paths);
        self
    }

    pub fn max_memory(mut self, bytes: usize) -> Self {
        self.permissions.max_memory_bytes = Some(bytes);
        self
    }

    pub fn max_cpu_time(mut self, duration: Duration) -> Self {
        self.permissions.max_cpu_time = Some(duration);
        self
    }

    pub fn build(self) -> Permissions {
        self.permissions
    }
}

impl Default for PermissionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource tracker for monitoring and enforcing limits
pub struct ResourceTracker {
    permissions: Arc<Permissions>,
    memory_used: AtomicUsize,
    start_time: Instant,
    cpu_time: Arc<RwLock<Duration>>,
}

impl ResourceTracker {
    pub fn new(permissions: Permissions) -> Self {
        Self {
            permissions: Arc::new(permissions),
            memory_used: AtomicUsize::new(0),
            start_time: Instant::now(),
            cpu_time: Arc::new(RwLock::new(Duration::ZERO)),
        }
    }

    pub fn track_memory_allocation(&self, bytes: usize) -> ToolResult<()> {
        let new_total = self.memory_used.fetch_add(bytes, Ordering::Relaxed) + bytes;

        if let Some(max) = self.permissions.max_memory_bytes {
            if new_total > max {
                return Err(ToolError::ResourceLimitExceeded(format!(
                    "Memory limit exceeded: {} > {}",
                    new_total, max
                )));
            }
        }

        Ok(())
    }

    pub fn track_memory_deallocation(&self, bytes: usize) {
        self.memory_used.fetch_sub(bytes, Ordering::Relaxed);
    }

    pub fn check_cpu_time(&self) -> ToolResult<()> {
        if let Some(max_time) = self.permissions.max_cpu_time {
            let elapsed = self.start_time.elapsed();
            if elapsed > max_time {
                return Err(ToolError::ResourceLimitExceeded(format!(
                    "CPU time limit exceeded: {:?} > {:?}",
                    elapsed, max_time
                )));
            }
        }
        Ok(())
    }

    pub fn memory_usage(&self) -> usize {
        self.memory_used.load(Ordering::Relaxed)
    }

    pub fn elapsed_time(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Rate limiter for API calls or tool executions
pub struct RateLimiter {
    tokens: Arc<AtomicU64>,
    max_tokens: u64,
    refill_rate: u64,
    last_refill: Arc<RwLock<Instant>>,
    refill_interval: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: u64, duration: Duration) -> Self {
        Self {
            tokens: Arc::new(AtomicU64::new(max_requests)),
            max_tokens: max_requests,
            refill_rate: max_requests,
            last_refill: Arc::new(RwLock::new(Instant::now())),
            refill_interval: duration,
        }
    }

    pub fn per_second(requests: u64) -> Self {
        Self::new(requests, Duration::from_secs(1))
    }

    pub fn per_minute(requests: u64) -> Self {
        Self::new(requests, Duration::from_secs(60))
    }

    pub async fn acquire(&self) -> ToolResult<()> {
        loop {
            self.refill();

            let current = self.tokens.load(Ordering::Relaxed);
            if current > 0 {
                if self
                    .tokens
                    .compare_exchange(current, current - 1, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    return Ok(());
                }
            } else {
                // Wait a bit before retrying
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }

    fn refill(&self) {
        let mut last_refill = self.last_refill.write();
        let elapsed = last_refill.elapsed();

        if elapsed >= self.refill_interval {
            let periods = elapsed.as_secs_f64() / self.refill_interval.as_secs_f64();
            let tokens_to_add = (self.refill_rate as f64 * periods) as u64;

            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = (current + tokens_to_add).min(self.max_tokens);
            self.tokens.store(new_tokens, Ordering::Relaxed);

            *last_refill = Instant::now();
        }
    }
}

fn matches_pattern(text: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        // Simple wildcard matching
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.is_empty() {
            return true;
        }

        let mut pos = 0;
        for (i, part) in parts.iter().enumerate() {
            if i == 0 && !part.is_empty() {
                if !text.starts_with(part) {
                    return false;
                }
                pos = part.len();
            } else if i == parts.len() - 1 && !part.is_empty() {
                if !text.ends_with(part) {
                    return false;
                }
            } else if !part.is_empty() {
                if let Some(index) = text[pos..].find(part) {
                    pos += index + part.len();
                } else {
                    return false;
                }
            }
        }
        true
    } else {
        text == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("api.example.com", "*.example.com"));
        assert!(matches_pattern("api.example.com", "api.*"));
        assert!(matches_pattern("api.example.com", "*"));
        assert!(!matches_pattern("api.example.com", "*.other.com"));
    }

    #[test]
    fn test_resource_tracker() {
        let permissions = Permissions::builder().max_memory(1000).build();

        let tracker = ResourceTracker::new(permissions);

        assert!(tracker.track_memory_allocation(500).is_ok());
        assert!(tracker.track_memory_allocation(400).is_ok());
        assert!(tracker.track_memory_allocation(200).is_err()); // Exceeds limit
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::per_second(2);

        assert!(limiter.acquire().await.is_ok());
        assert!(limiter.acquire().await.is_ok());
        // Third request should wait
    }
}
