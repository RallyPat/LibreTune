//! MCP protocol handler: maps LibreTune's agent read-tool catalogue onto
//! rmcp's [`ServerHandler`].
//!
//! This module owns the protocol translation only — no sockets, no HTTP
//! (that is [`super::server`]), and no tune knowledge (that is
//! [`libretune_core::agent`] and the Tauri-side [`ReadToolExecutor`]).
//!
//! The handler is deliberately built over an `Arc<dyn ReadToolExecutor>`
//! rather than a `tauri::AppHandle`: the executor trait is the same seam the
//! in-app assistant already runs through, so the MCP surface can never
//! drift from what the assistant sees, and the tests below can drive the
//! whole handler with a stub executor instead of a live app.
//!
//! ## Scope: read-only
//! Only [`libretune_core::agent::tools::is_read_tool`] tools are listed and
//! dispatched. The `propose_*` tools are withheld on purpose — a proposal
//! needs the review queue in the app UI to mean anything, and that queue is
//! per-chat-session frontend state today. Withholding them here is a
//! stronger guarantee than a client-side prompt: an external agent cannot
//! call what is not in `tools/list`.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map as JsonMap, Value as Json};

use libretune_core::agent::orchestrator::ReadToolExecutor;
use libretune_core::agent::tools;

/// Builds the executor that serves one MCP session. A factory rather than a
/// shared instance so each session reads through a freshly-resolved view of
/// app state, matching how `agent_send_message` builds one per turn.
pub type ExecutorFactory = Arc<dyn Fn() -> Arc<dyn ReadToolExecutor> + Send + Sync>;

/// The contract shown to the connecting agent. Adapted from the in-app
/// assistant's framing: an MCP client's model never sees LibreTune's UI, so
/// the "you cannot apply anything" rule has to be stated here or the model
/// will assume otherwise.
const MCP_INSTRUCTIONS: &str = "LibreTune's read-only tune inspection tools. \
     All numeric analysis must come from these tools — never invent or \
     compute tuning numbers yourself. You can NOT change the tune, apply \
     edits, or burn to the ECU: no write tools are offered here. To change \
     anything, the user must use LibreTune's own AI assistant or edit \
     manually — say so rather than claiming a change was made. If a tool \
     returns an object with an `error` field, report that reason honestly \
     instead of guessing at the answer.";

/// One MCP server session over LibreTune's read tools.
pub struct LibreTuneMcp {
    executor: ExecutorFactory,
}

impl LibreTuneMcp {
    pub fn new(executor: ExecutorFactory) -> Self {
        Self { executor }
    }

    /// The read-only tool list in rmcp's shape. A plain inherent method (not
    /// the trait method) so tests can call it without constructing rmcp's
    /// `RequestContext`, which has no public constructor outside a real
    /// transport.
    pub(crate) fn read_tools(&self) -> Vec<Tool> {
        tools::catalogue()
            .into_iter()
            .filter(|def| tools::is_read_tool(&def.name))
            .map(to_rmcp_tool)
            .collect()
    }

    /// Run one tool call. Never returns `Err`: a refused or failed tool is
    /// *data* for the calling model (MCP's tool-level error), not a
    /// JSON-RPC protocol fault.
    pub(crate) async fn dispatch(
        &self,
        name: &str,
        arguments: Option<JsonMap<String, Json>>,
    ) -> CallToolResult {
        if !tools::is_read_tool(name) {
            return tool_error(format!(
                "unknown tool '{name}' — this server exposes read-only tune tools only"
            ));
        }

        let executor = (self.executor)();
        if !executor.handles(name) {
            return tool_error(format!("tool '{name}' is not available right now"));
        }

        let args = Json::Object(arguments.unwrap_or_default()).to_string();
        to_call_result(executor.execute(name, &args).await)
    }
}

/// A tool-level MCP error carrying `message` as plain text.
fn tool_error(message: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message)])
}

/// Turn the executor's JSON-string answer into an MCP result.
///
/// The executor signals failure in-band as `{"error": "..."}` (see
/// `commands::agent::json_err`) because that is what an LLM tool-result
/// message wants. MCP has a real error channel, so unwrap it here — an
/// agent that only reads `isError` must not mistake a failure for data.
fn to_call_result(raw: String) -> CallToolResult {
    match serde_json::from_str::<Json>(&raw) {
        Ok(Json::Object(map)) => {
            if let Some(Json::String(message)) = map.get("error") {
                return tool_error(message.clone());
            }
            CallToolResult::structured(Json::Object(map))
        }
        // Non-object JSON (or not JSON at all): pass it through as text
        // rather than inventing a wrapper object the schema never promised.
        _ => CallToolResult::success(vec![ContentBlock::text(raw)]),
    }
}

/// `ToolDef::parameters` is an object schema for every tool in the
/// catalogue (`tools.rs` writes them as literals); `unwrap_or_default` is a
/// guard on that invariant, not a swallowed error.
fn to_rmcp_tool(def: libretune_core::llm::types::ToolDef) -> Tool {
    let schema = def.parameters.as_object().cloned().unwrap_or_default();
    Tool::new(def.name, def.description, Arc::new(schema))
}

impl ServerHandler for LibreTuneMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("libretune", env!("CARGO_PKG_VERSION")))
            .with_instructions(MCP_INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.read_tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.dispatch(&request.name, request.arguments).await)
    }
}
