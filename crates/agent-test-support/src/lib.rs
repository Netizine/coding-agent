//! Offline test doubles. Scripts are consumed once and requests are recorded.
use agent_core::{AgentError, ErrorKind, ModelRequest, ProviderCapabilities, ResponseUpdate};
use agent_runtime::{ModelProvider, RunContext, UpdateStream};
use futures_core::Stream;
use futures_util::future::BoxFuture;
use serde::de::DeserializeOwned;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

pub type Script = Vec<Result<ResponseUpdate, AgentError>>;

#[derive(Clone)]
pub struct ScriptedProvider {
    capabilities: ProviderCapabilities,
    scripts: Arc<Mutex<VecDeque<Script>>>,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
    active: Arc<AtomicUsize>,
}

impl ScriptedProvider {
    pub fn new(scripts: impl IntoIterator<Item = Script>) -> Self {
        Self {
            capabilities: ProviderCapabilities {
                streaming: true,
                tools: true,
                structured_output: true,
                ..ProviderCapabilities::default()
            },
            scripts: Arc::new(Mutex::new(scripts.into_iter().collect())),
            requests: Arc::default(),
            active: Arc::default(),
        }
    }
    pub fn with_capabilities(mut self, capabilities: ProviderCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests.lock().expect("test request lock").clone()
    }
    pub fn active_streams(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }
}

impl ModelProvider for ScriptedProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities.clone()
    }
    fn run(
        &self,
        context: RunContext,
        request: ModelRequest,
    ) -> BoxFuture<'_, Result<UpdateStream, AgentError>> {
        Box::pin(async move {
            self.capabilities.validate(&request)?;
            if context.cancellation.is_cancelled() {
                return Err(AgentError::new(ErrorKind::Cancelled, "cancelled script"));
            }
            let updates = self
                .scripts
                .lock()
                .expect("test script lock")
                .pop_front()
                .ok_or_else(|| AgentError::new(ErrorKind::Provider, "script exhausted"))?;
            self.requests
                .lock()
                .expect("test request lock")
                .push(request);
            self.active.fetch_add(1, Ordering::SeqCst);
            Ok(Box::pin(ScriptStream {
                updates: updates.into(),
                active: self.active.clone(),
            }) as UpdateStream)
        })
    }
}

struct ScriptStream {
    updates: VecDeque<Result<ResponseUpdate, AgentError>>,
    active: Arc<AtomicUsize>,
}
impl Stream for ScriptStream {
    type Item = Result<ResponseUpdate, AgentError>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.updates.pop_front())
    }
}
impl Drop for ScriptStream {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn fixture<T: DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).expect("valid checked-in fixture")
}
