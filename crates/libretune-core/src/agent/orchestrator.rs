//! The per-turn agent orchestrator.
//!
//! Pipeline for one user turn:
//!   1. Gather context (caller-supplied snapshot of tune + realtime state).
//!   2. Build a [`ChatRequest`] with the system prompt, history, and the
//!      [`crate::agent::tools::catalogue`].
//!   3. Call the [`LlmClient`].
//!   4. Map the model's [`ToolCall`]s into [`Action`]s.
//!   5. Validate via [`ActionPlayer::validate_action_set`].
//!   6. Clamp table edits to [`AutoTuneAuthorityLimits`].
//!   7. Return a [`Proposal`] for the UI review queue.
//!
//! Nothing here applies anything. The orchestrator only *produces* a proposal;
//! application is a separate, user-triggered step.

use crate::action_scripting::{Action, ActionMetadata, ActionPlayer, ActionSet};
use crate::agent::safety::clamp_table_edit;
use crate::agent::tiers::{constant_safety_tier, ConstantSafetyTier};
use crate::agent::tools::{self, CapabilityTier};
use crate::autotune::AutoTuneAuthorityLimits;
use crate::llm::types::{ChatRequest, FinishReason, LlmError, Message, ToolCall};
use crate::llm::LlmClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One proposed change, ready for the review queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedAction {
    /// The underlying action to apply if approved.
    pub action: Action,
    /// Safety tier (only meaningful for constant changes).
    pub safety_tier: ConstantSafetyTier,
    /// Validation outcome: warnings if it passed, errors if it failed.
    pub validation: ValidationResult,
    /// If clamped to authority limits, the original requested value.
    pub clamped_from: Option<f64>,
    /// Why the clamp happened (if it did).
    pub clamp_reason: Option<String>,
    /// Free-text reason the model gave (from the tool call's `reason` arg).
    pub reason: Option<String>,
}

/// Validation outcome for one proposed action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ValidationResult {
    /// Passed with non-fatal warnings (possibly empty).
    Ok { warnings: Vec<String> },
    /// Failed validation — must not be applied. Surfaced for the user to see
    /// what the model got wrong.
    Failed { errors: Vec<String> },
}

/// A complete proposal for one turn: the assistant's text reply plus the
/// proposed actions (some of which may have failed validation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// The model's natural-language reply (its explanation to the user).
    pub reply: String,
    /// Why the turn ended (tool_calls / stop / length / ...).
    pub finish_reason: String,
    /// Proposed actions, in the order the model emitted them.
    pub proposed: Vec<ProposedAction>,
    /// Whether every proposed action passed validation.
    pub all_valid: bool,
    /// Model-reported token usage (when available).
    pub usage: Option<crate::llm::types::Usage>,
}

/// Inputs the orchestrator needs that it cannot fetch itself (kept
/// provider-agnostic and I/O-free so the loop is unit-testable).
#[derive(Debug, Clone, Default)]
pub struct OrchestratorInputs {
    /// Conversation history *before* this turn's user message.
    pub history: Vec<Message>,
    /// The user's message this turn.
    pub user_message: String,
    /// A pre-rendered system prompt describing the tune/ECU context. Built by
    /// [`crate::agent::context`] from the live ECU state.
    pub system_prompt: String,
    /// Per-cell current values for tables the model might edit, keyed by
    /// table name → `(x,y)` → value. Used for authority clamping.
    pub current_table_values: HashMap<String, HashMap<(u16, u16), f64>>,
    /// What the model may do this turn (read / tune / config). Filters the
    /// tool catalogue and gates propose-tool mapping. Defaults to the most
    /// restrictive tier.
    pub capability_tier: CapabilityTier,
}

