//! Cancellable HTTP I/O behind the synchronous model provider interface.

use crate::agent::AgentContext;
use cortex_core::{CortexError, Result};
use std::future::Future;
use std::time::Duration;

pub(super) fn run<T>(
    context: &AgentContext,
    operation: impl Future<Output = Result<T>>,
) -> Result<T> {
    if context.is_cancelled() {
        return Err(cancelled());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CortexError::Internal(format!("failed to initialize model transport: {e}")))?;
    // The operation owns its response. Dropping it and the runtime closes pending
    // network I/O rather than leaving a detached HTTP worker behind.
    let result = runtime.block_on(async {
        tokio::pin!(operation);
        loop {
            tokio::select! {
                biased;
                _ = tokio::time::sleep(Duration::from_millis(25)) => {
                    if context.is_cancelled() {
                        return Err(cancelled());
                    }
                }
                result = &mut operation => {
                    return if context.is_cancelled() { Err(cancelled()) } else { result };
                }
            }
        }
    });
    // DNS resolution may use Tokio's blocking pool. Do not wait for an OS
    // resolver during cancellation; no HTTP future survives this point.
    runtime.shutdown_background();
    result
}

fn cancelled() -> CortexError {
    CortexError::Cancelled("model request cancelled by request".to_string())
}

pub(super) fn client(timeout_secs: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|e| CortexError::Internal(format!("failed to build model HTTP client: {e}")))
}

pub(super) async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let response = request.send().await.map_err(http_error)?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.map_err(http_error)?;
        return Err(CortexError::Internal(format!(
            "model API returned {status}: {body}"
        )));
    }
    Ok(response)
}

pub(super) fn http_error(error: reqwest::Error) -> CortexError {
    CortexError::Internal(format!("model HTTP request failed: {error}"))
}
