//! Shared HTTP behaviour for every provider call.
//!
//! A dropped connection is the most common way a chat turn dies, and each
//! adapter used to build its own client with no timeouts and no retry: the
//! first transport hiccup ended the turn and threw the prompt away.

use anyhow::Result;
use reqwest::{Client, StatusCode};
use std::time::Duration;

/// How many times a transient failure is worth repeating before giving up.
pub const MAX_ATTEMPTS: u32 = 3;

/// A client with real timeouts.
///
/// There is deliberately no overall request timeout: a reasoning model can
/// legitimately take minutes. What is bounded is the part that hangs when a
/// network goes away — connecting, and an idle socket the far end has
/// silently dropped.
pub fn client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .tcp_keepalive(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(Into::into)
}

/// Transport failures worth repeating. A rejected key or a malformed request
/// will fail the same way every time, so those are not retried.
pub fn is_transient(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || error.is_request()
}

/// Provider back-pressure and provider-side faults are worth repeating.
pub fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// Wait before the next attempt, and say so — a silent multi-second pause
/// looks like a hang.
pub async fn backoff(label: &str, attempt: u32, reason: &str) {
    let wait = Duration::from_millis(600 * 2_u64.pow(attempt.saturating_sub(1)));
    eprintln!(
        "\r\x1b[2K\x1b[2m  ↻ {} failed ({}); retrying in {:.0}s [{}/{}]\x1b[0m",
        label,
        reason,
        wait.as_secs_f32(),
        attempt,
        MAX_ATTEMPTS
    );
    tokio::time::sleep(wait).await;
}
