/**
 * useEnabledCondition — evaluates an INI-style boolean expression against
 * the realtime channel store, with light debouncing and result caching.
 *
 * Returns `true` when:
 *   - `expr` is null/empty/undefined (i.e. no gating), OR
 *   - the backend `evaluate_expression` Tauri command returns true.
 *
 * Plan v2 / D-6: lets dashboard clusters and individual gauges/indicators
 * be hidden based on an `enabled_condition` (e.g. `"hasLambdaSensor"` or
 * `"rpm > 0"`).
 *
 * Polling is centralized at module scope rather than per-instance: a
 * dashboard commonly mounts many gauges/indicators, and several of them
 * often share the exact same condition string (e.g. every wideband gauge
 * gated on `"hasLambdaSensor"`). Previously each hook instance ran its own
 * unstaggered `setInterval(250ms)` and its own `evaluate_expression` IPC
 * call, so N components with the same condition meant N redundant timers
 * and N redundant backend calls re-serializing the whole channel map every
 * tick. Now there is exactly one shared timer, and each distinct expression
 * is evaluated at most once per tick regardless of how many components are
 * gated on it; all subscribed instances for that expression share the
 * result.
 */
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useRealtimeStore } from '../../../stores/realtimeStore';

const POLL_MS = 250;

type Listener = (result: boolean) => void;

interface ExprState {
  result: boolean;
  listeners: Set<Listener>;
}

/** One entry per distinct expression currently gating at least one mounted component. */
const exprStates = new Map<string, ExprState>();
let intervalId: ReturnType<typeof setInterval> | null = null;

function notify(expr: string, value: boolean) {
  const state = exprStates.get(expr);
  if (!state) return;
  state.result = value;
  for (const listener of state.listeners) listener(value);
}

async function evaluateOne(expr: string): Promise<void> {
  const context = useRealtimeStore.getState().channels;
  try {
    const value = await invoke<boolean>('evaluate_expression', { expression: expr, context });
    notify(expr, value);
  } catch {
    // Permissive default: if backend can't evaluate (not connected, unknown
    // identifier, etc.) we keep the element visible rather than mysteriously
    // hiding it.
    notify(expr, true);
  }
}

/** One shared tick: evaluates every distinct subscribed expression once. */
function pollAll(): void {
  for (const expr of exprStates.keys()) {
    void evaluateOne(expr);
  }
}

function ensureScheduler(): void {
  if (intervalId !== null) return;
  intervalId = setInterval(pollAll, POLL_MS);
}

function stopSchedulerIfIdle(): void {
  if (exprStates.size === 0 && intervalId !== null) {
    clearInterval(intervalId);
    intervalId = null;
  }
}

/** Subscribes `listener` to `expr`'s evaluated result, sharing the poll and
 * the in-flight/last-known result with any other subscriber of the same
 * expression. Returns an unsubscribe function. */
function subscribe(expr: string, listener: Listener): () => void {
  let state = exprStates.get(expr);
  const isNewExpr = !state;
  if (!state) {
    state = { result: true, listeners: new Set() };
    exprStates.set(expr, state);
  }
  state.listeners.add(listener);
  ensureScheduler();

  if (isNewExpr) {
    // Don't make a newly-mounted component wait up to POLL_MS for its first
    // result just because it happened to subscribe between ticks.
    void evaluateOne(expr);
  } else {
    // Another subscriber already has a result for this expression — hand it
    // over immediately instead of showing the default until the next tick.
    listener(state.result);
  }

  return () => {
    const current = exprStates.get(expr);
    if (!current) return;
    current.listeners.delete(listener);
    if (current.listeners.size === 0) {
      exprStates.delete(expr);
      stopSchedulerIfIdle();
    }
  };
}

export function useEnabledCondition(expr: string | null | undefined): boolean {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    if (!expr || expr.trim().length === 0) {
      setEnabled(true);
      return;
    }
    const unsubscribe = subscribe(expr, setEnabled);
    return unsubscribe;
  }, [expr]);

  return enabled;
}
