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
    run_turn_observed, OrchestratorInputs, Proposal, ReadToolExecutor, TurnObserver,
    ValidationResult,
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

/// The user's display-unit preferences, passed per-request from the
/// frontend (they live in localStorage there). Only the categories the
/// assistant can currently convert are carried.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UnitPrefs {
    /// "C" | "F" | "K"
    pub temperature: Option<String>,
    /// "kPa" | "PSI" | "bar" | "inHg"
    pub pressure: Option<String>,
    /// "AFR" | "Lambda"
    pub afr: Option<String>,
    /// Stoichiometric AFR fuel key for AFR↔lambda ("gasoline", "e85", ...).
    pub fuel_type: Option<String>,
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
    /// Display-unit preferences for read-tool results (optional; raw values
    /// are returned when absent).
    #[serde(default)]
    pub unit_prefs: Option<UnitPrefs>,
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
    let provider = if s.ai_provider.is_empty() {
        "openai".to_string()
    } else {
        s.ai_provider.clone()
    };
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
    /// Display-unit preferences for read results (None = raw values).
    unit_prefs: Option<UnitPrefs>,
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
                | tools::tool_names::SUMMARIZE_TUNE
                | tools::tool_names::TUNE_HEALTH
                | tools::tool_names::REALTIME_SNAPSHOT
                | tools::tool_names::QUERY_DATALOG
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
            tools::tool_names::SUMMARIZE_TUNE => {
                let name = json_str_field(arguments, "table_name");
                match name {
                    Some(n) => self.exec_summarize(&n, false).await,
                    None => json_err("summarize_tune_context requires 'table_name'"),
                }
            }
            tools::tool_names::TUNE_HEALTH => {
                let name = json_str_field(arguments, "table_name");
                match name {
                    Some(n) => self.exec_summarize(&n, true).await,
                    None => json_err("tune_health_check requires 'table_name'"),
                }
            }
            tools::tool_names::REALTIME_SNAPSHOT => self.exec_realtime_snapshot().await,
            tools::tool_names::QUERY_DATALOG => self.exec_query_datalog(arguments).await,
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

        // 3. Convert to the user's preferred display unit (best-effort;
        //    min/max follow the value so the model never mixes units).
        let (current, min, max, units, original) = {
            let mut original = serde_json::Value::Null;
            let converted = current.and_then(|v| self.convert_unit(v, &units));
            match converted {
                Some((v, ref label)) => {
                    original = serde_json::json!({ "value": current, "units": units });
                    let (cmin, cmax) = match (
                        self.convert_unit(min, &units),
                        self.convert_unit(max, &units),
                    ) {
                        (Some((mn, _)), Some((mx, _))) => (mn, mx),
                        _ => (min, max),
                    };
                    (Some(v), cmin, cmax, label.clone(), original)
                }
                None => (current, min, max, units, original),
            }
        };

        serde_json::to_string(&serde_json::json!({
            "name": name,
            "label": label,
            "units": units,
            "min": min,
            "max": max,
            "current_value": current,
            "original_value": original,
            "options": bit_options,
            "help": help,
        }))
        .unwrap_or_else(|_| json_err("serialize failed"))
    }

    /// Convert a scalar value from its INI unit to the user's preferred
    /// display unit. Returns `(converted_value, new_unit_label)` or `None`
    /// when no conversion applies (unknown unit, same unit, or no prefs).
    fn convert_unit(&self, value: f64, units: &str) -> Option<(f64, String)> {
        let prefs = self.unit_prefs.as_ref()?;
        let u = units.trim();
        let lower = u.to_lowercase();

        // Temperature (INI uses "C" / "°C" / "F" / "°F").
        let is_c = u == "C" || u == "°C" || lower == "celsius";
        let is_f = u == "F" || u == "°F" || lower == "fahrenheit";
        if is_c || is_f {
            let target = prefs.temperature.as_deref()?;
            let c_value = if is_c {
                value
            } else {
                libretune_core::unit_conversion::fahrenheit_to_celsius(value)
            };
            return match target {
                "F" if !is_f => Some((
                    libretune_core::unit_conversion::celsius_to_fahrenheit(c_value),
                    "°F".into(),
                )),
                "C" if !is_c => Some((c_value, "°C".into())),
                // Same unit (or unsupported K target): no conversion.
                _ => None,
            };
        }

        // Pressure (INI uses "kPa" / "PSI" / "bar").
        if lower == "kpa" || lower == "psi" || lower == "bar" {
            let target = prefs.pressure.as_deref()?.to_lowercase();
            let kpa_value = match lower.as_str() {
                "kpa" => value,
                "psi" => libretune_core::unit_conversion::psi_to_kpa(value),
                _ => libretune_core::unit_conversion::psi_to_kpa(
                    libretune_core::unit_conversion::bar_to_psi(value),
                ),
            };
            return match target.as_str() {
                "psi" if lower != "psi" => Some((
                    libretune_core::unit_conversion::kpa_to_psi(kpa_value),
                    "PSI".into(),
                )),
                "kpa" if lower != "kpa" => Some((kpa_value, "kPa".into())),
                "bar" if lower != "bar" => Some((
                    libretune_core::unit_conversion::psi_to_bar(
                        libretune_core::unit_conversion::kpa_to_psi(kpa_value),
                    ),
                    "bar".into(),
                )),
                _ => None,
            };
        }

        // AFR → lambda.
        if lower == "afr" && prefs.afr.as_deref() == Some("Lambda") {
            let fuel = prefs.fuel_type.as_deref().unwrap_or("gasoline");
            let lambda = libretune_core::unit_conversion::afr_to_lambda(value, fuel);
            return Some((lambda, "λ".into()));
        }

        None
    }

    /// `summarize_tune_context` / `tune_health_check`: aggregate one table
    /// through the core summary engine (coverage, AFR error, anomalies,
    /// predicted cells, region health), fed from the live table grid and any
    /// AutoTune session recommendations for that table.
    ///
    /// `health_only` trims the payload to the health fields for the lighter
    /// `tune_health_check` tool.
    async fn exec_summarize(&self, name: &str, health_only: bool) -> String {
        use libretune_core::agent::summarize::{summarize_tune_context, TuneContextInputs};
        use std::collections::HashMap;

        let state = self.app.state::<AppState>();

        // 1. Current grid + bins via the same reader the table editors use.
        let t = match crate::commands::table_internals::get_table_data_internal(&state, name).await
        {
            Ok(t) => t,
            Err(e) => return json_err(&format!("could not read table '{name}': {e}")),
        };

        // 2. Role + dimensionality from the definition.
        let (role, is_3d) = {
            let def_guard = state.definition.lock().await;
            match def_guard
                .as_ref()
                .and_then(|d| d.get_table_by_name_or_map(name))
            {
                Some(td) => (format!("{:?}", td.role), td.is_3d()),
                None => return json_err(&format!("table '{name}' not found in definition")),
            }
        };

        // 3. AutoTune recommendations, but only when the session was for
        //    THIS table (primary or configured secondary) — recommendations
        //    from another table's session would be meaningless here.
        let (primary_tbl, secondary_tbl) = {
            let config_guard = state.autotune_config.lock().await;
            match config_guard.as_ref() {
                Some(c) => (c.table_name.clone(), c.secondary_table_name.clone()),
                None => (String::new(), None),
            }
        };
        let recs: HashMap<(usize, usize), libretune_core::autotune::AutoTuneRecommendation> =
            if primary_tbl == name {
                state
                    .autotune_state
                    .lock()
                    .await
                    .get_recommendations()
                    .into_iter()
                    .map(|r| ((r.cell_x, r.cell_y), r))
                    .collect()
            } else if secondary_tbl.as_deref() == Some(name) {
                state
                    .autotune_secondary_state
                    .lock()
                    .await
                    .get_recommendations()
                    .into_iter()
                    .map(|r| ((r.cell_x, r.cell_y), r))
                    .collect()
            } else {
                HashMap::new()
            };

        // 4. Hit-count grid (row-major) from the recommendations.
        let rows = t.z_values.len();
        let cols = t.z_values.first().map(|r| r.len()).unwrap_or(0);
        let mut hit_counts = vec![vec![0u32; cols]; rows];
        for ((x, y), r) in &recs {
            if *y < rows && *x < cols {
                hit_counts[*y][*x] = r.hit_count;
            }
        }

        // 5. Summarize. For 2D tables pass empty y-bins so the anomaly /
        //    prediction engines (which need a real 2-axis grid) are skipped
        //    instead of running against the 2D placeholder axis.
        let y_bins: &[f64] = if is_3d { &t.y_bins } else { &[] };
        // Operating point from a one-shot realtime poll. Best-effort: no
        // connection / no rpm channel just means no operating point.
        let operating_point = crate::commands::realtime_get::realtime_snapshot_internal(&state)
            .await
            .ok()
            .and_then(|data| {
                build_operating_point(&data, &t.x_bins, &t.y_bins, &t.x_axis_name, &t.y_axis_name)
            });
        let inputs = TuneContextInputs {
            table_values: &t.z_values,
            x_bins: &t.x_bins,
            y_bins,
            hit_counts: &hit_counts,
            recommendations: &recs,
            operating_point,
            max_anomalies: 20,
        };
        let summary = summarize_tune_context(name, &role, &inputs);

        if health_only {
            serde_json::to_string(&serde_json::json!({
                "table": name,
                "overall_health_score": summary.overall_health_score,
                "region_health": summary.region_health,
                "digest": summary.digest,
            }))
            .unwrap_or_else(|_| json_err("serialize failed"))
        } else {
            serde_json::to_string(&summary).unwrap_or_else(|_| json_err("serialize failed"))
        }
    }

    /// `get_realtime_snapshot`: one poll of the ECU's current channels,
    /// curated to the tuning-relevant subset so the payload stays small.
    async fn exec_realtime_snapshot(&self) -> String {
        let state = self.app.state::<AppState>();
        match crate::commands::realtime_get::realtime_snapshot_internal(&state).await {
            Ok(data) => {
                let curated = curate_snapshot_channels(&data);
                serde_json::to_string(&serde_json::json!({
                    "channels": curated,
                    "total_channels_available": data.len(),
                }))
                .unwrap_or_else(|_| json_err("serialize failed"))
            }
            Err(e) => json_err(&format!("no realtime data ({e})")),
        }
    }

    /// `query_datalog`: summary stats or tail rows over a saved log (by
    /// name, from the project's `datalogs/` folder) or the current
    /// in-memory session. Responses are bounded (≤50 channels for summary,
    /// ≤50 rows × ≤12 columns for tail) to control token cost.
    async fn exec_query_datalog(&self, arguments: &str) -> String {
        let state = self.app.state::<AppState>();
        let log_name = json_str_field(arguments, "log");
        let mode = json_str_field(arguments, "mode").unwrap_or_else(|| "summary".into());
        let channel_filter = json_str_array(arguments, "channels");

        let data = match log_name {
            Some(name) => {
                match crate::commands::data_logging::load_datalog_file(&state, &name).await {
                    Ok(d) => d,
                    Err(e) => {
                        // Self-healing: hand back the available log names so
                        // the model can retry with a real one.
                        let logs = crate::commands::data_logging::list_datalog_files(&state).await;
                        return serde_json::to_string(&serde_json::json!({
                            "error": e,
                            "available_logs": logs.iter().map(|l| &l.name).collect::<Vec<_>>(),
                        }))
                        .unwrap_or_else(|_| json_err("serialize failed"));
                    }
                }
            }
            None => crate::commands::data_logging::current_session_datalog(&state).await,
        };

        if data.entries.is_empty() {
            return serde_json::to_string(&serde_json::json!({
                "source": data.source,
                "entry_count": 0,
                "note": "no entries recorded; start datalogging or name a saved log ('log' parameter)",
            }))
            .unwrap_or_else(|_| json_err("serialize failed"));
        }

        // Resolve the channel column indexes the caller asked for (or all).
        let indexes: Vec<usize> = data
            .channels
            .iter()
            .enumerate()
            .filter(|(_, name)| match &channel_filter {
                Some(f) if !f.is_empty() => f.iter().any(|want| want.eq_ignore_ascii_case(name)),
                _ => true,
            })
            .map(|(i, _)| i)
            .collect();

        let entry_count = data.entries.len();
        let duration_s = data
            .entries
            .last()
            .map(|e| e.timestamp.as_secs_f64())
            .unwrap_or(0.0);

        if mode == "tail" {
            let tail_start = entry_count.saturating_sub(50);
            let columns: Vec<(String, usize)> = indexes
                .into_iter()
                .take(12)
                .map(|i| (data.channels[i].clone(), i))
                .collect();
            let rows: Vec<serde_json::Value> = data.entries[tail_start..]
                .iter()
                .map(|e| {
                    let mut row = serde_json::json!({ "t": e.timestamp.as_secs_f64() });
                    for (name, i) in &columns {
                        row[name] = serde_json::json!(e.values.get(*i).copied());
                    }
                    row
                })
                .collect();
            return serde_json::to_string(&serde_json::json!({
                "source": data.source,
                "mode": "tail",
                "entry_count": entry_count,
                "duration_s": duration_s,
                "rows": rows,
            }))
            .unwrap_or_else(|_| json_err("serialize failed"));
        }

        // summary (default)
        let mut channel_stats: Vec<serde_json::Value> = Vec::new();
        for &i in indexes.iter().take(50) {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            let mut sum = 0.0;
            let mut n = 0u64;
            let mut last = None;
            for e in &data.entries {
                if let Some(v) = e.values.get(i) {
                    if v.is_finite() {
                        min = min.min(*v);
                        max = max.max(*v);
                        sum += *v;
                        n += 1;
                        last = Some(*v);
                    }
                }
            }
            if n == 0 {
                continue;
            }
            channel_stats.push(serde_json::json!({
                "channel": data.channels[i],
                "min": min,
                "max": max,
                "mean": sum / n as f64,
                "last": last,
                "samples": n,
            }));
        }
        serde_json::to_string(&serde_json::json!({
            "source": data.source,
            "mode": "summary",
            "entry_count": entry_count,
            "duration_s": duration_s,
            "channels": channel_stats,
            "channels_available": data.channels.len(),
        }))
        .unwrap_or_else(|_| json_err("serialize failed"))
    }
}

