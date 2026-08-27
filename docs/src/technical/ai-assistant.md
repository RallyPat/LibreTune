# AI Assistant Architecture

This document describes the technical architecture of LibreTune's bring-your-own-LLM
AI Assistant. It is intended for developers contributing to or extending the feature.

## Design Principles

1. **Bring your own model** — LibreTune never hosts a model. The user supplies the
   provider, key, and model. This keeps the feature dependency-free and private.
2. **Propose, never apply** — the assistant's only output is a validated, clamped
   *proposal*. Application and burn are always separate, user-triggered steps.
3. **Reuse existing primitives** — the assistant is a thin layer over the existing
   `Action` enum, `validate_action_set`, AutoTune engines, and INI metadata. It does
   not duplicate safety logic.
4. **Provider-agnostic** — a single `Provider` trait abstracts all LLM backends. The
   orchestrator never sees provider-specific JSON.

## Module Layout

```
crates/libretune-core/src/
├── agent/
│   ├── mod.rs           # module root + re-exports
│   ├── orchestrator.rs  # multi-turn agent loop + ReadToolExecutor/TurnObserver traits
│   ├── tools.rs         # tool catalogue, CapabilityTier, catalogue_for_tier()
│   ├── context.rs       # context-gathering helpers (constants, tables)
│   ├── summarize.rs     # summarize_tune_context() aggregation
│   ├── safety.rs        # authority-limit clamping
│   ├── tiers.rs         # constant safety tiering (Safe/Caution/Dangerous)
│   └── apply.rs         # pure grid application of approved table actions
└── llm/
    ├── mod.rs           # module root
    ├── types.rs         # ChatRequest/ChatResponse/Message/ToolCall/LlmError
    ├── provider.rs      # Provider trait + factory
    ├── client.rs        # LlmClient (top-level entry point)
    └── providers/
        ├── openai.rs    # OpenAI Chat Completions (native protocol)
        ├── anthropic.rs # Anthropic Messages API (native protocol)
        └── google.rs    # Google Gemini generateContent (native protocol)
```

The Tauri layer wraps these in `crates/libretune-app/src-tauri/src/commands/agent.rs`
(commands: `agent_status`, `agent_send_message`, `agent_apply_proposals`),
plus `commands/ai_keychain.rs` (OS-keychain storage for the API key).

## The Agent Loop

One user turn runs a **multi-turn loop** inside `orchestrator::run_turn`
(delegating to `run_turn_observed`):

```
user message
     │
     ▼
┌─────────────────────────────────────────────┐
│  build ChatRequest (system + history + msg) │
│  + catalogue_for_tier(capability_tier)       │
└──────────────────────┬──────────────────────┘
                       ▼
              call Provider::chat
                       │
        ┌──────────────┴───────────────┐
        ▼                              ▼
  read tool calls?              propose tool calls?
        │                              │
        ▼                              ▼
  execute via                 tier gate → map → Action[]
  ReadToolExecutor            validate_action_set
  (TurnObserver emits          clamp to authority
   agent:progress)            accumulate into Proposal
        │                              │
        ▼                              ▼
  append tool-result                   │
  messages to history                  │
        │                              │
        ▼                              ▼
  loop back ─────────────►  (no reads left? → done)
                                   │
                                   ▼
                            return Proposal
```

The loop is bounded by `MAX_READ_ROUNDS` (6) to cap cost and prevent runaway
conversations. Read results are fed back as tool-result messages so the model can
reason over actual table/constant data before emitting its final reply.

An optional `TurnObserver` (`on_model_call` / `on_read_tool` / `on_complete`)
lets the command layer surface progress without the core knowing about Tauri
— `agent_send_message` implements it by emitting `agent:progress` events that
the chat panel renders as live "reading veTable1…" activity.

