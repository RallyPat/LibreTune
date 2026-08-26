/**
 * AgentSidePanel — the docked right-hand panel for the AI assistant.
 *
 * Replaces the previous full-screen modal overlay with a VS-Code-chat-style
 * side panel: non-modal, resizable, and optionally pop-out-able to its own
 * window so it can live on a second monitor.
 *
 * Layout: a header bar (title + pop-out + collapse buttons) over a vertical
 * stack of the chat transcript (top, flexible) and the review queue (bottom,
 * collapsible). The panel is rendered by `TunerLayout` as a flex child of
 * `.tuner-layout-main`; the resize handle lives on the left edge.
 */
import { useCallback, useRef, useState, useEffect, MouseEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChatPanel, type TranscriptEntry } from './ChatPanel';
import { ProposalQueue } from './ProposalQueue';
import type {
  AgentStatus,
  ApplyProposalsResponse,
  ChatHistory,
  ChatSummary,
  ProposedAction,
} from '../../types/agent';
import './AgentPanel.css';

export interface AgentSidePanelProps {
  /** Panel width in px (controlled by the parent layout for persistence). */
  width: number;
  /** Called while the left-edge resize handle is dragged. */
  onResize: (width: number) => void;
  /** Collapse the panel back to hidden. */
  onCollapse: () => void;
  /** Pop the panel out into its own window. */
  onPopOut: () => void;
}

const DEFAULT_SYSTEM_PROMPT = `You are LibreTune's AI tuning assistant. You help the user tune and configure their ECU.
You only ever PROPOSE changes via tool calls — you never apply anything directly.
Every proposal will be validated against the ECU definition and clamped to authority limits before the user reviews it.
Be concise. When proposing changes, always explain your reasoning in the 'reason' field.
If you need more data (e.g. read a table or a constant), use a read tool first.`;

