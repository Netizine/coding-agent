//! Owned background-task spike: bounded shutdown, no detached handles.
use agent_core::{AgentError, ErrorKind};
use std::{future::Future, time::Duration};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct RunTasks {
    token: CancellationToken,
    handles: Vec<JoinHandle<()>>,
}
impl RunTasks {
    pub fn new(parent: &CancellationToken) -> Self {
        Self {
            token: parent.child_token(),
            handles: Vec::new(),
        }
    }
    pub fn spawn<F, Fut>(&mut self, work: F)
    where
        F: FnOnce(CancellationToken) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.handles.push(tokio::spawn(work(self.token.clone())));
    }
    /// Cooperative cleanup first; timed-out workers are aborted and reaped.
    pub async fn shutdown(mut self, grace: Duration) -> Result<(), AgentError> {
        self.token.cancel();
        let deadline = tokio::time::Instant::now() + grace;
        let mut failure = None;
        // Handles remain owned by self while awaiting, including if shutdown is dropped.
        for handle in &mut self.handles {
            match tokio::time::timeout_at(deadline, &mut *handle).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => {
                    failure = Some(AgentError::new(
                        ErrorKind::Provider,
                        "background task failed",
                    ))
                }
                Err(_) => {
                    handle.abort();
                    let _ = handle.await;
                    failure = Some(AgentError::new(
                        ErrorKind::Timeout,
                        "background shutdown timed out",
                    ));
                }
            }
        }
        self.handles.clear();
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
impl Drop for RunTasks {
    fn drop(&mut self) {
        self.token.cancel();
        for h in &self.handles {
            h.abort();
        }
    }
}