/// Executes read-only tool calls against the live ECU/tune state.
///
/// When the model emits a read tool (e.g. `read_table`, `list_tables`,
/// `summarize_tune_context`), the orchestrator hands it here and feeds the
/// returned JSON string back to the model as a tool-result message, then calls
/// the model again — closing the loop so "let me look at your VE table" can
/// actually return an analysis.
///
/// Implementations live in the Tauri layer (which has access to `AppState`);
/// the core library only defines the contract so the loop is testable without
/// a live provider or ECU.
#[async_trait::async_trait]
pub trait ReadToolExecutor: Send + Sync {
    /// Returns `true` if this executor handles the named tool.
    fn handles(&self, tool_name: &str) -> bool;

    /// Execute one read tool call, returning a JSON string to feed back to the
    /// model. The string is inserted verbatim into a tool-result message.
    async fn execute(&self, tool_name: &str, arguments: &str) -> String;
}

/// Progress callbacks for one assistant turn.
///
/// The loop is opaque from the outside — a turn can span several
/// model-call → read-tool → model-call rounds, each taking seconds. The
/// observer lets the command layer surface "reading veTable1…" activity
/// without the core knowing anything about Tauri events.
pub trait TurnObserver: Send + Sync {
    /// Called before each model call. `round` counts from 0.
    fn on_model_call(&self, _round: usize) {}

    /// Called before executing a read tool call.
    fn on_read_tool(&self, _round: usize, _tool_name: &str, _arguments: &str) {}

    /// Called once with the model's accumulated proposal count before the
    /// final return (useful for "{n} proposals ready" feedback).
    fn on_complete(&self, _proposal_count: usize) {}
}

/// Maximum number of read→respond round-trips before forcing the loop to stop.
/// Caps runaway loops and cost.
const MAX_READ_ROUNDS: usize = 6;

/// Run one user turn to completion. Does not mutate any ECU state.
///
/// This loops: it calls the model, and if the model emits **read** tool calls
/// it executes them (via `read_executor`) and calls the model again with the
/// results, until the model emits a final text reply or only propose tool
/// calls. Propose calls accumulate into the returned [`Proposal`].
pub async fn run_turn(
    client: &LlmClient,
    inputs: &OrchestratorInputs,
    authority: &AutoTuneAuthorityLimits,
    read_executor: Option<&dyn ReadToolExecutor>,
) -> Result<Proposal, LlmError> {
    run_turn_observed(client, inputs, authority, read_executor, None).await
}

