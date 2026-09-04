//! Loopback HTTP transport and lifecycle for the MCP server.
//!
//! Ported from OpenTune's `ai_mcp_server.rs`. This module binds the socket
//! and guards it; the protocol lives in [`super::handler`] and never sees a
//! request that failed the checks here.
//!
//! ## Security invariants
//! - Binds `127.0.0.1` only — never `0.0.0.0`, so the tune is never
//!   reachable off-box even on a hostile network.
//! - Every request must carry `Authorization: Bearer <token>` matching the
//!   per-install token, compared in constant time ([`tokens_match`]) so a
//!   local attacker cannot recover it byte-by-byte from response timing.
//! - rmcp's Host check (DNS-rebinding prevention) is pinned to loopback
//!   forms explicitly rather than inherited from the library default.
//! - rmcp's Origin check is turned on explicitly: its default
//!   `allowed_origins` is an *empty* list, which means "skip Origin
//!   validation entirely". The entries are deliberately portless — rmcp
//!   only compares the port when the allow-list entry names one, and the
//!   bound port is unpredictable when the configured port is `0`.
//! - The token is never logged: no error path here names it.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Request, State as AxumState};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

use super::handler::{ExecutorFactory, LibreTuneMcp};
use super::token::load_or_create_token;

const BEARER_PREFIX: &str = "Bearer ";

/// Lowest port the settings UI accepts. Below 1024 needs elevation on unix
/// and is reserved for well-known services everywhere.
pub const MIN_MCP_PORT: u16 = 1024;

/// Constant-time token comparison. The length check up front is safe (the
/// token's *length* is not the secret); the xor-fold that follows has no
/// early return, so no timing signal reveals which byte differed.
pub(crate) fn tokens_match(candidate: &[u8], expected: &[u8]) -> bool {
    if candidate.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in candidate.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Reject anything without a matching bearer token with a bare 401 — no
/// body, so the response never echoes what was sent or hints at why.
async fn require_bearer_token(
    AxumState(expected): AxumState<Arc<str>>,
    request: Request,
    next: Next,
) -> Response {
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(BEARER_PREFIX));

    match provided {
        Some(candidate) if tokens_match(candidate.as_bytes(), expected.as_bytes()) => {
            next.run(request).await
        }
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// The single-route (`/mcp`) router: rmcp's streamable-HTTP service behind
/// the bearer layer.
pub(crate) fn build_router(executor: ExecutorFactory, token: String) -> Router {
    let service = StreamableHttpService::new(
        move || Ok(LibreTuneMcp::new(Arc::clone(&executor))),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_allowed_hosts(["localhost", "127.0.0.1", "::1"])
            .with_allowed_origins(["http://localhost", "http://127.0.0.1", "http://[::1]"]),
    );

    Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            Arc::<str>::from(token),
            require_bearer_token,
        ))
}

/// One running server's handles.
struct RunningServer {
    shutdown_tx: oneshot::Sender<()>,
    join_handle: JoinHandle<()>,
    local_addr: SocketAddr,
}

/// Managed app-wide: the current server, or `None` when stopped.
#[derive(Default)]
pub struct McpServerState(Mutex<Option<RunningServer>>);

impl McpServerState {
    /// The bound address while running. Mutex poisoning is recovered from
    /// rather than propagated — a panic elsewhere should not permanently
    /// wedge the Settings UI's status query.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|running| running.local_addr)
    }
}

/// Bind loopback and start serving. `port == 0` asks the OS for a free
/// port; read the real one back via [`McpServerState::local_addr`] (that is
/// how the tests avoid a fixed port and a sleep).
///
/// Returns `Err` without side effects when a server is already running —
/// callers that mean to restart call [`stop_mcp_server`] first.
pub async fn start_mcp_server(
    state: &McpServerState,
    executor: ExecutorFactory,
    token: String,
    port: u16,
) -> Result<(), String> {
    if state.local_addr().is_some() {
        return Err("The MCP server is already running — stop it first".into());
    }

    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("Could not start the MCP server on 127.0.0.1:{port}: {e}"))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| format!("Could not read the MCP server's bound address: {e}"))?;

    let router = build_router(executor, token);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let join_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let mut guard = lock_state(state);
    *guard = Some(RunningServer {
        shutdown_tx,
        join_handle,
        local_addr,
    });
    Ok(())
}

fn lock_state(state: &McpServerState) -> std::sync::MutexGuard<'_, Option<RunningServer>> {
    state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Stop the running server. Idempotent.
///
/// Signal-then-abort: the oneshot is best-effort (it only lands if the task
/// is still polling `with_graceful_shutdown`), the abort is what guarantees
/// the listener closes. Awaiting the aborted handle is what makes
/// "stop, then start on the same port" deterministic — by the time this
/// returns, the socket is released.
pub async fn stop_mcp_server(state: &McpServerState) {
    let running = lock_state(state).take();
    let Some(running) = running else {
        return;
    };
    let _ = running.shutdown_tx.send(());
    running.join_handle.abort();
    let _ = running.join_handle.await;
}

/// What reconciling desired settings against reality requires. Extracted so
/// the decision matrix is unit-testable without a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReconcileAction {
    NoOp,
    Start,
    Stop,
    Restart,
}

pub(crate) fn reconcile_action(
    desired_enabled: bool,
    desired_port: u16,
    running_port: Option<u16>,
) -> ReconcileAction {
    match (desired_enabled, running_port) {
        (false, None) => ReconcileAction::NoOp,
        (false, Some(_)) => ReconcileAction::Stop,
        (true, None) => ReconcileAction::Start,
        (true, Some(port)) if port == desired_port => ReconcileAction::NoOp,
        (true, Some(_)) => ReconcileAction::Restart,
    }
}

/// Bring the server in line with the saved settings — covers enable,
/// disable, and a port change while enabled. Loads or creates the token
/// itself, and only on the paths that actually need one.
pub async fn reconcile_mcp_server(
    state: &McpServerState,
    executor: ExecutorFactory,
    config_dir: PathBuf,
    desired_enabled: bool,
    desired_port: u16,
) -> Result<(), String> {
    let action = reconcile_action(
        desired_enabled,
        desired_port,
        state.local_addr().map(|addr| addr.port()),
    );

    if matches!(action, ReconcileAction::Stop | ReconcileAction::Restart) {
        stop_mcp_server(state).await;
    }
    if matches!(action, ReconcileAction::Start | ReconcileAction::Restart) {
        let token = load_or_create_token(&config_dir)?;
        start_mcp_server(state, executor, token, desired_port).await?;
    }
    Ok(())
}

/// Mint a fresh token and, if a server is running, restart it on the *same*
/// port so the new token takes effect at once.
///
/// Without the restart, "Regenerate" would be actively harmful: the running
/// server would keep honouring the old (possibly leaked) token while the
/// token shown in Settings 401s. When nothing is running this stays a pure
/// token swap — regenerating must not start a server the user never enabled.
pub async fn regenerate_and_restart(
    state: &McpServerState,
    executor: ExecutorFactory,
    config_dir: PathBuf,
) -> Result<String, String> {
    let token = super::token::regenerate_token(&config_dir)?;
    if let Some(port) = state.local_addr().map(|addr| addr.port()) {
        stop_mcp_server(state).await;
        start_mcp_server(state, executor, token.clone(), port).await?;
    }
    Ok(token)
}