fn json_str_field(arguments: &str, field: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|v| v.get(field)?.as_str().map(|s| s.to_string()))
}

fn json_str_array(arguments: &str, field: &str) -> Option<Vec<String>> {
    let parsed = serde_json::from_str::<serde_json::Value>(arguments).ok()?;
    let arr = parsed.get(field)?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
    )
}

/// Case-insensitive channel lookup with the first alias-style fallback.
fn channel_value(data: &std::collections::HashMap<String, f64>, names: &[&str]) -> Option<f64> {
    for want in names {
        if let Some((_, v)) = data.iter().find(|(k, _)| k.eq_ignore_ascii_case(want)) {
            return Some(*v);
        }
    }
    None
}

/// Push a (name, value) pair into `out` unless it is already there or the
/// cap is reached.
fn push_capped(out: &mut Vec<(String, f64)>, name: &str, value: f64, cap: usize) {
    if out.len() < cap && !out.iter().any(|(n, _)| n == name) {
        out.push((name.to_string(), value));
    }
}

/// Curate a realtime snapshot down to the tuning-relevant channels: exact
/// canonical names first, then substring matches, capped at 24 entries.
fn curate_snapshot_channels(data: &std::collections::HashMap<String, f64>) -> Vec<(String, f64)> {
    const EXACT: &[&str] = &[
        "rpm", "map", "tps", "clt", "iat", "afr", "lambda", "batt", "baro", "fuelLoad",
    ];
    const PATTERNS: &[&str] = &[
        "rpm", "map", "tps", "throttle", "clt", "coolant", "iat", "afr", "lambda", "batt", "volt",
        "baro", "ego", "duty", "advance", "spark", "dwell", "enrich", "load", "temp",
    ];
    const CAP: usize = 24;

    let mut out: Vec<(String, f64)> = Vec::new();
    for want in EXACT {
        if let Some((k, v)) = data.iter().find(|(k, _)| k.eq_ignore_ascii_case(want)) {
            push_capped(&mut out, k, *v, CAP);
        }
    }
    for (name, value) in data {
        let lower = name.to_lowercase();
        if PATTERNS.iter().any(|p| lower.contains(p)) {
            push_capped(&mut out, name, *value, CAP);
        }
    }
    out
}

