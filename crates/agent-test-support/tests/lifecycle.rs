use agent_core::{
    AgentError, ErrorKind, ModelRequest, ProviderCapabilities, ResponseUpdate, ToolDescriptor,
    tool::Schema,
};
use agent_runtime::{
    FunctionTool, ModelProvider, ProviderMiddleware, RunContext, UpdateStream, collect, invoke,
    state::{CodecRegistry, StateEnvelope},
    tasks::RunTasks,
    with_middleware,
};
use agent_test_support::ScriptedProvider;
use futures_util::{StreamExt, future::BoxFuture};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    process::Command,
    sync::{Arc, Mutex},
};

#[tokio::test]
async fn scripted_provider_stream_is_owned_and_collectable() {
    let stream = {
        let provider = ScriptedProvider::new([vec![
            Ok(ResponseUpdate::text("a")),
            Ok(ResponseUpdate::text("b")),
        ]]);
        invoke(&provider, &RunContext::default(), ModelRequest::default())
            .await
            .unwrap()
    };
    assert_eq!(collect(stream).await.unwrap().text(), "ab");
}

#[tokio::test]
async fn drop_and_failure_release_stream_and_do_not_cancel_parent() {
    let provider = ScriptedProvider::new([
        vec![Ok(ResponseUpdate::text("a"))],
        vec![
            Ok(ResponseUpdate::text("partial")),
            Err(AgentError::new(ErrorKind::Provider, "script error")),
            Ok(ResponseUpdate::text("must not emit")),
        ],
    ]);
    let parent = RunContext::default();
    let stream = invoke(&provider, &parent, ModelRequest::default())
        .await
        .unwrap();
    assert_eq!(provider.active_streams(), 1);
    drop(stream);
    assert_eq!(provider.active_streams(), 0);
    assert!(!parent.cancellation.is_cancelled());
    assert!(
        collect(
            invoke(&provider, &parent, ModelRequest::default())
                .await
                .unwrap()
        )
        .await
        .is_err()
    );
    assert_eq!(provider.active_streams(), 0);
}

#[tokio::test]
async fn errors_are_terminal_and_requests_recorded() {
    let provider = ScriptedProvider::new([vec![
        Err(AgentError::new(ErrorKind::Provider, "failed")),
        Ok(ResponseUpdate::text("bad")),
    ]]);
    let mut stream = invoke(&provider, &RunContext::default(), ModelRequest::default())
        .await
        .unwrap();
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    assert_eq!(provider.requests().len(), 1);
    assert_eq!(provider.active_streams(), 0);
}

struct Pending;
impl ModelProvider for Pending {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }
    fn run(
        &self,
        _: RunContext,
        _: ModelRequest,
    ) -> BoxFuture<'_, Result<UpdateStream, AgentError>> {
        Box::pin(async { Ok(Box::pin(futures_util::stream::pending()) as UpdateStream) })
    }
}

#[tokio::test]
async fn cancellation_wakes_a_pending_consumer() {
    let parent = RunContext::default();
    let mut stream = invoke(&Pending, &parent, ModelRequest::default())
        .await
        .unwrap();
    parent.cancellation.cancel();
    assert_eq!(
        stream.next().await.unwrap().unwrap_err().kind,
        ErrorKind::Cancelled
    );
    assert!(stream.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn deadline_wakes_a_pending_consumer() {
    let parent = RunContext {
        deadline: Some(tokio::time::Instant::now() + std::time::Duration::from_secs(10)),
        ..RunContext::default()
    };
    let mut stream = invoke(&Pending, &parent, ModelRequest::default())
        .await
        .unwrap();
    assert_eq!(
        stream.next().await.unwrap().unwrap_err().kind,
        ErrorKind::Timeout
    );
}

#[tokio::test]
async fn pre_cancelled_run_never_reaches_provider() {
    let provider = ScriptedProvider::new([]);
    let parent = RunContext::default();
    parent.cancellation.cancel();
    assert!(
        invoke(&provider, &parent, ModelRequest::default())
            .await
            .is_err()
    );
    assert!(provider.requests().is_empty());
}

struct Initializing {
    seen: Arc<Mutex<Option<RunContext>>>,
}
impl ModelProvider for Initializing {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }
    fn run(
        &self,
        context: RunContext,
        _: ModelRequest,
    ) -> BoxFuture<'_, Result<UpdateStream, AgentError>> {
        *self.seen.lock().unwrap() = Some(context);
        Box::pin(std::future::pending())
    }
}

