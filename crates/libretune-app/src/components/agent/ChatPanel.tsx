/**
 * ChatPanel — the conversational surface for the AI assistant.
 *
 * The user types a message; we call `agent_send_message`, which runs one
 * orchestrator turn against the configured LLM provider and returns a
 * Proposal (assistant reply text + proposed actions). The reply is appended
 * to the transcript and any proposed actions flow up to the parent for the
 * review queue.
 */
import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../common';
import { useUnitPreferences } from '../../contexts/useUnitPreferences';
import type {
  AgentStatus,
  Proposal,
  ProposedAction,
  SerializedMessage,
} from '../../types/agent';
import './AgentPanel.css';

export interface ChatPanelProps {
  status: AgentStatus | null;
  /** When the assistant proposes changes, hand them to the review queue. */
  onProposals: (proposed: ProposedAction[]) => void;
  /** System prompt describing current ECU/tune context. */
  systemPrompt: string;
  /** Controlled transcript (owned by the parent for chat history). */
  transcript: TranscriptEntry[];
  /** Update the transcript. */
  onTranscriptChange: (next: TranscriptEntry[]) => void;
}

export interface TranscriptEntry {
  role: 'user' | 'assistant';
  content: string;
  pending?: boolean;
}

export function ChatPanel({
  status,
  onProposals,
  systemPrompt,
  transcript,
  onTranscriptChange,
}: ChatPanelProps) {
  const [input, setInput] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const { preferences } = useUnitPreferences();

  // Auto-scroll to the latest message.
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [transcript]);

  const enabled = status?.enabled && status?.risk_acknowledged && status?.configured;

  const send = async () => {
    const message = input.trim();
    if (!message || sending || !enabled) return;
    setInput('');
    setError(null);

    // Snapshot the transcript at the START of this turn. We build the updated
    // transcript from this snapshot rather than re-reading the `transcript`
    // prop later (which would be a stale closure value after the parent
    // re-renders). This keeps a single send() invocation self-consistent.
    const snapshot = [...transcript];
    const history: SerializedMessage[] = snapshot
      .filter((e) => !e.pending)
      .map((e) => ({ role: e.role, content: e.content }));

    // The transcript for this turn: prior messages + user msg + pending reply.
    const turnTranscript: TranscriptEntry[] = [
      ...snapshot,
      { role: 'user', content: message },
      { role: 'assistant', content: '', pending: true },
    ];
    onTranscriptChange(turnTranscript);
    setSending(true);

    try {
      const proposal = await invoke<Proposal>('agent_send_message', {
        request: {
          user_message: message,
          history,
          system_prompt: systemPrompt,
          // Read-tool results are converted to the user's display units so
          // the model's answers match what the UI shows.
          unit_prefs: {
            temperature: preferences.temperature,
            pressure: preferences.pressure,
            afr: preferences.afr,
            fuel_type: preferences.fuelType,
          },
        },
      });
      // Replace the pending placeholder with the real reply.
      const afterReply = turnTranscript.map((e) =>
        e.pending ? { role: 'assistant' as const, content: proposal.reply || '(no reply)' } : e
      );
      onTranscriptChange(afterReply);
      if (proposal.proposed.length > 0) {
        onProposals(proposal.proposed);
      }
    } catch (e) {
      const errStr = String(e);
      // The "__cancelled__" sentinel means the user clicked Stop.
      const cancelled = errStr.includes('__cancelled__');
      // Replace the pending placeholder with an error/stopped note.
      const afterErr = turnTranscript.map((entry) =>
        entry.pending
          ? { role: 'assistant' as const, content: cancelled ? '_(stopped)_' : `⚠️ Error: ${errStr}` }
          : entry
      );
      onTranscriptChange(afterErr);
      if (!cancelled) {
        setError(errStr);
      }
    } finally {
      setSending(false);
    }
  };

  /** Cancel an in-flight request (the Stop button). */
  const stop = async () => {
    try {
      await invoke('agent_stop');
    } catch {
      // ignore — the sentinel handling in send() covers the UX
    }
  };

  if (status === null) {
    // Status hasn't been fetched yet — don't flash the "disabled" message.
    return (
      <div className="agent-chat-disabled">Loading…</div>
    );
  }

  if (!status.enabled) {
    return (
      <div className="agent-chat-disabled">
        The AI assistant is disabled. Enable it in Settings (and acknowledge the
        risk warning) to start.
      </div>
    );
  }

  if (!enabled) {
    // Enabled but missing config — tell the user what's missing.
    const missing: string[] = [];
    if (!status.risk_acknowledged) missing.push('risk acknowledgement');
    if (!status.configured) missing.push('a provider and model');
    return (
      <div className="agent-chat-disabled">
        The assistant is enabled but needs {missing.join(' and ')}.
        Configure it in Settings to start chatting.
      </div>
    );
  }

  return (
    <div className="agent-chat">
      <div className="agent-chat-transcript" ref={scrollRef}>
        {transcript.length === 0 && (
          <div className="agent-chat-empty">
            Ask the assistant to help tune or configure your ECU. For example:
            <ul>
              <li>“Enable launch control”</li>
              <li>“Tune my VE table around 3000 rpm”</li>
              <li>“Why is my car running lean at cruise?”</li>
            </ul>
          </div>
        )}
        {transcript.map((entry, i) => (
          <div key={i} className={`agent-chat-message agent-chat-${entry.role}`}>
            <span className="agent-chat-role">
              {entry.role === 'user' ? 'You' : 'Assistant'}
            </span>
            <span className="agent-chat-content">
              {entry.pending ? 'Thinking…' : entry.content}
            </span>
          </div>
        ))}
      </div>

      {error && <div className="agent-chat-error">{error}</div>}

      <div className="agent-chat-input-row">
        <textarea
          className="agent-chat-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          placeholder={
            enabled ? 'Message the assistant… (Enter to send, Shift+Enter for newline)' : 'Configure the assistant in Settings first'
          }
          disabled={!enabled || sending}
          rows={2}
        />
        <Button
          variant={sending ? 'danger' : 'primary'}
          onClick={() => (sending ? void stop() : void send())}
          disabled={!enabled || (!sending && !input.trim())}
        >
          {sending ? 'Stop' : 'Send'}
        </Button>
      </div>
    </div>
  );
}