/// [`run_turn`] with an optional [`TurnObserver`] for progress reporting.
pub async fn run_turn_observed(
    client: &LlmClient,
    inputs: &OrchestratorInputs,
    authority: &AutoTuneAuthorityLimits,
    read_executor: Option<&dyn ReadToolExecutor>,
    observer: Option<&dyn TurnObserver>,
) -> Result<Proposal, LlmError> {
    // 1. Assemble the initial request.
    let mut messages: Vec<Message> = Vec::with_capacity(inputs.history.len() + 2);
    messages.push(Message::system(&inputs.system_prompt));
    messages.extend(inputs.history.iter().cloned());
    messages.push(Message::user(&inputs.user_message));

    // Accumulators across rounds.
    let mut proposed: Vec<ProposedAction> = Vec::new();
    let mut all_valid = true;
    let mut last_usage: Option<crate::llm::types::Usage> = None;
    let mut last_reply = String::new();
    let mut last_finish_reason = FinishReason::Stop;

    // 2. Multi-turn loop: read tools are executed and fed back; propose tools
    //    accumulate. Bounded by MAX_READ_ROUNDS to cap cost/runaways.
    for _round in 0..=MAX_READ_ROUNDS {
        if let Some(obs) = observer {
            obs.on_model_call(_round);
        }
        let req = ChatRequest::new(messages.clone())
            .with_tools(tools::catalogue_for_tier(inputs.capability_tier));
        let resp = client.chat(&req).await?;

        last_reply = resp.content.clone();
        last_finish_reason = resp.finish_reason.clone();
        if resp.usage.is_some() {
            last_usage = resp.usage.clone();
        }

        // Partition tool calls into reads (need execution) and proposes
        // (become review-queue items).
        let (reads, proposes): (Vec<&ToolCall>, Vec<&ToolCall>) = resp
            .tool_calls
            .iter()
            .partition(|tc| tools::is_read_tool(&tc.name));

        // Map propose calls into ProposedActions.
        for tc in &proposes {
            let mapped = map_tool_call(tc, inputs, authority);
            if matches!(mapped.validation, ValidationResult::Failed { .. }) {
                all_valid = false;
            }
            proposed.push(mapped);
        }

        // If there are no read calls, this round is done — the model either
        // emitted a plain reply or only proposes.
        if reads.is_empty() {
            break;
        }

        // Append the assistant's tool-call message to the history so the model
        // sees what it asked for, then append a tool-result message for each
        // read call.
        messages.push(Message {
            role: crate::llm::types::MessageRole::Assistant,
            content: resp.content.clone(),
            tool_calls: resp.tool_calls.clone(),
            tool_name: None,
        });

        for tc in &reads {
            if let Some(obs) = observer {
                obs.on_read_tool(_round, &tc.name, &tc.arguments);
            }
            let result = match read_executor {
                Some(ex) if ex.handles(&tc.name) => ex.execute(&tc.name, &tc.arguments).await,
                _ => {
                    // No executor available — tell the model the read failed so
                    // it can fall back to reasoning instead of stalling.
                    format!(
                        "{{\"error\":\"no executor available for read tool '{}'; cannot fetch live data\"}}",
                        tc.name
                    )
                }
            };
            messages.push(Message {
                role: crate::llm::types::MessageRole::Tool,
                content: result,
                tool_calls: Vec::new(),
                tool_name: Some(tc.name.clone()),
            });
        }

        // If we're about to hit the round cap, tell the model to wrap up —
        // this must fire one round early (MAX_READ_ROUNDS - 1) so the
        // following iteration still has a `client.chat()` call left in the
        // `0..=MAX_READ_ROUNDS` bound to actually send it. Firing on
        // `_round == MAX_READ_ROUNDS` composes the message on the loop's
        // final iteration, which then exits without ever calling the model
        // again — the wrap-up request is built but never sent.
        if _round == MAX_READ_ROUNDS - 1 {
            messages.push(Message::user(
                "I've gathered enough data. Please give me your final analysis and any proposed changes.",
            ));
        }
    }

    if let Some(obs) = observer {
        obs.on_complete(proposed.len());
    }

    Ok(Proposal {
        reply: last_reply,
        finish_reason: finish_reason_str(&last_finish_reason),
        proposed,
        all_valid,
        usage: last_usage,
    })
}

fn finish_reason_str(fr: &FinishReason) -> String {
    match fr {
        FinishReason::Stop => "stop".into(),
        FinishReason::ToolCalls => "tool_calls".into(),
        FinishReason::Length => "length".into(),
        FinishReason::ContentFilter => "content_filter".into(),
        FinishReason::Other(s) => s.clone(),
    }
}

