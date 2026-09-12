//! Tests for the local MCP server.
//!
//! Everything here runs without a `tauri::App`: the handler is built over a
//! stub [`ReadToolExecutor`], which is exactly why the production handler
//! takes a factory instead of an `AppHandle`.

use std::sync::Arc;

use libretune_core::agent::orchestrator::ReadToolExecutor;
use libretune_core::agent::tools;
use serde_json::json;

use super::handler::{ExecutorFactory, LibreTuneMcp};
use super::server::{
    build_router, reconcile_action, start_mcp_server, stop_mcp_server, tokens_match,
    McpServerState, ReconcileAction,
};
use super::token::{load_or_create_token, regenerate_token, MCP_TOKEN_FILE};

// ---------------------------------------------------------------- stubs

/// Answers every read tool with a fixed payload, echoing back what it was
/// asked so dispatch's argument plumbing is observable.
struct StubExecutor {
    /// When set, every call returns this raw string instead of the echo.
    canned: Option<String>,
}

#[async_trait::async_trait]
impl ReadToolExecutor for StubExecutor {
    fn handles(&self, tool_name: &str) -> bool {
        tools::is_read_tool(tool_name)
    }

    async fn execute(&self, tool_name: &str, arguments: &str) -> String {
        if let Some(canned) = &self.canned {
            return canned.clone();
        }
        json!({ "tool": tool_name, "arguments": arguments }).to_string()
    }
}

fn stub_factory(canned: Option<String>) -> ExecutorFactory {
    Arc::new(move || {
        Arc::new(StubExecutor {
            canned: canned.clone(),
        }) as Arc<dyn ReadToolExecutor>
    })
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("libretune-mcp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir created");
    dir
}

// ---------------------------------------------------------------- token

#[test]
fn token_is_created_once_and_reused() {
    let dir = temp_dir("token-reuse");
    let first = load_or_create_token(&dir).expect("token minted");
    assert_eq!(first.len(), 64, "32 random bytes, hex encoded");
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(first, load_or_create_token(&dir).expect("token reread"));
}

#[test]
fn blank_token_file_is_replaced_not_trusted() {
    // A truncated or hand-emptied file must not become an empty bearer
    // token — that would authenticate `Authorization: Bearer `.
    let dir = temp_dir("token-blank");
    std::fs::write(dir.join(MCP_TOKEN_FILE), "   \n").expect("blank file written");
    let token = load_or_create_token(&dir).expect("token minted");
    assert_eq!(token.len(), 64);
}

#[test]
fn regenerate_invalidates_the_previous_token() {
    let dir = temp_dir("token-regen");
    let first = load_or_create_token(&dir).expect("token minted");
    let second = regenerate_token(&dir).expect("token regenerated");
    assert_ne!(first, second);
    assert_eq!(second, load_or_create_token(&dir).expect("token reread"));
}

#[cfg(unix)]
#[test]
fn token_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("token-perms");
    load_or_create_token(&dir).expect("token minted");
    let mode = std::fs::metadata(dir.join(MCP_TOKEN_FILE))
        .expect("token file exists")
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "no other local user may read the token"
    );
}

// ------------------------------------------------------------ auth check

#[test]
fn tokens_match_only_on_exact_equality() {
    assert!(tokens_match(b"abc123", b"abc123"));
    assert!(!tokens_match(b"abc123", b"abc124"));
    assert!(
        !tokens_match(b"abc12", b"abc123"),
        "length mismatch rejected"
    );
    assert!(!tokens_match(b"", b"abc123"));
}

// -------------------------------------------------------------- handler

#[tokio::test]
async fn only_read_tools_are_advertised() {
    let mcp = LibreTuneMcp::new(stub_factory(None));
    let names: Vec<String> = mcp
        .read_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();

    assert_eq!(names.len(), 8, "the eight read tools, no more: {names:?}");
    for name in &names {
        assert!(tools::is_read_tool(name), "{name} is not a read tool");
    }
    for withheld in [
        tools::tool_names::PROPOSE_TABLE_EDIT,
        tools::tool_names::PROPOSE_BULK_OP,
        tools::tool_names::PROPOSE_CONSTANT_CHANGE,
    ] {
        assert!(
            !names.iter().any(|n| n == withheld),
            "{withheld} must never reach an external agent"
        );
    }
}

#[tokio::test]
async fn every_advertised_tool_has_an_object_schema() {
    // rmcp serializes `input_schema` straight to the wire; a non-object
    // schema would make the tool uncallable for a conforming client.
    let mcp = LibreTuneMcp::new(stub_factory(None));
    for tool in mcp.read_tools() {
        assert_eq!(
            tool.input_schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "{} has a non-object schema",
            tool.name
        );
    }
}

#[tokio::test]
async fn dispatch_forwards_arguments_and_returns_structured_content() {
    let mcp = LibreTuneMcp::new(stub_factory(None));
    let mut args = serde_json::Map::new();
    args.insert("table_name".into(), json!("veTable1"));

    let result = mcp
        .dispatch(tools::tool_names::READ_TABLE, Some(args))
        .await;

    assert_eq!(result.is_error, Some(false));
    let structured = result.structured_content.expect("structured content");
    assert_eq!(structured["tool"], json!(tools::tool_names::READ_TABLE));
    assert!(
        structured["arguments"]
            .as_str()
            .expect("arguments echoed as a JSON string")
            .contains("veTable1"),
        "arguments were not forwarded: {structured}"
    );
}