#[tokio::test]
async fn abandoned_initialization_cancels_its_child_scope() {
    let seen = Arc::new(Mutex::new(None));
    let provider = Initializing { seen: seen.clone() };
    let parent = RunContext::default();
    let mut run = Box::pin(invoke(&provider, &parent, ModelRequest::default()));
    assert!(futures_util::poll!(run.as_mut()).is_pending());
    drop(run);
    assert!(
        seen.lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .cancellation
            .is_cancelled()
    );
    assert!(!parent.cancellation.is_cancelled());
}

#[tokio::test]
async fn cancelling_one_stream_leaves_sibling_running() {
    let provider = ScriptedProvider::new([
        vec![Ok(ResponseUpdate::text("one"))],
        vec![Ok(ResponseUpdate::text("two"))],
    ]);
    let parent = RunContext::default();
    let first = invoke(&provider, &parent, ModelRequest::default())
        .await
        .unwrap();
    let second = invoke(&provider, &parent, ModelRequest::default())
        .await
        .unwrap();
    drop(first);
    assert_eq!(provider.active_streams(), 1);
    assert_eq!(collect(second).await.unwrap().text(), "two");
    assert_eq!(provider.active_streams(), 0);
}

struct Order {
    name: &'static str,
    events: Arc<Mutex<Vec<String>>>,
}
impl ProviderMiddleware for Order {
    fn run<'a>(
        &'a self,
        next: &'a dyn ModelProvider,
        context: RunContext,
        request: ModelRequest,
    ) -> BoxFuture<'a, Result<UpdateStream, AgentError>> {
        Box::pin(async move {
            self.events
                .lock()
                .unwrap()
                .push(format!("{}:before", self.name));
            let result = next.run(context, request).await;
            self.events
                .lock()
                .unwrap()
                .push(format!("{}:after", self.name));
            result
        })
    }
}

#[tokio::test]
async fn middleware_first_registered_is_outermost() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let provider = with_middleware(
        Arc::new(ScriptedProvider::new([vec![]])),
        vec![
            Arc::new(Order {
                name: "a",
                events: events.clone(),
            }),
            Arc::new(Order {
                name: "b",
                events: events.clone(),
            }),
        ],
    );
    collect(
        invoke(
            provider.as_ref(),
            &RunContext::default(),
            ModelRequest::default(),
        )
        .await
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        ["a:before", "b:before", "b:after", "a:after"]
    );
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EchoInput {
    text: String,
}
struct Echo {
    descriptor: ToolDescriptor,
    schema: Schema,
}
impl FunctionTool for Echo {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }
    fn call(&self, _: RunContext, args: Value) -> BoxFuture<'_, Result<Value, AgentError>> {
        Box::pin(async move {
            let input: EchoInput = self.schema.decode(args)?;
            Ok(json!({"text":input.text}))
        })
    }
}

#[tokio::test]
async fn dynamic_tool_schema_spike() {
    let tool: Arc<dyn FunctionTool> = Arc::new(Echo {
        descriptor: ToolDescriptor::typed::<EchoInput>("echo", "echo a string").unwrap(),
        schema: Schema::for_type::<EchoInput>().unwrap(),
    });
    assert_eq!(
        tool.call(RunContext::default(), json!({"text":"hello"}))
            .await
            .unwrap(),
        json!({"text":"hello"})
    );
    assert!(
        tool.call(RunContext::default(), json!({"text":"hello","extra":true}))
            .await
            .is_err()
    );
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Counter {
    count: u64,
}

#[test]
fn fresh_process_state_restore() {
    let envelope = StateEnvelope::encode("example.counter", 1, &Counter { count: 7 }).unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "persistence_child", "--nocapture"])
        .env("AF_STATE_SPIKE", serde_json::to_string(&envelope).unwrap())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("restored-counter-7"));
}

