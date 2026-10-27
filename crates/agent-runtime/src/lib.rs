//! Provider contracts and cancellation ownership. The Agent/tool loop arrives in P2.
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use agent_core::{
    AgentError, ErrorKind, ModelRequest, ProviderCapabilities, Response, ResponseUpdate,
    ToolDescriptor,
};
use futures_core::Stream;
use futures_util::{StreamExt, future::BoxFuture};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

pub mod state;
pub mod tasks;

pub type UpdateStream =
    Pin<Box<dyn Stream<Item = Result<ResponseUpdate, AgentError>> + Send + 'static>>;

#[derive(Clone, Debug, Default)]
pub struct RunContext {
    pub cancellation: CancellationToken,
    pub deadline: Option<tokio::time::Instant>,
}

impl RunContext {
    pub fn child(&self) -> Self {
        Self {
            cancellation: self.cancellation.child_token(),
            deadline: self.deadline,
        }
    }

    pub async fn cancelled(&self) -> AgentError {
        match self.deadline {
            Some(deadline) => tokio::select! {
                biased;
                _ = self.cancellation.cancelled() => AgentError::new(ErrorKind::Cancelled, "run cancelled"),
                _ = tokio::time::sleep_until(deadline) => AgentError::new(ErrorKind::Timeout, "run deadline exceeded"),
            },
            None => {
                self.cancellation.cancelled().await;
                AgentError::new(ErrorKind::Cancelled, "run cancelled")
            }
        }
    }
}

pub trait ModelProvider: Send + Sync {
    fn capabilities(&self) -> ProviderCapabilities;
    /// The returned stream must own its client/request state, not borrow `self`.
    fn run(
        &self,
        context: RunContext,
        request: ModelRequest,
    ) -> BoxFuture<'_, Result<UpdateStream, AgentError>>;
}

pub trait FunctionTool: Send + Sync {
    fn descriptor(&self) -> &ToolDescriptor;
    fn call(
        &self,
        context: RunContext,
        arguments: Value,
    ) -> BoxFuture<'_, Result<Value, AgentError>>;
}

/// Provider-call middleware; agent and tool middleware are separate P2 contracts.
pub trait ProviderMiddleware: Send + Sync {
    fn run<'a>(
        &'a self,
        next: &'a dyn ModelProvider,
        context: RunContext,
        request: ModelRequest,
    ) -> BoxFuture<'a, Result<UpdateStream, AgentError>>;
}

struct WrappedProvider {
    next: Arc<dyn ModelProvider>,
    middleware: Arc<dyn ProviderMiddleware>,
}

impl ModelProvider for WrappedProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        self.next.capabilities()
    }
    fn run(
        &self,
        context: RunContext,
        request: ModelRequest,
    ) -> BoxFuture<'_, Result<UpdateStream, AgentError>> {
        self.middleware.run(self.next.as_ref(), context, request)
    }
}

/// The first registered middleware is outermost.
pub fn with_middleware(
    mut provider: Arc<dyn ModelProvider>,
    middleware: Vec<Arc<dyn ProviderMiddleware>>,
) -> Arc<dyn ModelProvider> {
    for layer in middleware.into_iter().rev() {
        provider = Arc::new(WrappedProvider {
            next: provider,
            middleware: layer,
        });
    }
    provider
}

/// Starts one provider call with capability checks and an independently owned cancellation scope.
pub async fn invoke(
    provider: &dyn ModelProvider,
    parent: &RunContext,
    request: ModelRequest,
) -> Result<UpdateStream, AgentError> {
    provider.capabilities().validate(&request)?;
    let context = parent.child();
    // Also cancels initialization work if this future is dropped before a stream exists.
    let guard = context.cancellation.clone().drop_guard();
    let stream = tokio::select! {
        biased;
        error = context.cancelled() => return Err(error),
        result = provider.run(context.clone(), request) => result?,
    };
    let cancel_context = context.clone();
    let guarded = CancelStream {
        inner: Some(stream),
        cancellation: context.cancellation,
        cancelled: Box::pin(async move { cancel_context.cancelled().await }),
        done: false,
    };
    guard.disarm();
    Ok(Box::pin(guarded))
}

struct CancelStream {
    inner: Option<UpdateStream>,
    cancellation: CancellationToken,
    cancelled: Pin<Box<dyn Future<Output = AgentError> + Send>>,
    done: bool,
}

impl Stream for CancelStream {
    type Item = Result<ResponseUpdate, AgentError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }
        if let Poll::Ready(error) = self.cancelled.as_mut().poll(cx) {
            self.done = true;
            self.inner = None;
            self.cancellation.cancel();
            return Poll::Ready(Some(Err(error)));
        }
        let result = self
            .inner
            .as_mut()
            .expect("active stream")
            .as_mut()
            .poll_next(cx);
        if matches!(result, Poll::Ready(None | Some(Err(_)))) {
            self.done = true;
            self.inner = None;
            self.cancellation.cancel();
        }
        result
    }
}

impl Drop for CancelStream {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

/// Collect matches Go's failure policy: an error returns no successful partial response.
/// Consume updates directly when partial output needs to be retained.
pub async fn collect(mut stream: UpdateStream) -> Result<Response, AgentError> {
    let mut response = Response::default();
    while let Some(update) = stream.next().await {
        response.update(update?);
    }
    response.coalesce();
    Ok(response)
}