/// Turn one model [`ToolCall`] into a [`ProposedAction`]. Propose-tools map
/// to an [`Action`]; read-tools are noted but not applied (the orchestrator
/// only proposes — reads are answered out-of-band by the command layer).
fn map_tool_call(
    tc: &ToolCall,
    inputs: &OrchestratorInputs,
    authority: &AutoTuneAuthorityLimits,
) -> ProposedAction {
    // Tier gate (defense in depth). The catalogue attached to the request
    // already omits tools above the configured tier, but a model can still
    // hallucinate a disallowed call — reject it explicitly rather than
    // mapping it to an action.
    if !inputs.capability_tier.allows(&tc.name) {
        return failed(
            Action::Pause { duration_ms: 0 },
            vec![format!(
                "tool '{}' is not permitted at capability tier {:?}",
                tc.name, inputs.capability_tier
            )],
            tc,
        );
    }

    let args: serde_json::Value = match serde_json::from_str(&tc.arguments) {
        Ok(v) => v,
        Err(e) => {
            return failed(
                Action::Pause { duration_ms: 0 },
                vec![format!("could not parse tool arguments: {e}")],
                tc,
            );
        }
    };

    match tc.name.as_str() {
        tools::tool_names::PROPOSE_TABLE_EDIT => map_table_edit(&args, inputs, authority, tc)
            .unwrap_or_else(|errs| failed(Action::Pause { duration_ms: 0 }, errs, tc)),
        tools::tool_names::PROPOSE_CONSTANT_CHANGE => map_constant_change(&args, tc)
            .unwrap_or_else(|errs| failed(Action::Pause { duration_ms: 0 }, errs, tc)),
        tools::tool_names::PROPOSE_BULK_OP => map_bulk_op(&args, tc)
            .unwrap_or_else(|errs| failed(Action::Pause { duration_ms: 0 }, errs, tc)),
        // Read tools: they don't produce an action; surface as a no-op note.
        // The command layer answers read calls by feeding results back into
        // the next turn's history.
        _ => ProposedAction {
            action: Action::Pause { duration_ms: 0 },
            safety_tier: ConstantSafetyTier::Safe,
            validation: ValidationResult::Ok {
                warnings: vec![format!("read tool '{}' answered out-of-band", tc.name)],
            },
            clamped_from: None,
            clamp_reason: None,
            reason: Some(format!("read: {}", tc.name)),
        },
    }
}

fn map_table_edit(
    args: &serde_json::Value,
    inputs: &OrchestratorInputs,
    authority: &AutoTuneAuthorityLimits,
    _tc: &ToolCall,
) -> Result<ProposedAction, Vec<String>> {
    let table_name = get_str(args, "table_name")?;
    let x_index = get_u16(args, "x_index")?;
    let y_index = get_u16(args, "y_index")?;
    let new_value = get_f64(args, "new_value")?;
    let reason = get_str(args, "reason").ok();

    // Clamp to authority limits (needs current value).
    let current = inputs
        .current_table_values
        .get(&table_name)
        .and_then(|m| m.get(&(x_index, y_index)).copied());
    let action = Action::TableEdit {
        table_name: table_name.clone(),
        x_index,
        y_index,
        new_value,
        old_value: current,
    };
    let clamped = clamp_table_edit(action, authority, current);

    // Validate the (possibly clamped) action.
    let set = single_action_set(clamped.action.clone());
    let validation = match ActionPlayer::validate_action_set(&set, None) {
        Ok(w) => ValidationResult::Ok { warnings: w },
        Err(e) => ValidationResult::Failed { errors: e },
    };

    Ok(ProposedAction {
        action: clamped.action,
        safety_tier: ConstantSafetyTier::Caution,
        validation,
        clamped_from: clamped.clamped_from,
        clamp_reason: clamped.reason,
        reason,
    })
}

fn map_constant_change(
    args: &serde_json::Value,
    _tc: &ToolCall,
) -> Result<ProposedAction, Vec<String>> {
    let name = get_str(args, "name")?;
    let value = get_f64(args, "value")?;
    let reason = get_str(args, "reason").ok();
    let tier = constant_safety_tier(&name);

    let action = Action::ConstantChange {
        constant_name: name,
        new_value: value,
        old_value: None,
    };
    let set = single_action_set(action.clone());
    let validation = match ActionPlayer::validate_action_set(&set, None) {
        Ok(w) => ValidationResult::Ok { warnings: w },
        Err(e) => ValidationResult::Failed { errors: e },
    };

    Ok(ProposedAction {
        action,
        safety_tier: tier,
        validation,
        clamped_from: None,
        clamp_reason: None,
        reason,
    })
}