## The Provider Trait

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError>;
}
```

Each concrete provider translates the generic `ChatRequest` to its wire format
(OpenAI `tools[]`, Anthropic `tool_use` blocks, Gemini `functionDeclarations`),
calls its endpoint via the shared `reqwest` client, and parses the response back
into a generic `ChatResponse`. Adding a new provider requires only implementing
this trait — no orchestrator changes.

## The ReadToolExecutor Trait

```rust
#[async_trait]
pub trait ReadToolExecutor: Send + Sync {
    fn handles(&self, tool_name: &str) -> bool;
    async fn execute(&self, tool_name: &str, arguments: &str) -> String;
}
```

The core library defines this contract; the Tauri layer implements it as
`LiveReadExecutor`, which reaches `AppState` via `tauri::Manager::state()` and
reads tables/constants against the loaded definition and tune. This split keeps
the loop unit-testable without a live provider or ECU.

## Tool Catalogue

Defined in `agent/tools.rs`. The model may call:

| Tool | Type | Purpose |
|------|------|---------|
| `list_tables` | read | Discover table names + roles + dimensions |
| `read_table` | read | Get a table's values, axis bins, units |
| `read_constant` | read | Get a constant's value, min/max, options |
| `list_features` | read | List feature-toggle (bits) constants |
| `summarize_tune_context` | read | Aggregated coverage + AFR error + anomalies |
| `tune_health_check` | read | Per-region health scores |
| `get_realtime_snapshot` | read | Current sensor values (one ECU poll, curated) |
| `query_datalog` | read | Summary stats / tail rows over a session or saved log |
| `propose_table_edit` | propose | Stage a single-cell edit (reviewed + clamped) |
| `propose_bulk_operation` | propose | Stage scale/smooth/interpolate (reviewed) |
| `propose_constant_change` | propose | Stage a constant change (tier-flagged) |

### Capability Tiers

`tools::CapabilityTier` (`Read` ⊂ `Tune` ⊂ `Config`, parsed from the
`ai_capability_tier` setting; unknown values collapse to `Read`) controls what
the model may call:

- **Read** — read tools only.
- **Tune** — read + `propose_table_edit` / `propose_bulk_operation`.
- **Config** — tune + `propose_constant_change`.

Enforcement is two-layered: `catalogue_for_tier()` filters the tools attached
to every request (so the model is never offered out-of-tier tools), and
`map_tool_call` hard-rejects any out-of-tier propose call (defense in depth
against hallucinated tool calls).

### The Apply Path

`agent_apply_proposals` validates and then **applies** approved actions
(neither the orchestrator nor the model can):

1. Every action is re-validated against the loaded definition (per-action
   `ActionSet`); failures are skipped with an error.
2. A **restore point** is created before any write (`create_restore_point_internal`).
3. Table actions are grouped per table: one read
   (`get_table_data_internal`) → pure application via core
   `agent::apply::apply_table_actions_to_grid` (converts `(x, y)` to
   `(row, col)`, dispatches bulk ops to `table_ops`) → one write
   (`update_table_z_values_internal`, the same path the table editors use:
   cache + tune mirror + optional ECU RAM write). A group fails as a unit.
4. Constants go through `update_constant_internal`, which also runs the
   pin-conflict guard for bits constants.
5. Per-table **drift warnings** fire when accepted edits shift the edited
   cells' mean by more than 10%.
6. Per `auto_commit_on_save`: *always* saves the tune and commits (never
   auto-initializes a repo); *ask* returns a prepared commit message.

**Nothing ever burns.** Staged changes reach ECU RAM exactly like a manual
table edit; burning stays a separate user action.

## TableRole Inference

`EcuDefinition::infer_table_roles()` attaches a machine-readable `TableRole`
enum (`Ve`, `Ignition`, `AfrTarget`, `WarmupEnrichment`, `Other`) to every
`TableDefinition`, derived from the INI's `[VeAnalyze]` and `[WueAnalyze]`
sections. This lets the assistant know what a table *does* without guessing from
its name.

## Validation Extensions

The assistant motivated extending `ActionPlayer::validate_action_set` beyond
existence checks. It now validates:

- **Constant `min`/`max`** (display-unit bounds)
- **`DataType` raw storage range** (pre-scale)
- **Table cell-index bounds** (`x_index`/`y_index` vs `x_size`/`y_size`)
- **Bits-type enumeration** (value must be a valid option index)

## Frontend

- `components/agent/AgentSidePanel.tsx` — the docked right-hand panel (header,
  resize handle, pop-out/collapse buttons, collapsible review queue, chat
  switcher, per-chat token counter).
- `components/agent/ChatPanel.tsx` — the conversational transcript + input;
  listens for `agent:progress` (live activity), sends `unit_prefs` per turn,
  and accepts `extraContext` / `prefill` for the Ask AI flow.
- `components/agent/ProposalQueue.tsx` — the per-item review surface; renders
  pin-conflict and batch warnings and the apply outcome note.
- `components/tables/TableToolbar.tsx` — the **Ask AI** button; emits the
  `agent:ask` event with table/title/axes/selection. `App.tsx` opens the
  panel on that event; `AgentSidePanel` injects the context into the next
  turn's system prompt and pre-fills the input.
- `components/common/RiskAcknowledgement.tsx` — reusable risk-ack primitive.
- The panel can pop out via the existing `WebviewWindow` + hash-routing system
  (see `PopOutWindow.tsx`, type `agent`).

## API Key Storage

`commands/ai_keychain.rs` stores the API key in the OS keychain (`keyring`
crate: Windows Credential Manager, macOS Keychain, Linux Secret Service).
Plaintext keys found in the settings file migrate to the keychain on first
load; saves blank the file copy whenever the keychain holds the key. Every
operation degrades gracefully — with no keychain backend (headless Linux,
locked sessions) the previous plaintext-file behavior is preserved so the
assistant never breaks.

## Error Handling

- The core layer uses a typed `LlmError` enum (`Network`, `Auth`, `RateLimit`,
  `Parse`, `ApiError`, `Config`).
- Tauri commands flatten to `Result<T, String>` at the boundary.
- Settings saves are per-setting (one failure does not abort the others).
- Read-tool failures are returned to the *model* as `{"error": …}` JSON so it
  can fall back to reasoning; a wrong datalog name additionally lists the
  available logs so the model can retry.
