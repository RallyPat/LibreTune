# AI Assistant

LibreTune includes an optional **AI Assistant** that can help you tune and
configure your ECU. It is a **bring-your-own-LLM** feature: you supply the
provider, API key, and model (OpenAI, Anthropic, Google, or any
OpenAI-compatible local endpoint such as Ollama), and the assistant acts as a
co-pilot.

> ⚠️ **At your own risk.** The assistant can propose changes that affect
> engine behavior. It only ever *proposes*; nothing is applied or burned
> automatically. See [Safety Model](#safety-model) below.

## Overview

The assistant lives in a **docked side panel** on the right side of the window
(similar to VS Code's chat), so you can chat with it while still viewing and
editing tables and dashboards. It can also be **popped out** into its own
window for multi-monitor setups.

What the assistant can do:

- **Explain** tables, constants, and gauges in plain English.
- **Diagnose** problems from tune data and table health.
- **Read tables live** and analyze them in the same conversation — when it says
  "let me look at your VE table," it actually reads the data and comes back with
  an analysis.
- **Summarize tune health** — data coverage, suspect cells, unexplored regions
  the assistant could estimate, and per-region health scores for a table.
- **See live sensor data** — a realtime snapshot of RPM, MAP, TPS, CLT, AFR and
  other channels, so you can ask "what's happening right now?"
- **Analyze datalogs** — per-channel statistics or recent rows from the current
  logging session or a saved log, so you can ask "here's my drive log, what
  should I change?"
- **Propose ECU configuration changes** (feature enablement, scalar constants).
- **Propose tune edits** to individual cells or bulk regions.

The assistant is **not** a closed-loop controller. It does not run every
realtime tick — that remains the job of the algorithmic [AutoTune](./autotune.md)
engine. Think of it as a smart strategy and explanation layer on top of the
existing tools.

## Enabling the Assistant

1. Open **Tools → Settings**.
2. Scroll to the **AI Assistant (at your own risk)** section.
3. Read the risk warning and check the acknowledgement box.
4. Fill in the provider details (see [Providers](#providers)).
5. Choose a **Capability Tier** (see below).
6. Check **Enable AI Assistant**.
7. Click **Apply** (saves without closing) or **OK** (saves and closes).

> **Note:** Each setting saves independently. If one setting fails to save
> (for example, enabling without a risk acknowledgement), the others still save
> and the failure is shown in the dialog footer.

### Capability Tiers

Tiers are **cumulative** — each level includes everything the previous one
allows:

| Tier | What it allows |
|------|----------------|
| **Read — inspect and diagnose only** | Explain and analyze; read tables, constants, realtime data, and datalogs. Cannot propose changes. Start here until you trust the assistant. |
| **Tune — read + propose table edits** | Additionally propose table-cell edits and bulk operations (scale, smooth, interpolate, set equal). |
| **Config — tune + propose constant changes** | Additionally propose constant changes (feature toggles, scalars). Pin-affecting constants are flagged **Dangerous**. |

The tier is enforced on both ends: the assistant is not even *offered* the
propose tools above the configured tier, and an out-of-tier tool call is
rejected if a model emits one anyway. Every proposal still goes through the
review queue — nothing is applied without your approval.

## Using the Assistant

1. Open **Tools → AI Assistant** (a toggle — click again to hide the panel).
2. Type a message in the chat input. Examples:
   - "Can you review my ignition table?"
   - "How does my VE table look? Any glaring issues?"
   - "What's the engine doing right now?"
   - "Summarize my last datalog — where was AFR off target?"
   - "Enable launch control."
   - "Explain the fuelAlgorithm setting."

Values the assistant reads back (constants, realtime channels) are converted
to your **display units** (Settings → Units), so its answers match what the
UI shows.
3. The assistant may **read data first** (it will say "Let me pull up…"), then
   reply with an analysis. This is normal — it's gathering context. While it
   works, the pending message shows **live activity** such as
   "reading veTable1…" or "thinking (round 2)…", so you can see what it is
   doing instead of watching a silent spinner.
4. If the assistant wants to make changes, it emits a **proposal** that appears
   in the **Review queue** at the bottom of the panel.
5. Each proposed item shows:
   - **What** it wants to change (table cell, constant, bulk operation).
   - **Why** it wants to change it (the assistant's reasoning).
   - **Safety tier** (Safe / Caution / Dangerous) for configuration changes.
   - **Validation status** — whether the proposal is within the INI's declared bounds.
   - **Clamp notice** — if a tuning proposal exceeded the authority limit.
   - **Pin-conflict warning** — if a configuration change would assign a pin
     another live function already uses.
6. Accept or reject each item, or click **Accept all valid**.
7. Click **Stage accepted changes** to apply them to the working tune. When
   you do:
   - A **restore point** is created first, so you can roll the whole batch
     back from **Tools → Restore Points** if you change your mind.
   - If a table's accepted edits shift its cells by a large amount on average,
     a **batch warning** tells you — individually-valid edits can still walk a
     table too far in one direction.
   - With version control's auto-commit set to *Always*, the tune is saved and
     committed automatically (the commit hash is shown); with *Ask*, you get a
     ready-made commit message to use after saving.
8. **Burn** the tune to the ECU separately, only when you are satisfied.
   Staging writes to the ECU's RAM (exactly like a manual table edit) but never
   burns.

### The Side Panel

- **Resizable** — drag the left edge to resize (280–640px).
- **Collapsible review queue** — click the queue header to collapse/expand it.
  It auto-expands when new proposals arrive.
- **Pop-out** — click the external-link icon in the panel header to open the
  assistant in its own window. Use that window's **Dock** button to bring it
  back to the docked panel.
- **Stop** — while the assistant is thinking, the **Send** button becomes a
  red **Stop** button. Click it to cancel the in-flight request (useful if a
  provider is slow or stuck). A cancelled request shows `_(stopped)_` in the
  transcript rather than an error.
- **Token counter** — the panel header shows the total tokens used by the
  current chat (cost transparency for hosted providers). It resets when you
  switch or start a new chat.

### Ask AI from a Table Editor

Every table editor's toolbar has an **Ask AI** button (the robot icon). Click
it to open the assistant with that table already in context: the assistant
knows which table you're looking at, its axes, and your current cell
selection, and the input is pre-filled with "About *table name*: " — just
finish the question. This is the quickest way to go from "this cell looks
wrong" to a diagnosed, proposed fix.

### Chat History

The assistant persists your conversations **per project**, so you can close
the app and come back to pick up where you left off.

- **Auto-save** — every message is saved automatically.
- **Chat switcher** — click the **list icon** in the panel header to see all
  past chats for the current project, most-recent first. Click one to switch
  to it; click the × to delete it.
- **New chat** — click the **plus icon** to start a fresh conversation.
- **Auto-open** — on launch, the most recent chat for the current project
  loads automatically.

Chats are stored as JSON files under `projectCfg/ai_chats/` inside the project
folder, so they travel with the project if you copy it.

## UI State Persistence

LibreTune remembers your workspace layout across restarts. When you close and
reopen the app (or refresh the window), it restores:

- **Sidebar visibility** — whether the left sidebar is shown or hidden.
- **Sidebar expansion** — which folders are expanded/collapsed.
- **AI Assistant panel** — whether the right-hand chat panel is open.
- **Selected dashboard** — which dashboard was last loaded.
- **Window size and position** — handled by the window-state plugin.

This means the app opens "exactly as you left it" without any extra setup.

## Providers

LibreTune speaks each provider's **native protocol** (not just an
OpenAI-compatible shim):

| Provider | Setting value | Notes |
|----------|---------------|-------|
| **OpenAI** | `openai` | Also works with any OpenAI-compatible endpoint: OpenRouter, Ollama (`/v1`), LM Studio, vLLM. |
| **Anthropic** | `anthropic` | Claude models (`claude-3-5-sonnet-…`, etc.). |
| **Google** | `google` | Gemini models (`gemini-1.5-pro`, etc.). |

The settings dialog also offers **presets** — a one-click dropdown that fills
in the provider, base URL, and a suggested model for OpenAI, Anthropic,
Google, and the local options below.

### Provider Settings

- **Base URL** — leave empty for the provider default. For a local model, point
  at its endpoint, e.g. `http://localhost:11434/v1` for Ollama.
- **API key** — your provider key. Optional for local/no-auth endpoints.
- **Model** — e.g. `gpt-4o`, `claude-3-5-sonnet-20241022`, `gemini-1.5-pro`.
  The model **must support tool/function calling** for the assistant to work.

### Local Models (Recommended for Privacy)

Because you bring your own model, you control where your data goes. For maximum
privacy, run a local model with [Ollama](https://ollama.com):

1. Install Ollama and pull a tool-capable model (e.g. `ollama pull llama3.1`).
2. In settings, set **Provider** = OpenAI, **Base URL** = `http://localhost:11434/v1`.
3. Leave the API key empty.
4. Set **Model** to the model name you pulled.

Tune data never leaves your machine.

## Safety Model

The assistant is deliberately constrained at multiple layers:

| Layer | What it does |
|-------|--------------|
| **Propose only** | The model emits tool calls that become a proposal list. It has no apply/burn command. |
| **Capability tiers** | The configured tier (read / tune / config) determines which tools the model is even offered; out-of-tier calls are rejected if a model emits them anyway. |
| **INI validation** | Every proposal is checked against the loaded INI: table/constant existence, cell-index bounds, `min`/`max`, `DataType` storage range, and bits-type enum values. |
| **Authority clamping** | Tune edits are clamped to the same per-cell and percentage limits used by AutoTune (`AutoTuneAuthorityLimits`). |
| **Pin-conflict warnings** | Constant proposals that would move a function onto a pin another live function already uses are flagged in the review queue. |
| **Batch drift warnings** | A batch of individually-valid edits that collectively shifts a table's mean by more than 10% triggers a warning at apply time. |
| **Human approval** | Accepted proposals are staged; they do not burn automatically. |
| **Dangerous-constant flagging** | Constants that affect pins, triggers, output inversion, or hard limits are marked **Dangerous** and require explicit per-item confirmation. |
| **Pre-apply restore point** | Staging creates a restore point first, so any AI apply is a one-click rollback. |
| **No ECU burn** | The assistant has no burn command. You must click Burn yourself. |
| **Read-loop bounded** | Multi-turn read interactions are capped (6 rounds) to prevent runaway cost. |

### How proposals are validated

When the assistant proposes a value, the validator checks three things:

1. **Display-unit bounds** — the INI declares `min` and `max` for every
   constant. A proposed value outside that range is rejected with an error.
2. **Storage-type range** — even before scaling, the raw value must fit the
   underlying type (e.g. a `U16` must be 0–65535). This catches values the
   display bounds might miss when `scale ≠ 1`.
3. **Bits-type enumeration** — for feature toggles, the value must be a valid
   index into the constant's declared options.

## Tips for Good Results

- **Start with the Read tier** until you understand how the assistant reasons.
- **Ask it to read first**: "Read my VE table and tell me which cells have good
  data coverage" gives it context before it proposes edits.
- **Use the Ask AI button** on a table editor when a specific table is what
  you want to talk about — it starts the conversation with the right context.
- **Point it at your datalog**: "Summarize my last datalog and tell me where
  AFR was off target" pairs well with a VE-table proposal request.
- **Be specific**: "Lean out the 3000 RPM / 80 kPa cell by 2%" is safer than
  "fix my tune."
- **Review every clamp**: if a value was authority-clamped, the original
  requested value and reason are shown in the review queue.
- **Compare with AutoTune**: for VE tables, the algorithmic AutoTune engine
  remains the ground truth. Use the assistant for strategy, then cross-check
  its proposals.

## Troubleshooting

| Problem | Likely cause / fix |
|---------|-------------------|
| "AI assistant is not enabled" | Enable it in Settings and acknowledge the risk warning. |
| "AI assistant risk acknowledgement is missing" | Check the acknowledgement box in the AI Assistant settings section. |
| "Authentication failed" | Check your API key. |
| "Could not parse provider response" | The model may not support tool/function calling. Use a model that does (e.g. `gpt-4o`, `claude-3-5-sonnet`). |
| Proposals fail validation | The model asked for an out-of-bounds value or a non-existent table/constant. Reject it and rephrase your request. |
| Assistant does not propose changes | You may be in Read tier, or the model chose not to call a tool. Ask more specifically, or raise the capability tier. |
| Pin-conflict warning on a proposal | The change would assign a pin another enabled function already uses. Clear the other assignment first, or reject the proposal. |
| Batch warning about a large mean shift | The accepted edits move the table further than 10% on average. Confirm that is intended, or reject some items and re-apply. |
| Assistant stalls after "let me look at…" | This should not happen — the read loop executes reads and feeds results back. If it does, the model may be returning no tool calls; check the provider/model supports tool calling. |
| Datalog queries return "no entries recorded" | Start datalogging first, or pass the saved log's file name (the assistant is told which logs are available when it guesses wrong). |
| Settings don't save when I hit Apply | Look at the footer of the settings dialog — it shows which setting failed (hover for details). |

## Privacy & Data Handling

- **Hosted providers** (OpenAI, Anthropic, Google): tune data and ECU context
  are sent over HTTPS to that provider. Do not use them with sensitive data
  unless you trust their policies.
- **Local providers** (Ollama, LM Studio, vLLM): data stays on your machine.
- **API key storage**: the key is stored in your **operating system's
  keychain** (Windows Credential Manager, macOS Keychain, Linux Secret
  Service) whenever one is available — never in the settings file. If no
  keychain backend exists (e.g. some headless Linux sessions), LibreTune
  falls back to storing it in the settings file so the assistant keeps
  working.
- Changing the provider or API key resets the risk acknowledgement, so you
  re-confirm before the assistant is re-enabled.
