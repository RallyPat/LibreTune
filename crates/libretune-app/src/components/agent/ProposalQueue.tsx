/**
 * ProposalQueue — the review surface for LLM-proposed actions.
 *
 * Every proposal the assistant emits flows here. The user reviews each item,
 * sees its validation status / safety tier / clamp notice, and either accepts
 * or rejects it. Accepted items are staged via `agent_apply_proposals`.
 *
 * The assistant NEVER applies anything directly. This queue is the human gate.
 */
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../common';
import type {
  AgentAction,
  ApplyProposalsResponse,
  ProposedAction,
} from '../../types/agent';
import './AgentPanel.css';

export interface ProposalQueueProps {
  /** Proposed actions currently awaiting review. */
  proposed: ProposedAction[];
  /** Clear the queue (after applying / dismissing). */
  onClear: () => void;
  /** Notified when items are applied (so the parent can refresh views / report). */
  onApplied?: (response: ApplyProposalsResponse) => void;
}

/** Render a human-readable label for an action. */
function actionLabel(a: AgentAction): string {
  switch (a.type) {
    case 'TableEdit':
      return `Set ${a.data.table_name}[${a.data.x_index},${a.data.y_index}] = ${a.data.new_value}`;
    case 'ConstantChange':
      return `Set ${a.data.constant_name} = ${a.data.new_value}`;
    case 'BulkOperation':
      return `${a.data.operation} on ${a.data.table_name} (${a.data.cells.length} cells)`;
    case 'Pause':
      return `Pause ${a.data.duration_ms}ms`;
    case 'SendCommand':
      return `Command: ${a.data.command}`;
  }
}

function tierBadgeClass(tier: ProposedAction['safety_tier']): string {
  return `proposal-tier proposal-tier-${tier}`;
}

function tierLabel(tier: ProposedAction['safety_tier']): string {
  switch (tier) {
    case 'safe':
      return 'Safe';
    case 'caution':
      return 'Caution';
    case 'dangerous':
      return 'Dangerous';
  }
}

export function ProposalQueue({ proposed, onClear, onApplied }: ProposalQueueProps) {
  // Track per-item accepted/rejected state by index. An item not in the map
  // is still undecided.
  const [decisions, setDecisions] = useState<Record<number, boolean>>({});
  const [applying, setApplying] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [batchWarnings, setBatchWarnings] = useState<string[]>([]);

  const setDecision = (idx: number, accept: boolean) => {
    setDecisions((prev) => ({ ...prev, [idx]: accept }));
  };

  const acceptedActions: AgentAction[] = proposed
    .filter((_, i) => decisions[i] === true && proposed[i].validation.status !== 'failed')
    .map((p) => p.action);

  const acceptCount = proposed.filter((_, i) => decisions[i] === true).length;

  const applyAll = async () => {
    if (acceptedActions.length === 0) return;
    setApplying(true);
    setApplyError(null);
    setBatchWarnings([]);
    try {
      const response = await invoke<ApplyProposalsResponse>('agent_apply_proposals', {
        request: { actions: acceptedActions },
      });
      setBatchWarnings(response.batch_warnings ?? []);
      onApplied?.(response);
      onClear();
      setDecisions({});
    } catch (e) {
      setApplyError(String(e));
    } finally {
      setApplying(false);
    }
  };

  if (proposed.length === 0) {
    return (
      <div className="proposal-queue-empty">
        No proposed changes. Ask the assistant to tune or configure something.
      </div>
    );
  }

  return (
    <div className="proposal-queue">
      <div className="proposal-queue-header">
        <span className="proposal-queue-title">
          Proposed changes ({proposed.length})
        </span>
        <div className="proposal-queue-actions">
          <Button
            variant="secondary"
            size="sm"
            onClick={() => {
              const next: Record<number, boolean> = {};
              proposed.forEach((_, i) => {
                next[i] = proposed[i].validation.status !== 'failed';
              });
              setDecisions(next);
            }}
          >
            Accept all valid
          </Button>
          <Button variant="ghost" size="sm" onClick={onClear}>
            Reject all
          </Button>
        </div>
      </div>

      <ul className="proposal-list">
        {proposed.map((p, i) => {
          const decided = decisions[i];
          const failed = p.validation.status === 'failed';
          return (
            <li
              key={i}
              className={`proposal-item${decided === false ? ' rejected' : ''}${
                decided === true ? ' accepted' : ''
              }`}
            >
              <div className="proposal-item-main">
                <span className={tierBadgeClass(p.safety_tier)}>{tierLabel(p.safety_tier)}</span>
                <span className="proposal-item-label">{actionLabel(p.action)}</span>
              </div>

              {p.reason && <div className="proposal-item-reason">{p.reason}</div>}

              {p.clamp_reason && (
                <div className="proposal-item-clamp">
                  Clamped{p.clamped_from !== null ? ` from ${p.clamped_from}` : ''}: {p.clamp_reason}
                </div>
              )}

              {failed ? (
                <div className="proposal-item-errors">
                  {p.validation.status === 'failed' &&
                    p.validation.errors.map((e, j) => <div key={j}>• {e}</div>)}
                </div>
              ) : p.validation.status === 'ok' && p.validation.warnings.length > 0 ? (
                <div className="proposal-item-warnings">
                  {p.validation.warnings.map((w, j) => (
                    <div key={j}>• {w}</div>
                  ))}
                </div>
              ) : null}

              <div className="proposal-item-buttons">
                <Button
                  variant={decided === true ? 'primary' : 'ghost'}
                  size="sm"
                  disabled={failed}
                  onClick={() => setDecision(i, true)}
                >
                  Accept
                </Button>
                <Button
                  variant={decided === false ? 'danger' : 'ghost'}
                  size="sm"
                  onClick={() => setDecision(i, false)}
                >
                  Reject
                </Button>
              </div>
            </li>
          );
        })}
      </ul>

      {applyError && <div className="proposal-apply-error">{applyError}</div>}

      {batchWarnings.length > 0 && (
        <div className="proposal-batch-warnings">
          {batchWarnings.map((w, i) => (
            <div key={i}>• {w}</div>
          ))}
        </div>
      )}

      <div className="proposal-queue-footer">
        <Button
          variant="primary"
          onClick={applyAll}
          disabled={applying || acceptCount === 0}
        >
          {applying
            ? 'Applying…'
            : `Stage ${acceptCount} accepted change${acceptCount === 1 ? '' : 's'} (does not burn)`}
        </Button>
      </div>
    </div>
  );
}
