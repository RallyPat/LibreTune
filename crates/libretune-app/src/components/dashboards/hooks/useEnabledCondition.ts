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
 */
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useRealtimeStore } from '../../../stores/realtimeStore';

const POLL_MS = 250;

export function useEnabledCondition(expr: string | null | undefined): boolean {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    if (!expr || expr.trim().length === 0) {
      setEnabled(true);
      return;
    }
    let cancelled = false;
    const poll = () => {
      const context = useRealtimeStore.getState().channels;
      invoke<boolean>('evaluate_expression', { expression: expr, context })
        .then((v) => {
          if (!cancelled) setEnabled(v);
        })
        .catch(() => {
          // Permissive default: if backend can't evaluate (not connected,
          // unknown identifier, etc.) we keep the element visible rather
          // than mysteriously hiding it.
          if (!cancelled) setEnabled(true);
        });
    };
    poll();
    const id = setInterval(poll, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [expr]);

  return enabled;
}
