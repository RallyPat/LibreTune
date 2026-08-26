/**
 * AgentDock — the container for the AI assistant.
 *
 * Holds the review queue state, polls `agent_status` so the panels reflect the
 * current enable/config state, and renders the ChatPanel + ProposalQueue side
 * by side. Mounted in App.tsx, gated on the assistant being enabled.
 */
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChatPanel, type TranscriptEntry } from './ChatPanel';
import { ProposalQueue } from './ProposalQueue';
import type { AgentStatus, ApplyProposalsResponse, ProposedAction } from '../../types/agent';
import './AgentPanel.css';

export interface AgentDockProps {
  /** Optional: rebuild a context-aware system prompt from current ECU state. */
  buildSystemPrompt?: () => string;
}

const DEFAULT_SYSTEM_PROMPT = `You are LibreTune's AI tuning assistant. You help the user tune and configure their ECU.
You only ever PROPOSE changes via tool calls — you never apply anything directly.
Every proposal will be validated against the ECU definition and clamped to authority limits before the user reviews it.
Be concise. When proposing changes, always explain your reasoning in the 'reason' field.
If you need more data (e.g. read a table or a constant), use a read tool first.`;

export function AgentDock({ buildSystemPrompt }: AgentDockProps) {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [queue, setQueue] = useState<ProposedAction[]>([]);
  const [appliedNote, setAppliedNote] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<TranscriptEntry[]>([]);

  // Poll status on mount and when settings change (settings:changed event).
  const refreshStatus = async () => {
    try {
      const s = await invoke<AgentStatus>('agent_status');
      setStatus(s);
    } catch {
      setStatus(null);
    }
  };

  useEffect(() => {
    void refreshStatus();
    let unlisten: (() => void) | undefined;
    // Listen for settings changes so enabling/config updates the panels live.
    (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        unlisten = await listen('settings:changed', () => void refreshStatus());
      } catch {
        // non-fatal
      }
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  const handleApplied = (response: ApplyProposalsResponse) => {
    const ok = response.results.filter((r) => r.applied).length;
    const fail = response.results.length - ok;
    const parts: string[] = [
      fail === 0
        ? `Staged ${ok} change${ok === 1 ? '' : 's'} to the working tune. Burn to the ECU when ready.`
        : `Staged ${ok}, rejected ${fail} (failed validation).`,
    ];
    if (response.restore_point) {
      parts.push(`Restore point: ${response.restore_point}`);
    }
    if (response.auto_committed) {
      parts.push(`Committed ${response.auto_committed.slice(0, 7)}`);
    }
    setAppliedNote(parts.join(' — '));
    window.setTimeout(() => setAppliedNote(null), 6000);
  };

  const systemPrompt = buildSystemPrompt?.() ?? DEFAULT_SYSTEM_PROMPT;

  return (
    <div className="agent-dock">
      <div className="agent-dock-section agent-dock-chat">
        <ChatPanel
          status={status}
          systemPrompt={systemPrompt}
          transcript={transcript}
          onTranscriptChange={setTranscript}
          onProposals={(p) => setQueue((prev) => [...prev, ...p])}
        />
      </div>
      <div className="agent-dock-section agent-dock-queue">
        <div className="agent-dock-queue-title">Review queue</div>
        <ProposalQueue
          proposed={queue}
          onClear={() => setQueue([])}
          onApplied={handleApplied}
        />
        {appliedNote && <div className="agent-dock-applied-note">{appliedNote}</div>}
      </div>
    </div>
  );
}
