//! Tauri commands for the AI assistant agent loop.
//!
//! These wrap the [`libretune_core::agent`] orchestrator and
//! [`libretune_core::llm`] provider client. They never apply changes: a turn
//! produces a [`Proposal`] that the frontend stages in a review queue. Only
//! `agent_apply_proposals` mutates the working tune, and even then burning to
//! the ECU is a separate manual user action.

use crate::state::AppState;
use libretune_core::action_scripting::Action;
use libretune_core::agent::orchestrator::{
    run_turn, OrchestratorInputs, Proposal, ReadToolExecutor,
};
use libretune_core::agent::tiers::ConstantSafetyTier;
use libretune_core::agent::tools;
use libretune_core::autotune::AutoTuneAuthorityLimits;
use libretune_core::llm::types::{LlmError, Message};
use libretune_core::llm::{LlmClient, ProviderConfig};
use libretune_core::tune::TuneValue;
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Construct a `ProviderConfig` from stored settings.
fn config_from_settings(s: &crate::Settings) -> ProviderConfig {
    ProviderConfig {
        provider: s.ai_provider.clone(),
        base_url: s.ai_base_url.clone(),
        api_key: s.ai_api_key.clone(),
        model: s.ai_model.clone(),
    }
}

/// Build an `LlmClient` from current settings.
/// Errors surface as `Result<T, String>` per the app's convention.
fn build_client(s: &crate::Settings) -> Result<LlmClient, LlmError> {
    LlmClient::new(&config_from_settings(s))
}

/// A single chat message as stored in a chat history file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

/// One persisted chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistory {
    /// Unique id (uuid or timestamp-based).
    pub id: String,
    /// Auto-generated from the first user message.
    pub title: String,
    pub messages: Vec<ChatMessage>,
    pub created_at: String,
    pub updated_at: String,
}

/// Summary entry for the chat list (without full messages).
#[derive(Debug, Clone, Serialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub message_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

/// Resolve the ai_chats directory for the current project.
fn chats_dir(project_path: &std::path::Path) -> std::path::PathBuf {
    project_path.join("projectCfg").join("ai_chats")
}

/// List all saved chats for the current project.
#[tauri::command]
pub async fn agent_list_chats(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ChatSummary>, String> {
    let proj = state.current_project.lock().await;
    let Some(project) = proj.as_ref() else {
        return Ok(Vec::new());
    };
    let dir = chats_dir(&project.path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries: Vec<ChatSummary> = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(chat) = serde_json::from_str::<ChatHistory>(&content) {
                summaries.push(ChatSummary {
                    id: chat.id,
                    title: chat.title,
                    message_count: chat.messages.len(),
                    created_at: chat.created_at,
                    updated_at: chat.updated_at,
                });
            }
        }
    }
    // Most-recently-updated first.
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(summaries)
}

/// Load a full chat by id.
#[tauri::command]
pub async fn agent_load_chat(
    state: tauri::State<'_, AppState>,
    chat_id: String,
) -> Result<ChatHistory, String> {
    let proj = state.current_project.lock().await;
    let Some(project) = proj.as_ref() else {
        return Err("No project loaded".to_string());
    };
    let path = chats_dir(&project.path).join(format!("{chat_id}.json"));
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read chat {chat_id}: {e}"))?;
    serde_json::from_str::<ChatHistory>(&content).map_err(|e| e.to_string())
}