#[tokio::test]
async fn unknown_tool_is_a_tool_level_error_not_a_protocol_fault() {
    let mcp = LibreTuneMcp::new(stub_factory(None));
    let result = mcp.dispatch("burn_now", None).await;
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn propose_tools_are_refused_even_when_named_directly() {
    // Withholding them from `tools/list` is not enough on its own: a client
    // can call any name it likes.
    let mcp = LibreTuneMcp::new(stub_factory(None));
    let result = mcp
        .dispatch(tools::tool_names::PROPOSE_TABLE_EDIT, None)
        .await;
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn executor_error_object_becomes_an_mcp_error() {
    // The executor reports failure in-band as {"error": ...} because that is
    // what an LLM tool-result wants; MCP has a real error channel, and an
    // agent reading only `isError` must not mistake this for data.
    let canned = json!({ "error": "No INI definition loaded" }).to_string();
    let mcp = LibreTuneMcp::new(stub_factory(Some(canned)));

    let result = mcp.dispatch(tools::tool_names::LIST_TABLES, None).await;

    assert_eq!(result.is_error, Some(true));
    assert!(result.structured_content.is_none());
}

// ------------------------------------------------------------- lifecycle

#[test]
fn reconcile_matrix_covers_every_transition() {
    use ReconcileAction::*;
    assert_eq!(reconcile_action(false, 8765, None), NoOp);
    assert_eq!(reconcile_action(false, 8765, Some(8765)), Stop);
    assert_eq!(reconcile_action(true, 8765, None), Start);
    assert_eq!(reconcile_action(true, 8765, Some(8765)), NoOp);
    assert_eq!(reconcile_action(true, 9000, Some(8765)), Restart);
}

#[tokio::test]
async fn stop_is_idempotent() {
    let state = McpServerState::default();
    stop_mcp_server(&state).await;
    assert!(state.local_addr().is_none());
}

#[tokio::test]
async fn starting_twice_is_refused_without_disturbing_the_running_server() {
    let state = McpServerState::default();
    start_mcp_server(&state, stub_factory(None), "tok".into(), 0)
        .await
        .expect("first start binds");
    let addr = state.local_addr().expect("bound");

    let second = start_mcp_server(&state, stub_factory(None), "tok".into(), 0).await;

    assert!(second.is_err());
    assert_eq!(state.local_addr(), Some(addr), "original server untouched");
    stop_mcp_server(&state).await;
}

// ------------------------------------------------------------------ HTTP

/// An initialize request — the first thing any MCP client sends.
fn initialize_body() -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "libretune-test", "version": "0" }
        }
    })
}

async fn serve_on_ephemeral_port(token: &str) -> (McpServerState, String) {
    let state = McpServerState::default();
    start_mcp_server(&state, stub_factory(None), token.to_string(), 0)
        .await
        .expect("server binds an ephemeral loopback port");
    let addr = state.local_addr().expect("bound address");
    let url = format!("http://127.0.0.1:{}/mcp", addr.port());
    (state, url)
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

#[tokio::test]
async fn server_binds_loopback_only() {
    let (state, _url) = serve_on_ephemeral_port("tok").await;
    let addr = state.local_addr().expect("bound");
    assert!(addr.ip().is_loopback(), "bound {addr}, not loopback");
    stop_mcp_server(&state).await;
}

#[tokio::test]
async fn request_without_a_token_is_rejected() {
    let (state, url) = serve_on_ephemeral_port("secret-token").await;

    let response = client()
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&initialize_body())
        .send()
        .await
        .expect("request sent");

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert!(
        response.text().await.unwrap_or_default().is_empty(),
        "401 must not echo anything back"
    );
    stop_mcp_server(&state).await;
}

#[tokio::test]
async fn request_with_the_wrong_token_is_rejected() {
    let (state, url) = serve_on_ephemeral_port("secret-token").await;

    let response = client()
        .post(&url)
        .header("Authorization", "Bearer wrong-token")
        .header("Accept", "application/json, text/event-stream")
        .json(&initialize_body())
        .send()
        .await
        .expect("request sent");

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    stop_mcp_server(&state).await;
}

#[tokio::test]
async fn initialize_succeeds_with_the_right_token() {
    let (state, url) = serve_on_ephemeral_port("secret-token").await;

    let response = client()
        .post(&url)
        .header("Authorization", "Bearer secret-token")
        .header("Accept", "application/json, text/event-stream")
        .json(&initialize_body())
        .send()
        .await
        .expect("request sent");

    assert!(
        response.status().is_success(),
        "status {}",
        response.status()
    );
    let body = response.text().await.expect("body");
    assert!(
        body.contains("libretune"),
        "serverInfo missing from: {body}"
    );
    stop_mcp_server(&state).await;
}

#[tokio::test]
async fn stopping_frees_the_port() {
    // The whole point of awaiting the aborted task in `stop_mcp_server`.
    let state = McpServerState::default();
    start_mcp_server(&state, stub_factory(None), "tok".into(), 0)
        .await
        .expect("first bind");
    let port = state.local_addr().expect("bound").port();
    stop_mcp_server(&state).await;

    let restarted = McpServerState::default();
    start_mcp_server(&restarted, stub_factory(None), "tok".into(), port)
        .await
        .expect("the same port is free again");
    stop_mcp_server(&restarted).await;
}

#[test]
fn router_builds_without_a_socket() {
    // Cheap guard on the rmcp/axum wiring: a service-type mismatch here is a
    // compile error, and a panicking builder would be caught at runtime.
    let _ = build_router(stub_factory(None), "tok".into());
}
