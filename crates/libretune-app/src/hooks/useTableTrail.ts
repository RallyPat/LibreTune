/**
 * Accumulate the live-cell trail for a grid view (issue #132).
 *
 * One shared implementation of the history-trail bookkeeping the tab-view
 * TableEditor has always done inline: append on cell change (deduping
 * same-cell repeats), prune expired entries, and tick ~5 Hz while entries
 * exist so fades keep moving. Callers pass `null` for `liveCell` when the
 * position is unknown (no data yet — the existing trail keeps fading);
 * pass `enabled: false` (e.g. trail toggled off) to stop accumulating and
 * drop what is there.
 *
 * The incoming cell's identity is stabilized here rather than in the caller:
 * a realtime tick produces a fresh `{row, col}` object even when the cell did
 * not change, and acting on that would re-run effects for nothing.
 */
import { useEffect, useRef, useState } from 'react';
import type { TrailPoint } from '../utils/tableTrail';

export interface UseTableTrailResult {
  /** Oldest-first trail points that have not yet expired. */
  trail: TrailPoint[];
  /** Advances ~5 Hz while the trail is non-empty. Referencing it is not
   *  required — the tick's state update re-renders the calling component,
   *  which is what re-draws the overlay with fresh opacities. */
  nowTick: number;
}

export function useTableTrail(
  liveCell: { row: number; col: number } | null,
  fadeSec: number,
  enabled = true,
): UseTableTrailResult {
  const durationMs = fadeSec > 0 ? fadeSec * 1000 : Infinity;

  const [trail, setTrail] = useState<TrailPoint[]>([]);
  const [nowTick, setNowTick] = useState(0);
  const trailActiveRef = useRef(false);
  trailActiveRef.current = trail.length > 0;

  // Last cell seen by the append effect, by value: a new object for the same
  // cell (every realtime tick) must not re-fire the logic below.
  const lastCellRef = useRef<{ row: number; col: number } | null>(null);

  useEffect(() => {
    if (!enabled) {
      lastCellRef.current = null;
      setTrail((prev) => (prev.length === 0 ? prev : []));
      return;
    }
    if (!liveCell) {
      lastCellRef.current = null;
      return;
    }
    const last = lastCellRef.current;
    if (last && last.row === liveCell.row && last.col === liveCell.col) {
      return; // same cell — nothing to append; expiry is the tick's job
    }
    lastCellRef.current = { row: liveCell.row, col: liveCell.col };

    const now = Date.now();
    const newEntry: TrailPoint = { row: liveCell.row, col: liveCell.col, time: now };

    setTrail((prev) => {
      // Oldest entry first: if it has not expired, nothing has.
      const nothingExpired = prev.length === 0 || now - prev[0].time < durationMs;

      // Same cell as the last entry: nothing to add. Return the previous
      // array unchanged when nothing expired, so no re-render is queued.
      const lastEntry = prev[prev.length - 1];
      if (
        lastEntry &&
        lastEntry.row === newEntry.row &&
        lastEntry.col === newEntry.col
      ) {
        return nothingExpired
          ? prev
          : prev.filter((e) => now - e.time < durationMs);
      }

      const base = nothingExpired
        ? prev
        : prev.filter((e) => now - e.time < durationMs);
      return [...base, newEntry];
    });
  }, [enabled, liveCell, durationMs]);

  // Trail points fade over the lifetime, so something must trigger a
  // periodic re-render while entries exist: this is that tick (~5 Hz) plus
  // expiry cleanup. It must also return the previous array when nothing
  // expired, or it would queue a re-render of its own on every firing.
  useEffect(() => {
    if (!enabled) return;

    const interval = setInterval(() => {
      const now = Date.now();
      setTrail((prev) => {
        const filtered = prev.filter((e) => now - e.time < durationMs);
        return filtered.length === prev.length ? prev : filtered;
      });
      if (trailActiveRef.current) {
        setNowTick((t) => (t + 1) % 1_000_000);
      }
    }, 200);

    return () => clearInterval(interval);
  }, [enabled, durationMs]);

  return { trail, nowTick };
}