/// Locate the value in a bin axis (last bin whose start is ≤ value; clamped
/// to the first bin when below it).
fn bin_index(bins: &[f64], value: f64) -> Option<usize> {
    if bins.is_empty() {
        return None;
    }
    let mut idx = 0;
    for (i, &b) in bins.iter().enumerate() {
        if value >= b {
            idx = i;
        } else {
            break;
        }
    }
    Some(idx)
}

/// Build an [`OperatingPoint`] from a realtime snapshot and a table's axes.
///
/// The RPM axis is identified by its label; the other axis is treated as
/// load (MAP by default, TPS when the axis label says so). Returns `None`
/// when there is no RPM channel (engine off / different INI / no ECU
/// formula).
fn build_operating_point(
    data: &std::collections::HashMap<String, f64>,
    x_bins: &[f64],
    y_bins: &[f64],
    x_axis_name: &str,
    y_axis_name: &str,
) -> Option<libretune_core::agent::summarize::OperatingPoint> {
    let rpm = channel_value(data, &["rpm"])?;
    let afr = channel_value(data, &["afr", "lambda"]);

    let y_is_tps = {
        let y = y_axis_name.to_lowercase();
        y.contains("tps") || y.contains("throttle")
    };
    let load = if y_is_tps {
        channel_value(data, &["tps", "throttle"]).unwrap_or(0.0)
    } else {
        channel_value(data, &["map", "fuelload", "load"]).unwrap_or(0.0)
    };

    // Which axis is RPM? Default layout: x = RPM, y = load.
    let x_lower = x_axis_name.to_lowercase();
    let cell = if x_lower.contains("rpm") {
        match (bin_index(x_bins, rpm), bin_index(y_bins, load)) {
            (Some(c), Some(r)) => Some((c, r)),
            _ => None,
        }
    } else {
        match (bin_index(x_bins, load), bin_index(y_bins, rpm)) {
            (Some(c), Some(r)) => Some((c, r)),
            _ => None,
        }
    };

    Some(libretune_core::agent::summarize::OperatingPoint {
        rpm,
        load,
        cell,
        afr,
    })
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
    let executor = LiveReadExecutor {
        app: app.clone(),
        unit_prefs: request.unit_prefs.clone(),
    };

    let history: Vec<Message> = request.history.into_iter().map(Into::into).collect();
    let inputs = OrchestratorInputs {
        history,
        user_message: request.user_message,
        system_prompt: request.system_prompt,
        current_table_values: Default::default(),
        // Gate the tool catalogue (and propose mapping) to the configured
        // tier. `parse` collapses unknown values to the read-only tier.
        capability_tier: tools::CapabilityTier::parse(&s.ai_capability_tier),
    };

    let authority = default_authority();

    // Spawn the turn so it can be aborted by `agent_stop` (mirrors the realtime
    // stream pattern). A oneshot channel carries the result back; if the task
    // is aborted, the sender drops and the receiver resolves to a RecvError,
    // which we surface as the sentinel "__cancelled__" so the frontend can
    // treat it as a user-initiated stop rather than an error.
    let progress = AgentProgressEmitter { app: app.clone() };
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<Proposal, String>>();
    let handle = tokio::spawn(async move {
        let result = run_turn_observed(
            &client,
            &inputs,
            &authority,
            Some(&executor),
            Some(&progress),
        )
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
        Ok(Ok(mut proposal)) => {
            // Post-process: attach pin-conflict warnings to any constant
            // proposals (the model cannot see pin state; the user must).
            append_pin_conflict_warnings(&app, &mut proposal).await;
            Ok(proposal)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => {
            // Clear the now-finished handle.
            let state = app.state::<AppState>();
            let mut guard = state.agent_task.lock().await;
            *guard = None;
            Err("__cancelled__".to_string())
        }
    }
}

/// Emit `agent:progress` events while a turn runs, so the chat panel can
/// show "reading veTable1…" activity instead of a silent pending bubble
/// through multi-second read rounds. Follows the afr_delay_test:progress
/// event pattern.
struct AgentProgressEmitter {
    app: tauri::AppHandle,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "phase")]
enum AgentProgress {
    Thinking { round: usize },
    ReadingTool { round: usize, tool: String },
    Done { proposal_count: usize },
}

impl TurnObserver for AgentProgressEmitter {
    fn on_model_call(&self, round: usize) {
        use tauri::Emitter;
        let _ = self
            .app
            .emit("agent:progress", AgentProgress::Thinking { round });
    }

    fn on_read_tool(&self, round: usize, tool_name: &str, _arguments: &str) {
        use tauri::Emitter;
        let _ = self.app.emit(
            "agent:progress",
            AgentProgress::ReadingTool {
                round,
                tool: tool_name.to_string(),
            },
        );
    }

    fn on_complete(&self, proposal_count: usize) {
        use tauri::Emitter;
        let _ = self
            .app
            .emit("agent:progress", AgentProgress::Done { proposal_count });
    }
}

/// Attach pin-conflict warnings to proposed constant changes.
///
/// The orchestrator (core, state-free) cannot check pin assignments — only
/// the command layer can read the live tune. A proposal that would move a
/// function onto a pin another live function already uses gets a warning
/// appended to its validation; it is NOT failed, because the user may
/// intend to clear the other assignment first. The review queue shows the
/// warning next to the Accept button.
async fn append_pin_conflict_warnings(app: &tauri::AppHandle, proposal: &mut Proposal) {
    let state = app.state::<AppState>();
    for pa in proposal.proposed.iter_mut() {
        let Action::ConstantChange {
            constant_name,
            new_value,
            ..
        } = &pa.action
        else {
            continue;
        };
        if let Some(warning) =
            crate::commands::pin_conflicts::pin_conflict_warning(&state, constant_name, *new_value)
                .await
        {
            match &mut pa.validation {
                ValidationResult::Ok { warnings } => warnings.push(warning),
                // Failed proposals already block acceptance; an extra
                // warning would only duplicate noise.
                ValidationResult::Failed { .. } => {}
            }
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

/// Response for [`agent_apply_proposals`]: per-action results plus
/// batch-level warnings that only make sense across actions (e.g. the
/// accepted edits collectively shift a table's mean by a large amount),
/// and traceability fields (pre-apply restore point, git outcome).
#[derive(Debug, Serialize)]
pub struct ApplyProposalsResponse {
    pub results: Vec<ApplyResult>,
    pub batch_warnings: Vec<String>,
    /// Pre-apply restore point file name, when one could be created.
    pub restore_point: Option<String>,
    /// Commit sha when `auto_commit_on_save = "always"` committed the apply.
    pub auto_committed: Option<String>,
    /// Prepared commit message when `auto_commit_on_save = "ask"` — the
    /// frontend offers a one-click commit after the user saves.
    pub suggest_commit: Option<String>,
}

/// Apply a list of approved actions to the working tune.
///
/// Two phases:
///
/// 1. Every action is re-validated against the loaded definition (per-action
///    `ActionSet`); invalid ones are skipped with an error in the result.
/// 2. Validated actions are *applied*: table edits and bulk operations are
///    coalesced per table into a single read-modify-write through the same
///    internal path the table editors use, and constants go through
///    `update_constant_internal` (which also runs the pin-conflict guard for
///    bits constants).
///
/// **Nothing is burned to the ECU** — writes go to the working tune (and
/// ECU RAM when connected, exactly like a manual table edit), and the tune
/// is flagged modified so the user is prompted to burn afterward.
#[tauri::command]
pub async fn agent_apply_proposals(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: ApplyProposalsRequest,
) -> Result<ApplyProposalsResponse, String> {
    use libretune_core::action_scripting::{ActionMetadata, ActionPlayer, ActionSet};

    // --- Phase 1: re-validate every action (definition lock, read-only) ---
    let mut results: Vec<ApplyResult> = Vec::with_capacity(request.actions.len());
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
                Ok(_warnings) => results.push(ApplyResult {
                    applied: true,
                    error: None,
                    safety_tier: tier,
                }),
                Err(errors) => results.push(ApplyResult {
                    applied: false,
                    error: Some(errors.join("; ")),
                    safety_tier: tier,
                }),
            }
        }
    } // definition lock released here

    // --- Phase 2: apply the validated actions -----------------------------
    //
    // Table actions (TableEdit + BulkOperation) are grouped per table so a
    // batch of cell edits costs ONE read + ONE write per table — the same
    // coalescing a user dragging cells across the editor gets. A group fails
    // as a unit: if the read, the pure grid application, or the write errors,
    // every action in the group is marked failed with that error and the
    // table is left untouched (a partial grid is never written).

    // (table_name, action indexes) preserving first-appearance order.
    let mut table_groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, action) in request.actions.iter().enumerate() {
        if !results[i].applied {
            continue;
        }
        match action {
            Action::TableEdit { table_name, .. } | Action::BulkOperation { table_name, .. } => {
                match table_groups.iter_mut().find(|(n, _)| n == table_name) {
                    Some(group) => group.1.push(i),
                    None => table_groups.push((table_name.clone(), vec![i])),
                }
            }
            _ => {}
        }
    }

    let mut batch_warnings: Vec<String> = Vec::new();

    // --- Pre-apply restore point ------------------------------------------
    // Snapshot the tune BEFORE any write so the user can roll the whole
    // apply back from Restore Points. Only when something will actually be
    // written; failure is a warning, not a blocker (the writes themselves
    // are still validated and user-approved).
    let mut restore_point: Option<String> = None;
    if results.iter().any(|r| r.applied) {
        match crate::commands::restore_points::create_restore_point_internal(&state).await {
            Ok(rp) => restore_point = Some(rp.filename),
            Err(e) => {
                batch_warnings.push(format!("could not create a pre-apply restore point: {e}"))
            }
        }
    }

    for (table_name, indexes) in &table_groups {
        let outcome: Result<(), String> = async {
            let mut t =
                crate::commands::table_internals::get_table_data_internal(&state, table_name)
                    .await?;

            // Batch drift check (before mutating): individually-bounded edits
            // can still walk a whole table. Warn — do not block — when the
            // accepted cell edits for this table shift their mean by >10%.
            push_drift_warning(
                &mut batch_warnings,
                &t.z_values,
                table_name,
                indexes,
                &request.actions,
            );

            let actions: Vec<Action> = indexes
                .iter()
                .map(|&i| request.actions[i].clone())
                .collect();
            libretune_core::agent::apply_table_actions_to_grid(&mut t.z_values, &actions)?;
            crate::commands::table_internals::update_table_z_values_internal(
                &state, table_name, t.z_values,
            )
            .await
        }
        .await;

        if let Err(e) = outcome {
            for &i in indexes {
                results[i] = ApplyResult {
                    applied: false,
                    error: Some(e.clone()),
                    safety_tier: results[i].safety_tier,
                };
            }
        }
    }

    // Constants apply individually through the standard constant write path
    // (cache + tune mirror + optional ECU RAM write + pin-conflict guard).
    for (i, action) in request.actions.iter().enumerate() {
        if !results[i].applied {
            continue;
        }
        if let Action::ConstantChange {
            constant_name,
            new_value,
            ..
        } = action
        {
            if let Err(e) = crate::commands::constant_update::update_constant_internal(
                &state,
                constant_name.clone(),
                *new_value,
            )
            .await
            {
                results[i] = ApplyResult {
                    applied: false,
                    error: Some(e),
                    safety_tier: results[i].safety_tier,
                };
            }
        }
    }

    // The write paths above already set `tune_modified`; keep the explicit
    // flag so a future write path that forgets cannot silently skip the
    // "unsaved changes" prompt.
    if results.iter().any(|r| r.applied) {
        *state.tune_modified.lock().await = true;
    }

    // --- Git traceability (per auto_commit_on_save) ------------------------
    // "always": save the tune to the project file and commit — the AI
    // changes only exist on disk after a save, so committing without saving
    // would record the previous state. "ask": hand the frontend a prepared
    // commit message to offer after the user saves. "never": nothing.
    let mut auto_committed: Option<String> = None;
    let mut suggest_commit: Option<String> = None;
    if results.iter().any(|r| r.applied) {
        let settings = crate::load_settings(&app);
        let applied: Vec<Action> = results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.applied)
            .map(|(i, _)| request.actions[i].clone())
            .collect();
        let message = ai_commit_message(&applied);
        match settings.auto_commit_on_save.as_str() {
            "always" => {
                match crate::commands::project_tune_sync::save_tune_to_project_internal(&state)
                    .await
                {
                    Ok(()) => {
                        match crate::commands::git::commit_project_state(&state, &message).await {
                            Ok(sha) => auto_committed = Some(sha),
                            // "no git repository" is a skip, not a failure.
                            Err(e) if e.contains("no git repository") => {}
                            Err(e) => {
                                batch_warnings
                                    .push(format!("tune saved, but the auto-commit failed: {e}"));
                            }
                        }
                    }
                    Err(e) => {
                        batch_warnings.push(format!(
                            "auto-commit skipped — could not save the tune: {e}"
                        ));
                    }
                }
            }
            "ask" => suggest_commit = Some(message),
            _ => {}
        }
    }

    Ok(ApplyProposalsResponse {
        results,
        batch_warnings,
        restore_point,
        auto_committed,
        suggest_commit,
    })
}

