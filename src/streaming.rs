//! Streaming tool execution for handling large outputs.

use crate::{ExecutionContext, ToolError, ToolResult};
use async_trait::async_trait;
use futures::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Trait for tools that produce streaming output
#[async_trait]
pub trait StreamingToolExecutor: Send + Sync {
    type Item: serde::Serialize + Send;
    type Error: std::error::Error + Send + Sync + 'static;

    fn execute_stream<'a>(
        &'a self,
        ctx: &'a ExecutionContext,
    ) -> Pin<Box<dyn Stream<Item = Result<Self::Item, Self::Error>> + Send + 'a>>;
}

pin_project! {
    /// A stream wrapper that enforces resource limits
    pub struct LimitedStream<S> {
        #[pin]
        inner: S,
        max_items: Option<usize>,
        items_produced: usize,
    }
}

impl<S> LimitedStream<S> {
    pub fn new(stream: S, max_items: Option<usize>) -> Self {
        Self {
            inner: stream,
            max_items,
            items_produced: 0,
        }
    }
}

impl<S, T, E> Stream for LimitedStream<S>
where
    S: Stream<Item = Result<T, E>>,
{
    type Item = Result<T, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        if let Some(max) = this.max_items {
            if *this.items_produced >= *max {
                return Poll::Ready(None);
            }
        }

        match this.inner.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                *this.items_produced += 1;
                Poll::Ready(Some(item))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pin_project! {
    /// A stream that enforces timeout
    pub struct TimeoutStream<S> {
        #[pin]
        inner: S,
        deadline: Option<tokio::time::Instant>,
    }
}

impl<S> TimeoutStream<S> {
    pub fn new(stream: S, timeout: std::time::Duration) -> Self {
        Self {
            inner: stream,
            deadline: Some(tokio::time::Instant::now() + timeout),
        }
    }

    pub fn unlimited(stream: S) -> Self {
        Self {
            inner: stream,
            deadline: None,
        }
    }
}

impl<S, T> Stream for TimeoutStream<S>
where
    S: Stream<Item = ToolResult<T>>,
{
    type Item = ToolResult<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        if let Some(deadline) = this.deadline {
            if tokio::time::Instant::now() >= *deadline {
                return Poll::Ready(Some(Err(ToolError::Timeout(deadline.into_std().elapsed()))));
            }
        }

        this.inner.poll_next(cx)
    }
}

/// Helper to collect stream into a vector with limits
pub async fn collect_stream<S, T, E>(stream: S, max_items: Option<usize>) -> Result<Vec<T>, E>
where
    S: Stream<Item = Result<T, E>>,
{
    use futures::StreamExt;

    let limited = LimitedStream::new(stream, max_items);
    limited.collect::<Vec<_>>().await.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::{self, StreamExt};

    #[tokio::test]
    async fn test_limited_stream() {
        let data = vec![Ok::<i32, String>(1), Ok(2), Ok(3), Ok(4), Ok(5)];
        let stream = stream::iter(data);

        let limited = LimitedStream::new(stream, Some(3));
        let results: Vec<_> = limited.collect().await;

        assert_eq!(results.len(), 3);
    }
}