/// Save (create or update) a chat. Returns the saved chat with timestamps set.
#[tauri::command]
pub async fn agent_save_chat(
    state: tauri::State<'_, AppState>,
    mut chat: ChatHistory,
) -> Result<ChatHistory, String> {
    let proj = state.current_project.lock().await;
    let Some(project) = proj.as_ref() else {
        return Err("No project loaded".to_string());
    };
    let dir = chats_dir(&project.path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    if chat.created_at.is_empty() {
        chat.created_at = now.clone();
    }
    chat.updated_at = now;

    let path = dir.join(format!("{}.json", chat.id));
    let json = serde_json::to_string_pretty(&chat).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(chat)
}

/// Delete a chat by id.
#[tauri::command]
pub async fn agent_delete_chat(
    state: tauri::State<'_, AppState>,
    chat_id: String,
) -> Result<(), String> {
    let proj = state.current_project.lock().await;
    let Some(project) = proj.as_ref() else {
        return Err("No project loaded".to_string());
    };
    let path = chats_dir(&project.path).join(format!("{chat_id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A serialized [`Message`] that round-trips through JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMessage {
    pub role: String,
    pub content: String,
}

impl From<SerializedMessage> for Message {
    fn from(s: SerializedMessage) -> Self {
        match s.role.as_str() {
            "system" => Message::system(s.content),
            "assistant" => Message::assistant(s.content),
            _ => Message::user(s.content),
        }
    }
}

/// Request payload from the frontend for one assistant turn.
#[derive(Debug, Deserialize)]
pub struct AgentTurnRequest {
    /// The user's message this turn.
    pub user_message: String,
    /// Prior conversation as the frontend has it (serialized messages).
    pub history: Vec<SerializedMessage>,
    /// Pre-rendered system prompt describing the ECU/tune context. The
    /// frontend builds this from the current view (tables loaded, etc.).
    pub system_prompt: String,
}

/// Build a default authority-limit envelope for clamping proposals.
fn default_authority() -> AutoTuneAuthorityLimits {
    AutoTuneAuthorityLimits::default()
}

/// Check whether the assistant is configured and enabled. Cheap pre-flight.
#[tauri::command]
pub async fn agent_status(app: tauri::AppHandle) -> Result<AgentStatus, String> {
    let s = crate::load_settings(&app);
    // Treat an empty provider as the default ("openai") so the configured-flag
    // isn't falsely false for setups that only set a base URL + key + model.
    let provider = if s.ai_provider.is_empty() { "openai".to_string() } else { s.ai_provider.clone() };
    Ok(AgentStatus {
        enabled: s.ai_assistant_enabled,
        risk_acknowledged: s.ai_risk_acknowledged,
        provider: provider.clone(),
        model: s.ai_model.clone(),
        capability_tier: s.ai_capability_tier.clone(),
        // Configured if both provider and model are non-empty (key is optional
        // for local providers, so we don't require it).
        configured: !provider.is_empty() && !s.ai_model.is_empty(),
    })
}

#[derive(Debug, Serialize)]
pub struct AgentStatus {
    pub enabled: bool,
    pub risk_acknowledged: bool,
    pub provider: String,
    pub model: String,
    pub capability_tier: String,
    pub configured: bool,
}

/// Executes the assistant's read-only tool calls against the live ECU/tune
/// state. Held by `agent_send_message` and handed to the orchestrator so the
/// model's "let me read your VE table" calls actually return data instead of
/// stalling.
///
/// Holds a [`tauri::AppHandle`] (cheap to clone) to reach managed `AppState`
/// without borrowing the `tauri::State` lifetime into the executor.
struct LiveReadExecutor {
    app: tauri::AppHandle,
}

#[async_trait::async_trait]
impl ReadToolExecutor for LiveReadExecutor {
    fn handles(&self, tool_name: &str) -> bool {
        matches!(
            tool_name,
            tools::tool_names::READ_TABLE
                | tools::tool_names::READ_CONSTANT
                | tools::tool_names::LIST_TABLES
                | tools::tool_names::LIST_FEATURES
        )
    }

    async fn execute(&self, tool_name: &str, arguments: &str) -> String {
        match tool_name {
            tools::tool_names::LIST_TABLES => self.exec_list_tables().await,
            tools::tool_names::LIST_FEATURES => self.exec_list_features().await,
            tools::tool_names::READ_TABLE => {
                let name = json_str_field(arguments, "table_name");
                match name {
                    Some(n) => self.exec_read_table(&n).await,
                    None => json_err("read_table requires 'table_name'"),
                }
            }
            tools::tool_names::READ_CONSTANT => {
                let name = json_str_field(arguments, "name");
                match name {
                    Some(n) => self.exec_read_constant(&n).await,
                    None => json_err("read_constant requires 'name'"),
                }
            }
            _ => json_err(&format!("unhandled read tool '{tool_name}'")),
        }
    }
}

impl LiveReadExecutor {
    async fn exec_list_tables(&self) -> String {
        let state = self.app.state::<AppState>();
        let def_guard = state.definition.lock().await;
        let Some(def) = def_guard.as_ref() else {
            return json_err("No INI definition loaded");
        };
        let mut entries: Vec<serde_json::Value> = Vec::new();
        for t in def.tables.values() {
            entries.push(serde_json::json!({
                "name": t.name,
                "title": t.title,
                "role": format!("{:?}", t.role),
                "dimensions": [t.x_size, t.y_size],
                "x_label": t.x_label,
                "y_label": t.y_label,
            }));
        }
        serde_json::to_string(&serde_json::json!({ "tables": entries }))
            .unwrap_or_else(|_| json_err("serialize failed"))
    }

    async fn exec_list_features(&self) -> String {
        let state = self.app.state::<AppState>();
        let def_guard = state.definition.lock().await;
        let Some(def) = def_guard.as_ref() else {
            return json_err("No INI definition loaded");
        };
        let mut entries: Vec<serde_json::Value> = Vec::new();
        for c in def.constants.values() {
            if !c.bit_options.is_empty() {
                entries.push(serde_json::json!({
                    "name": c.name,
                    "label": c.label,
                    "options": c.bit_options,
                    "help": c.help,
                }));
            }
        }
        serde_json::to_string(&serde_json::json!({ "features": entries }))
            .unwrap_or_else(|_| json_err("serialize failed"))
    }

    async fn exec_read_table(&self, name: &str) -> String {
        let state = self.app.state::<AppState>();
        // Reuse the existing internal table reader so the model sees the same
        // data the table editors do.
        match crate::get_table_data_internal(&state, name).await {
            Ok(t) => serde_json::to_string(&serde_json::json!({
                "name": t.name,
                "title": t.title,
                "x_bins": t.x_bins,
                "y_bins": t.y_bins,
                "z_values": t.z_values,
                "x_axis": t.x_axis_name,
                "y_axis": t.y_axis_name,
            }))
            .unwrap_or_else(|_| json_err("serialize failed")),
            Err(e) => json_err(&format!("could not read table '{name}': {e}")),
        }
    }

    async fn exec_read_constant(&self, name: &str) -> String {
        let state = self.app.state::<AppState>();

        // 1. Read the constant metadata under the definition lock, then drop
        //    it before acquiring the tune lock (avoids nested-lock deadlocks).
        let (label, units, min, max, bit_options, help) = {
            let def_guard = state.definition.lock().await;
            let Some(def) = def_guard.as_ref() else {
                return json_err("No INI definition loaded");
            };
            let Some(c) = def.constants.get(name) else {
                return json_err(&format!("constant '{name}' not found"));
            };
            (
                c.label.clone(),
                c.units.clone(),
                c.min,
                c.max,
                c.bit_options.clone(),
                c.help.clone(),
            )
        };

        // 2. Read the current value from the loaded tune if present.
        let current: Option<f64> = {
            let tune_guard = state.current_tune.lock().await;
            match tune_guard.as_ref().and_then(|tune| tune.get_value(name)) {
                Some(TuneValue::Scalar(f)) => Some(*f),
                Some(TuneValue::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
                _ => None,
            }
        };

        serde_json::to_string(&serde_json::json!({
            "name": name,
            "label": label,
            "units": units,
            "min": min,
            "max": max,
            "current_value": current,
            "options": bit_options,
            "help": help,
        }))
        .unwrap_or_else(|_| json_err("serialize failed"))
    }
}

fn json_str_field(arguments: &str, field: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|v| v.get(field)?.as_str().map(|s| s.to_string()))
}

fn json_err(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}

/// Run one assistant turn. Returns a [`Proposal`] for the review queue.
///
/// Does not apply anything. The frontend renders `proposal.proposed` as a
/// reviewable list; the user explicitly approves items before
/// `agent_apply_proposals` stages them to the working tune.
#[tauri::command]
pub async fn agent_send_message(
    app: tauri::AppHandle,
    request: AgentTurnRequest,
) -> Result<Proposal, String> {
    let s = crate::load_settings(&app);

    // Gate: must be enabled + risk-acknowledged.
    if !s.ai_assistant_enabled {
        return Err("AI assistant is not enabled".to_string());
    }
    if !s.ai_risk_acknowledged {
        return Err("AI assistant risk acknowledgement is missing".to_string());
    }

    let client = build_client(&s).map_err(|e| e.to_string())?;
    let executor = LiveReadExecutor { app: app.clone() };

    let history: Vec<Message> = request.history.into_iter().map(Into::into).collect();
    let inputs = OrchestratorInputs {
        history,
        user_message: request.user_message,
        system_prompt: request.system_prompt,
        current_table_values: Default::default(),
    };

    let authority = default_authority();

    // Spawn the turn so it can be aborted by `agent_stop` (mirrors the realtime
    // stream pattern). A oneshot channel carries the result back; if the task
    // is aborted, the sender drops and the receiver resolves to a RecvError,
    // which we surface as the sentinel "__cancelled__" so the frontend can
    // treat it as a user-initiated stop rather than an error.
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<Proposal, String>>();
    let handle = tokio::spawn(async move {
        let result = run_turn(&client, &inputs, &authority, Some(&executor))
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });

    // Store the handle so agent_stop can abort it. Replace any prior handle.
    {
        let state = app.state::<AppState>();
        let mut guard = state.agent_task.lock().await;
        if let Some(old) = guard.take() {
            old.abort();
        }
        *guard = Some(handle);
    }

    // Await the result. A RecvError means the task was aborted (cancelled).
    match rx.await {
        Ok(result) => result,
        Err(_) => {
            // Clear the now-finished handle.
            let state = app.state::<AppState>();
            let mut guard = state.agent_task.lock().await;
            *guard = None;
            Err("__cancelled__".to_string())
        }
    }
}

/// Cancel an in-flight assistant turn (the "Stop" button).
/// Aborts the spawned task; the awaiting `agent_send_message` resolves to the
/// sentinel error `"__cancelled__"`.
#[tauri::command]
pub async fn agent_stop(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut guard = state.agent_task.lock().await;
    if let Some(handle) = guard.take() {
        handle.abort();
    }
    Ok(())
}

/// Request payload for applying a subset of a proposal.
#[derive(Debug, Deserialize)]
pub struct ApplyProposalsRequest {
    /// The actions to apply, exactly as the user approved them from the
    /// proposal queue. Re-validated here before apply.
    pub actions: Vec<Action>,
}

/// Result of applying one action.
#[derive(Debug, Serialize)]
pub struct ApplyResult {
    pub applied: bool,
    pub error: Option<String>,
    /// Safety tier (constants only) so the UI can show what was applied.
    pub safety_tier: Option<ConstantSafetyTier>,
}

/// Apply a list of approved actions to the working tune.
///
/// Each action is re-validated against the loaded definition; invalid ones are
/// skipped with an error in the result. **Nothing is burned to the ECU** —
/// the changes are staged in the working tune and flagged as modified, so the
/// user must explicitly burn afterward.
#[tauri::command]
pub async fn agent_apply_proposals(
    state: tauri::State<'_, AppState>,
    request: ApplyProposalsRequest,
) -> Result<Vec<ApplyResult>, String> {
    use libretune_core::action_scripting::{ActionMetadata, ActionPlayer, ActionSet};

    // 1. Validate every action while holding the definition lock (read-only).
    let mut results: Vec<ApplyResult> = Vec::with_capacity(request.actions.len());
    let mut any_applied = false;
    {
        let def = state.definition.lock().await;
        let def_ref = def.as_ref();

        for action in &request.actions {
            let tier = match action {
                Action::ConstantChange { constant_name, .. } => {
                    Some(libretune_core::agent::constant_safety_tier(constant_name))
                }
                _ => None,
            };

            let set = ActionSet {
                id: "apply".into(),
                name: "apply".into(),
                description: "Approved AI proposal action".into(),
                version: "1".into(),
                actions: vec![action.clone()],
                metadata: ActionMetadata {
                    created_by: "ai-assistant".into(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    modified_at: chrono::Utc::now().to_rfc3339(),
                    tags: vec!["ai-applied".into()],
                    compatible_ecus: vec![],
                },
            };

            match ActionPlayer::validate_action_set(&set, def_ref) {
                Ok(_warnings) => {
                    any_applied = true;
                    results.push(ApplyResult {
                        applied: true,
                        error: None,
                        safety_tier: tier,
                    });
                }
                Err(errors) => {
                    results.push(ApplyResult {
                        applied: false,
                        error: Some(errors.join("; ")),
                        safety_tier: tier,
                    });
                }
            }
        }
    } // definition lock released here

    // 2. If at least one action applied, flag the tune as modified so the
    //    user is prompted to burn. The actual table/constant mutation is
    //    performed by the frontend via the existing update commands (this
    //    command validates + signals intent; it does not itself write to
    //    tune state, to avoid duplicating the page-write path).
    if any_applied {
        let mut modified = state.tune_modified.lock().await;
        *modified = true;
    }

    Ok(results)
}