/// Build the git commit message for an applied AI batch: change count plus
/// per-target tallies (e.g. "table:veTable1 × 12, const:crankingPct × 1").
fn ai_commit_message(actions: &[Action]) -> String {
    let targets = libretune_core::agent::context::group_actions_by_target(actions);
    let tally = targets
        .iter()
        .map(|(name, count)| format!("{name} ×{count}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "AI assistant: {} change{}{}",
        actions.len(),
        if actions.len() == 1 { "" } else { "s" },
        if tally.is_empty() {
            String::new()
        } else {
            format!(" ({tally})")
        }
    )
}

/// Append a warning when the accepted TableEdits for one table shift the
/// mean of the edited cells by more than 10% of their current mean.
/// Pure helper (unit-testable): takes the current grid, not app state.
fn push_drift_warning(
    warnings: &mut Vec<String>,
    z_values: &[Vec<f64>],
    table_name: &str,
    indexes: &[usize],
    actions: &[Action],
) {
    let mut sum_delta = 0.0;
    let mut sum_current = 0.0;
    let mut n = 0usize;
    for &i in indexes {
        let Action::TableEdit {
            x_index,
            y_index,
            new_value,
            ..
        } = &actions[i]
        else {
            continue;
        };
        let Some(current) = z_values
            .get(*y_index as usize)
            .and_then(|r| r.get(*x_index as usize))
        else {
            continue;
        };
        sum_delta += (new_value - current).abs();
        sum_current += current.abs();
        n += 1;
    }
    if n == 0 {
        return;
    }
    let mean_current = sum_current / n as f64;
    let mean_delta = sum_delta / n as f64;
    if mean_current > f64::MIN_POSITIVE && mean_delta > mean_current * 0.10 {
        warnings.push(format!(
            "table '{table_name}': the {n} accepted edits shift the edited cells' mean by {:.0}% (mean {:.2}, mean |Δ| {:.2}) — make sure that is intended",
            100.0 * mean_delta / mean_current,
            mean_current,
            mean_delta
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(x: u16, y: u16, new_value: f64) -> Action {
        Action::TableEdit {
            table_name: "veTable1".into(),
            x_index: x,
            y_index: y,
            new_value,
            old_value: None,
        }
    }

    #[test]
    fn drift_warning_fires_on_large_shift() {
        let z = vec![vec![100.0; 4]; 4];
        let actions = vec![edit(0, 0, 120.0), edit(1, 0, 125.0)];
        let mut warnings = Vec::new();
        push_drift_warning(&mut warnings, &z, "veTable1", &[0, 1], &actions);
        assert_eq!(warnings.len(), 1, "should warn: mean shift ~22%");
        assert!(warnings[0].contains("veTable1"));
    }

    #[test]
    fn drift_warning_silent_on_small_shift() {
        let z = vec![vec![100.0; 4]; 4];
        let actions = vec![edit(0, 0, 105.0), edit(1, 0, 103.0)];
        let mut warnings = Vec::new();
        push_drift_warning(&mut warnings, &z, "veTable1", &[0, 1], &actions);
        assert!(warnings.is_empty(), "4% shift should not warn");
    }

    #[test]
    fn drift_warning_ignores_bulk_ops_and_out_of_range() {
        let z = vec![vec![100.0; 2]; 2];
        let actions = vec![
            edit(9, 9, 500.0), // out of range — skipped, no panic
            Action::BulkOperation {
                operation: "scale".into(),
                table_name: "veTable1".into(),
                cells: vec![(0, 0)],
                parameters: Default::default(),
                old_values: None,
            },
        ];
        let mut warnings = Vec::new();
        push_drift_warning(&mut warnings, &z, "veTable1", &[0, 1], &actions);
        assert!(warnings.is_empty());
    }
}