fn map_bulk_op(args: &serde_json::Value, _tc: &ToolCall) -> Result<ProposedAction, Vec<String>> {
    let table_name = get_str(args, "table_name")?;
    let operation = get_str(args, "operation")?;
    let reason = get_str(args, "reason").ok();

    let cells_arr = args
        .get("cells")
        .and_then(|v| v.as_array())
        .ok_or_else(|| vec!["missing 'cells' array".to_string()])?;
    let mut cells: Vec<(u16, u16)> = Vec::with_capacity(cells_arr.len());
    for c in cells_arr {
        let arr = c
            .as_array()
            .ok_or_else(|| vec!["cell must be [x,y]".to_string()])?;
        if arr.len() < 2 {
            return Err(vec!["cell must have two elements".to_string()]);
        }
        let x = arr[0]
            .as_u64()
            .ok_or_else(|| vec!["cell x not integer".to_string()])? as u16;
        let y = arr[1]
            .as_u64()
            .ok_or_else(|| vec!["cell y not integer".to_string()])? as u16;
        cells.push((x, y));
    }

    let parameters: HashMap<String, f64> = args
        .get("parameters")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f)))
                .collect()
        })
        .unwrap_or_default();

    let action = Action::BulkOperation {
        operation,
        table_name,
        cells,
        parameters,
        old_values: None,
    };
    let set = single_action_set(action.clone());
    let validation = match ActionPlayer::validate_action_set(&set, None) {
        Ok(w) => ValidationResult::Ok { warnings: w },
        Err(e) => ValidationResult::Failed { errors: e },
    };

    Ok(ProposedAction {
        action,
        safety_tier: ConstantSafetyTier::Caution,
        validation,
        clamped_from: None,
        clamp_reason: None,
        reason,
    })
}

fn failed(action: Action, errors: Vec<String>, tc: &ToolCall) -> ProposedAction {
    ProposedAction {
        action,
        safety_tier: ConstantSafetyTier::Caution,
        validation: ValidationResult::Failed { errors },
        clamped_from: None,
        clamp_reason: None,
        reason: Some(format!("tool '{}'", tc.name)),
    }
}

// --- small helpers -------------------------------------------------------

fn single_action_set(action: Action) -> ActionSet {
    ActionSet {
        id: "proposal".into(),
        name: "AI proposal".into(),
        description: "Single-action proposal from the assistant".into(),
        version: "1".into(),
        actions: vec![action],
        metadata: ActionMetadata {
            created_by: "ai-assistant".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            modified_at: chrono::Utc::now().to_rfc3339(),
            tags: vec!["ai-proposal".into()],
            compatible_ecus: vec![],
        },
    }
}

fn get_str(v: &serde_json::Value, key: &str) -> Result<String, Vec<String>> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| vec![format!("missing or non-string '{key}'")])
}

fn get_f64(v: &serde_json::Value, key: &str) -> Result<f64, Vec<String>> {
    v.get(key)
        .and_then(|x| x.as_f64())
        .ok_or_else(|| vec![format!("missing or non-numeric '{key}'")])
}

fn get_u16(v: &serde_json::Value, key: &str) -> Result<u16, Vec<String>> {
    v.get(key)
        .and_then(|x| x.as_u64())
        .map(|n| n as u16)
        .ok_or_else(|| vec![format!("missing or non-integer '{key}'")])
}