export function AgentSidePanel({ width, onResize, onCollapse, onPopOut }: AgentSidePanelProps) {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [queue, setQueue] = useState<ProposedAction[]>([]);
  const [appliedNote, setAppliedNote] = useState<string | null>(null);
  const [queueCollapsed, setQueueCollapsed] = useState(false);
  const isResizing = useRef(false);

  // --- Chat history state (owned here so it can be saved/switched) ---
  const [transcript, setTranscript] = useState<TranscriptEntry[]>([]);
  const [currentChatId, setCurrentChatId] = useState<string | null>(null);
  const [chatList, setChatList] = useState<ChatSummary[]>([]);
  const [chatListOpen, setChatListOpen] = useState(false);

  /** Persist the current transcript + id to the backend. Best-effort. */
  const persistChat = useCallback(
    async (id: string, messages: TranscriptEntry[]) => {
      try {
        const title =
          messages.find((m) => m.role === 'user')?.content.slice(0, 60) || 'New chat';
        const chat: ChatHistory = {
          id,
          title,
          created_at: '',
          updated_at: '',
          messages: messages
            .filter((m) => !m.pending)
            .map((m) => ({ role: m.role, content: m.content })),
        };
        await invoke<ChatHistory>('agent_save_chat', { chat });
        // Refresh the chat list so the sidebar stays in sync.
        const list = await invoke<ChatSummary[]>('agent_list_chats');
        setChatList(list);
      } catch (e) {
        console.error('Failed to persist chat:', e);
      }
    },
    []
  );

  /** Start a fresh chat. */
  const newChat = useCallback(() => {
    setTranscript([]);
    setCurrentChatId(null);
    setChatListOpen(false);
  }, []);

  /** Switch to an existing chat by id (loads its messages). */
  const openChat = useCallback(async (id: string) => {
    try {
      const chat = await invoke<ChatHistory>('agent_load_chat', { chatId: id });
      setTranscript(chat.messages.map((m) => ({ role: m.role, content: m.content })));
      setCurrentChatId(id);
    } catch (e) {
      console.error('Failed to load chat:', e);
    }
    setChatListOpen(false);
  }, []);

  /** Delete a chat from disk and the list. */
  const deleteChat = useCallback(async (id: string) => {
    try {
      await invoke('agent_delete_chat', { chatId: id });
      if (currentChatId === id) {
        setTranscript([]);
        setCurrentChatId(null);
      }
      const list = await invoke<ChatSummary[]>('agent_list_chats');
      setChatList(list);
    } catch (e) {
      console.error('Failed to delete chat:', e);
    }
  }, [currentChatId]);

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
    // Load chat list + auto-open the most recent chat.
    (async () => {
      try {
        const list = await invoke<ChatSummary[]>('agent_list_chats');
        setChatList(list);
        if (list.length > 0) {
          await openChat(list[0].id);
        }
      } catch {
        // non-fatal (no project loaded yet)
      }
    })();
    let unlisten: (() => void) | undefined;
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

  // Left-edge resize handle: dragging right grows the panel, left shrinks it
  // (inverted from the sidebar, which sits on the left).
  const handleResizeStart = useCallback(
    (e: MouseEvent) => {
      e.preventDefault();
      isResizing.current = true;
      const startX = e.clientX;
      const startWidth = width;

      const handleMouseMove = (moveEvent: globalThis.MouseEvent) => {
        if (!isResizing.current) return;
        // Moving the cursor LEFT (negative delta) should widen the panel.
        const delta = startX - moveEvent.clientX;
        onResize(startWidth + delta);
      };
      const handleMouseUp = () => {
        isResizing.current = false;
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    },
    [width, onResize]
  );

  const handleApplied = (response: ApplyProposalsResponse) => {
    const ok = response.results.filter((r) => r.applied).length;
    const fail = response.results.length - ok;
    const parts: string[] = [
      fail === 0
        ? `Staged ${ok} change${ok === 1 ? '' : 's'}. Burn to the ECU when ready.`
        : `Staged ${ok}, rejected ${fail} (failed validation).`,
    ];
    if (response.restore_point) {
      parts.push(`Restore point: ${response.restore_point}`);
    }
    if (response.auto_committed) {
      parts.push(`Committed ${response.auto_committed.slice(0, 7)}`);
    } else if (response.suggest_commit) {
      parts.push('Save the tune, then commit from Tune History to keep this batch in git');
    }
    setAppliedNote(parts.join(' — '));
    window.setTimeout(() => setAppliedNote(null), 8000);
  };

  return (
    <div className="agent-panel" style={{ width }}>
      {/* Left-edge resize handle */}
      <div
        className="agent-panel-resize"
        onMouseDown={handleResizeStart}
        role="separator"
        aria-orientation="vertical"
      />

      {/* Header */}
      <div className="agent-panel-header">
        <span className="agent-panel-title">AI Assistant</span>
        <div className="agent-panel-header-actions">
          <button
            className="agent-panel-icon-btn"
            onClick={() => setChatListOpen((o) => !o)}
            title="Chat history"
            aria-label="Chat history"
          >
            {/* list icon */}
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M2.5 3.5a.5.5 0 0 1 .5-.5h10a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5zm0 4a.5.5 0 0 1 .5-.5h10a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5zm0 4a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5z" />
            </svg>
          </button>
          <button
            className="agent-panel-icon-btn"
            onClick={newChat}
            title="New chat"
            aria-label="New chat"
          >
            {/* plus icon */}
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 2a.5.5 0 0 1 .5.5v5h5a.5.5 0 0 1 0 1h-5v5a.5.5 0 0 1-1 0v-5h-5a.5.5 0 0 1 0-1h5v-5A.5.5 0 0 1 8 2z" />
            </svg>
          </button>
          <button
            className="agent-panel-icon-btn"
            onClick={onPopOut}
            title="Pop out to separate window"
            aria-label="Pop out"
          >
            {/* external-link icon */}
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8.5 1.5a.5.5 0 0 0 0 1h3.793L6.146 8.646a.5.5 0 1 0 .708.708L13 3.207V7a.5.5 0 0 0 1 0V2a.5.5 0 0 0-.5-.5h-5z" />
              <path d="M3 3.5A1.5 1.5 0 0 1 4.5 2H7a.5.5 0 0 1 0 1H4.5a.5.5 0 0 0-.5.5v8a.5.5 0 0 0 .5.5h8a.5.5 0 0 0 .5-.5V9a.5.5 0 0 1 1 0v2.5A1.5 1.5 0 0 1 12.5 13h-8A1.5 1.5 0 0 1 3 11.5v-8z" />
            </svg>
          </button>
          <button
            className="agent-panel-icon-btn"
            onClick={onCollapse}
            title="Hide panel"
            aria-label="Hide panel"
          >
            {/* chevron-right (collapse to the right) */}
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
            </svg>
          </button>
        </div>
      </div>

      {/* Chat history list (toggled by the list button) */}
      {chatListOpen && (
        <div className="agent-chat-list">
          {chatList.length === 0 && (
            <div className="agent-chat-list-empty">No saved chats</div>
          )}
          {chatList.map((c) => (
            <div
              key={c.id}
              className={`agent-chat-list-item${currentChatId === c.id ? ' active' : ''}`}
            >
              <button className="agent-chat-list-item-title" onClick={() => void openChat(c.id)}>
                {c.title}
              </button>
              <button
                className="agent-chat-list-item-del"
                title="Delete chat"
                onClick={() => void deleteChat(c.id)}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Chat fills the flexible area */}
      <div className="agent-panel-chat">
        <ChatPanel
          status={status}
          systemPrompt={DEFAULT_SYSTEM_PROMPT}
          transcript={transcript}
          onTranscriptChange={(next) => {
            setTranscript(next);
            // Persist on every change (debounce-free for simplicity; the write
            // is cheap). On the first message, allocate an id.
            const id =
              currentChatId ??
              `chat-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
            if (!currentChatId) {
              setCurrentChatId(id);
            }
            void persistChat(id, next);
          }}
          onProposals={(p) => {
            setQueue((prev) => [...prev, ...p]);
            setQueueCollapsed(false);
          }}
        />
      </div>

      {/* Review queue — collapsible bottom section */}
      <div className={`agent-panel-queue${queueCollapsed ? ' collapsed' : ''}`}>
        <button
          className="agent-panel-queue-toggle"
          onClick={() => setQueueCollapsed((c) => !c)}
          title={queueCollapsed ? 'Expand review queue' : 'Collapse review queue'}
        >
          <svg
            width="10"
            height="10"
            viewBox="0 0 16 16"
            fill="currentColor"
            style={{ transform: queueCollapsed ? 'rotate(-90deg)' : 'none', transition: 'transform 0.15s' }}
          >
            <path d="M1.646 4.646a.5.5 0 0 1 .708 0L8 10.293l5.646-5.647a.5.5 0 0 1 .708.708l-6 6a.5.5 0 0 1-.708 0l-6-6a.5.5 0 0 1 0-.708z" />
          </svg>
          <span>Review queue{queue.length > 0 ? ` (${queue.length})` : ''}</span>
        </button>
        {!queueCollapsed && (
          <div className="agent-panel-queue-body">
            <ProposalQueue
              proposed={queue}
              onClear={() => setQueue([])}
              onApplied={handleApplied}
            />
            {appliedNote && <div className="agent-panel-applied-note">{appliedNote}</div>}
          </div>
        )}
      </div>
    </div>
  );
}