#[test]
fn persistence_child() {
    let Ok(value) = std::env::var("AF_STATE_SPIKE") else {
        return;
    };
    let envelope: StateEnvelope = serde_json::from_str(&value).unwrap();
    let mut registry = CodecRegistry::default();
    registry.register::<Counter>("example.counter", 1).unwrap();
    assert_eq!(
        registry.decode::<Counter>(&envelope).unwrap(),
        Counter { count: 7 }
    );
    println!("restored-counter-7");
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct QueuedState {
    queue: Vec<Value>,
    pending_requests: std::collections::BTreeMap<String, Value>,
}

#[test]
fn registry_rejects_wrong_types_versions_and_duplicate_registration() {
    let mut registry = CodecRegistry::default();
    registry.register::<Counter>("counter", 1).unwrap();
    assert!(registry.register::<Counter>("counter", 1).is_err());
    assert!(registry.register::<Counter>("", 1).is_err());
    assert!(registry.register::<Counter>("counter", 0).is_err());
    let mut envelope = StateEnvelope::encode("counter", 1, &Counter { count: 7 }).unwrap();
    assert!(registry.decode::<String>(&envelope).is_err());
    envelope.schema_version = 2;
    assert!(registry.decode::<Counter>(&envelope).is_err());
    envelope.schema_version = 1;
    envelope.value = json!({"count":"bad"});
    assert_eq!(
        registry.decode::<Counter>(&envelope).unwrap_err().kind,
        ErrorKind::CorruptState
    );
}

#[test]
fn queued_checkpoint_spike_restores_in_fresh_process() {
    let state = QueuedState {
        queue: vec![json!({"port":"input","value":42})],
        pending_requests: [("approval-1".into(), json!({"tool":"echo"}))].into(),
    };
    let envelope = StateEnvelope::encode("spike.queued-state", 1, &state).unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "queued_checkpoint_child", "--nocapture"])
        .env("AF_QUEUED_SPIKE", serde_json::to_string(&envelope).unwrap())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("restored-queue-and-request"));
}

#[test]
fn queued_checkpoint_child() {
    let Ok(value) = std::env::var("AF_QUEUED_SPIKE") else {
        return;
    };
    let envelope: StateEnvelope = serde_json::from_str(&value).unwrap();
    let mut registry = CodecRegistry::default();
    registry
        .register::<QueuedState>("spike.queued-state", 1)
        .unwrap();
    let restored: QueuedState = registry.decode(&envelope).unwrap();
    assert_eq!(restored.queue, vec![json!({"port":"input","value":42})]);
    assert_eq!(
        restored.pending_requests["approval-1"],
        json!({"tool":"echo"})
    );
    assert_eq!(registry.decode::<QueuedState>(&envelope).unwrap(), restored);
    println!("restored-queue-and-request");
}

#[tokio::test]
async fn task_shutdown_joins_cleanup_without_cancelling_parent() {
    let parent = RunContext::default();
    let cleaned = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = cleaned.clone();
    let mut tasks = RunTasks::new(&parent.cancellation);
    tasks.spawn(move |token| async move {
        token.cancelled().await;
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    tasks
        .shutdown(std::time::Duration::from_secs(1))
        .await
        .unwrap();
    assert!(cleaned.load(std::sync::atomic::Ordering::SeqCst));
    assert!(!parent.cancellation.is_cancelled());
}

struct DropSignal(tokio::sync::oneshot::Sender<()>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        let (replacement, _) = tokio::sync::oneshot::channel();
        let sender = std::mem::replace(&mut self.0, replacement);
        let _ = sender.send(());
    }
}

#[tokio::test(start_paused = true)]
async fn task_timeout_aborts_and_reaps_uncooperative_work() {
    let parent = RunContext::default();
    let mut tasks = RunTasks::new(&parent.cancellation);
    let (tx, rx) = tokio::sync::oneshot::channel();
    let guard = DropSignal(tx);
    tasks.spawn(move |_| async move {
        let _guard = guard;
        std::future::pending::<()>().await
    });
    assert_eq!(
        tasks
            .shutdown(std::time::Duration::from_secs(1))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Timeout
    );
    rx.await.unwrap();
    assert!(!parent.cancellation.is_cancelled());
}

#[tokio::test]
async fn abandoned_shutdown_aborts_owned_work() {
    let parent = RunContext::default();
    let mut tasks = RunTasks::new(&parent.cancellation);
    let (tx, rx) = tokio::sync::oneshot::channel();
    let guard = DropSignal(tx);
    tasks.spawn(move |_| async move {
        let _guard = guard;
        std::future::pending::<()>().await
    });
    let mut shutdown = Box::pin(tasks.shutdown(std::time::Duration::from_secs(60)));
    assert!(futures_util::poll!(shutdown.as_mut()).is_pending());
    drop(shutdown);
    tokio::time::timeout(std::time::Duration::from_secs(1), rx)
        .await
        .unwrap()
        .unwrap();
    assert!(!parent.cancellation.is_cancelled());
}

#[test]
fn incompatible_or_corrupt_state_is_rejected() {
    let mut envelope = StateEnvelope::encode("example.counter", 1, &Counter { count: 7 }).unwrap();
    assert_eq!(
        envelope.decode::<Counter>("other", 1).unwrap_err().kind,
        ErrorKind::IncompatibleSchema
    );
    assert!(envelope.decode::<Counter>("example.counter", 2).is_err());
    envelope.value = json!({"count":"invalid"});
    assert_eq!(
        envelope
            .decode::<Counter>("example.counter", 1)
            .unwrap_err()
            .kind,
        ErrorKind::CorruptState
    );
}