// Extend ChatRequest with a fluent .with_tools helper (local to this module
// to avoid adding a public builder for now).
trait ChatRequestExt {
    fn with_tools(self, tools: Vec<crate::llm::types::ToolDef>) -> Self;
}
impl ChatRequestExt for ChatRequest {
    fn with_tools(mut self, tools: Vec<crate::llm::types::ToolDef>) -> Self {
        self.tools = tools;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth() -> AutoTuneAuthorityLimits {
        AutoTuneAuthorityLimits {
            max_cell_value_change: 5.0,
            max_cell_percentage_change: 10.0,
            ..Default::default()
        }
    }

    /// Inputs unlocked for everything — most mapping tests want the propose
    /// tools available; the tier gate itself is covered separately below.
    fn unlocked_inputs() -> OrchestratorInputs {
        OrchestratorInputs {
            capability_tier: CapabilityTier::Config,
            ..Default::default()
        }
    }

    #[test]
    fn maps_table_edit_and_clamps() {
        let tc = ToolCall {
            id: "1".into(),
            name: tools::tool_names::PROPOSE_TABLE_EDIT.into(),
            arguments: r#"{"table_name":"veTable1","x_index":0,"y_index":0,"new_value":60.0}"#
                .into(),
        };
        let mut inputs = unlocked_inputs();
        inputs
            .current_table_values
            .entry("veTable1".into())
            .or_default()
            .insert((0, 0), 50.0);
        let pa = map_tool_call(&tc, &inputs, &auth());
        match pa.action {
            Action::TableEdit { new_value, .. } => {
                // 50 -> 60 clamped to 55 (per-cell limit 5).
                assert!((new_value - 55.0).abs() < 1e-9);
            }
            _ => panic!("expected TableEdit"),
        }
        assert!(pa.clamped_from.is_some());
    }

    #[test]
    fn maps_constant_change_with_tier() {
        let tc = ToolCall {
            id: "1".into(),
            name: tools::tool_names::PROPOSE_CONSTANT_CHANGE.into(),
            arguments: r#"{"name":"fanOutputPin","value":7}"#.into(),
        };
        let pa = map_tool_call(&tc, &unlocked_inputs(), &auth());
        assert_eq!(pa.safety_tier, ConstantSafetyTier::Dangerous);
        match pa.validation {
            ValidationResult::Ok { .. } => {}
            ValidationResult::Failed { errors } => panic!("should pass: {errors:?}"),
        }
    }

    #[test]
    fn invalid_args_surface_as_failed() {
        let tc = ToolCall {
            id: "1".into(),
            name: tools::tool_names::PROPOSE_TABLE_EDIT.into(),
            arguments: r#"{"table_name":"veTable1"}"#.into(), // missing x_index etc
        };
        let pa = map_tool_call(&tc, &unlocked_inputs(), &auth());
        assert!(matches!(pa.validation, ValidationResult::Failed { .. }));
    }

    #[test]
    fn tier_gate_rejects_propose_tools_below_tier() {
        // Read tier: every propose tool is rejected, even with valid args.
        for name in [
            tools::tool_names::PROPOSE_TABLE_EDIT,
            tools::tool_names::PROPOSE_BULK_OP,
            tools::tool_names::PROPOSE_CONSTANT_CHANGE,
        ] {
            let tc = ToolCall {
                id: "1".into(),
                name: name.into(),
                arguments: r#"{"table_name":"veTable1","x_index":0,"y_index":0,"new_value":60.0,"name":"x","value":1,"operation":"scale","cells":[[0,0]]}"#.into(),
            };
            let pa = map_tool_call(&tc, &OrchestratorInputs::default(), &auth());
            match pa.validation {
                ValidationResult::Failed { errors } => {
                    assert!(
                        errors.iter().any(|e| e.contains("not permitted")),
                        "expected tier rejection for {name}: {errors:?}"
                    );
                }
                ValidationResult::Ok { .. } => panic!("{name} should be rejected at Read tier"),
            }
        }
    }

    #[test]
    fn tier_gate_tune_rejects_constant_proposals() {
        let tc = ToolCall {
            id: "1".into(),
            name: tools::tool_names::PROPOSE_CONSTANT_CHANGE.into(),
            arguments: r#"{"name":"crankingPct","value":12}"#.into(),
        };
        let inputs = OrchestratorInputs {
            capability_tier: CapabilityTier::Tune,
            ..Default::default()
        };
        let pa = map_tool_call(&tc, &inputs, &auth());
        assert!(matches!(pa.validation, ValidationResult::Failed { .. }));
    }

    #[test]
    fn tier_gate_allows_permitted_tools() {
        // Tune tier still maps table edits normally.
        let tc = ToolCall {
            id: "1".into(),
            name: tools::tool_names::PROPOSE_TABLE_EDIT.into(),
            arguments: r#"{"table_name":"veTable1","x_index":0,"y_index":0,"new_value":51.0}"#
                .into(),
        };
        let inputs = OrchestratorInputs {
            capability_tier: CapabilityTier::Tune,
            ..Default::default()
        };
        let pa = map_tool_call(&tc, &inputs, &auth());
        assert!(matches!(pa.validation, ValidationResult::Ok { .. }));
    }

    /// A [`Provider`](crate::llm::provider::Provider) that always requests a
    /// read tool call (so `run_turn_observed`'s loop never exits early via
    /// `reads.is_empty()`) and records every [`ChatRequest`] it was handed,
    /// so the test below can inspect exactly what the final round sent.
    struct AlwaysReadsProvider {
        requests: std::sync::Arc<std::sync::Mutex<Vec<ChatRequest>>>,
    }

    #[async_trait::async_trait]
    impl crate::llm::provider::Provider for AlwaysReadsProvider {
        fn name(&self) -> &str {
            "always-reads-mock"
        }

        async fn chat(
            &self,
            req: &ChatRequest,
        ) -> Result<crate::llm::types::ChatResponse, LlmError> {
            self.requests.lock().unwrap().push(req.clone());
            Ok(crate::llm::types::ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: tools::tool_names::LIST_TABLES.into(),
                    arguments: "{}".into(),
                }],
                finish_reason: FinishReason::ToolCalls,
                usage: None,
            })
        }
    }

    /// Answers every read tool call with an empty JSON object — its content
    /// doesn't matter for this test, only that the loop keeps running.
    struct StubReadExecutor;

    #[async_trait::async_trait]
    impl ReadToolExecutor for StubReadExecutor {
        fn handles(&self, _tool_name: &str) -> bool {
            true
        }

        async fn execute(&self, _tool_name: &str, _arguments: &str) -> String {
            "{}".to_string()
        }
    }

    /// Regression test for the round-cap wrap-up bug: previously the
    /// "please wrap up" message was appended to `messages` only after the
    /// loop's final `client.chat()` call had already happened, so it was
    /// composed but never actually sent. With the fix (guard moved to
    /// `MAX_READ_ROUNDS - 1`), the model keeps requesting reads every round,
    /// so the loop runs all `MAX_READ_ROUNDS + 1` rounds, and the last
    /// request sent to the client must contain the wrap-up instruction.
    #[tokio::test]
    async fn wrap_up_message_is_actually_sent_to_client_at_round_cap() {
        let recorded: std::sync::Arc<std::sync::Mutex<Vec<ChatRequest>>> = Default::default();
        let provider = AlwaysReadsProvider {
            requests: recorded.clone(),
        };
        let client = LlmClient::from_provider(Box::new(provider));

        let inputs = OrchestratorInputs {
            capability_tier: CapabilityTier::Config,
            ..Default::default()
        };

        let result = run_turn(&client, &inputs, &auth(), Some(&StubReadExecutor)).await;
        assert!(result.is_ok(), "run_turn should not error: {result:?}");

        let calls = recorded.lock().unwrap();
        assert_eq!(
            calls.len(),
            MAX_READ_ROUNDS + 1,
            "expected one client.chat() call per round, including the final round-cap round"
        );

        let last_request = calls.last().expect("at least one call recorded");
        let sent_wrap_up = last_request
            .messages
            .iter()
            .any(|m| m.content.contains("I've gathered enough data"));
        assert!(
            sent_wrap_up,
            "expected the final round's request to actually include the wrap-up instruction, got: {:#?}",
            last_request.messages
        );
    }
}
