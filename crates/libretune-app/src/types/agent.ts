/**
 * TypeScript types for the AI assistant, mirroring the Rust structs in
 * libretune-core's `agent` and `llm` modules.
 *
 * These are the shapes returned by the `agent_*` Tauri commands.
 */

/** Mirrors `libretune_core::action_scripting::Action` (the variants we surface). */
export type AgentAction =
  | {
      type: 'TableEdit';
      data: {
        table_name: string;
        x_index: number;
        y_index: number;
        new_value: number;
        old_value: number | null;
      };
    }
  | {
      type: 'ConstantChange';
      data: {
        constant_name: string;
        new_value: number;
        old_value: number | null;
      };
    }
  | {
      type: 'BulkOperation';
      data: {
        operation: string;
        table_name: string;
        cells: [number, number][];
        parameters: Record<string, number>;
        old_values: number[] | null;
      };
    }
  | { type: 'Pause'; data: { duration_ms: number } }
  | { type: 'SendCommand'; data: { command: string } };

/** Safety tier for a proposed constant change. */
export type ConstantSafetyTier = 'safe' | 'caution' | 'dangerous';

/** Validation outcome for one proposed action. */
export type ValidationResult =
  | { status: 'ok'; warnings: string[] }
  | { status: 'failed'; errors: string[] };

/** One proposed change ready for the review queue. */
export interface ProposedAction {
  action: AgentAction;
  safety_tier: ConstantSafetyTier;
  validation: ValidationResult;
  clamped_from: number | null;
  clamp_reason: string | null;
  reason: string | null;
}

/** Token usage for a response. */
export interface LlmUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

/** A complete proposal for one assistant turn. */
export interface Proposal {
  reply: string;
  finish_reason: string;
  proposed: ProposedAction[];
  all_valid: boolean;
  usage: LlmUsage | null;
}

/** Serialized chat message for the conversation history. */
export interface SerializedMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

/** Display-unit preferences passed with each turn (read results convert to these). */
export interface AgentUnitPrefs {
  temperature?: string; // 'C' | 'F' | 'K'
  pressure?: string; // 'kPa' | 'PSI' | 'bar' | 'inHg'
  afr?: string; // 'AFR' | 'Lambda'
  fuel_type?: string;
}

/** Request payload for `agent_send_message`. */
export interface AgentTurnRequest {
  user_message: string;
  history: SerializedMessage[];
  system_prompt: string;
  unit_prefs?: AgentUnitPrefs;
}

/** Result of applying one action via `agent_apply_proposals`. */
export interface ApplyResult {
  applied: boolean;
  error: string | null;
  safety_tier: ConstantSafetyTier | null;
}

/** Response from `agent_apply_proposals`: per-action results + batch warnings. */
export interface ApplyProposalsResponse {
  results: ApplyResult[];
  batch_warnings: string[];
  restore_point: string | null;
  auto_committed: string | null;
  suggest_commit: string | null;
}

/** Status returned by `agent_status`. */
export interface AgentStatus {
  enabled: boolean;
  risk_acknowledged: boolean;
  provider: string;
  model: string;
  capability_tier: string;
  configured: boolean;
}

// --- Chat history persistence ---

/** One chat message stored in a chat history file. */
export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

/** One persisted chat conversation. */
export interface ChatHistory {
  id: string;
  title: string;
  messages: ChatMessage[];
  created_at: string;
  updated_at: string;
}

/** Summary entry for the chat list (no message bodies). */
export interface ChatSummary {
  id: string;
  title: string;
  message_count: number;
  created_at: string;
  updated_at: string;
}
