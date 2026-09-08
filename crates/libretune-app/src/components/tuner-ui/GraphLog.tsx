/**
 * Graph Log — stacked strip charts for viewing recorded channels.
 *
 * Renders the active graph-log tab as stacked panes sharing a time axis.
 * Each pane plots one channel against the left axis and one against the
 * right, each with its own fixed or auto scale.
 *
 * The graph only draws recorded data: the session log while recording or
 * after Stop, or a loaded log in playback. Before the first recording it
 * shows an empty grid with a hint.
 */
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Plus, X, Settings2, ZoomIn, ZoomOut } from 'lucide-react';
import { Dialog } from '../common';
import {
  useGraphLogStore,
  selectActiveTab,
  ChannelSlot,
  GraphPane,
  AxisSide,
} from '../../stores/graphLogStore';
import { nearestIndex as nearestIndexGeneric } from '../../utils/nearestIndex';
import './GraphLog.css';

export interface GraphSample {
  /** Timestamp in milliseconds (epoch or log-relative — only deltas matter) */
  t: number;
  values: Record<string, number>;
}

export interface GraphLogProps {
  /** Recorded session log or playback samples */
  samples: GraphSample[];
  /** Channels the user may assign to slots */
  availableChannels: string[];
  /** Cursor position 0..1 across the visible window (playback), or null */
  cursorPosition?: number | null;
}

const AXIS_TICKS = 5;
const PANE_MIN_HEIGHT = 70;
/** Horizontal padding reserved for the left/right axis labels */
const PAD_L = 52;
const PAD_R = 52;
/** Zoom step per Q/A keypress or zoom button click */
const ZOOM_FACTOR = 0.75;

function formatWindow(sec: number): string {
  if (sec < 10) return `${sec.toFixed(1)} s`;
  if (sec < 60) return `${Math.round(sec)} s`;
  const m = Math.floor(sec / 60);
  const s = Math.round(sec % 60);
  return s > 0 ? `${m}m ${s}s` : `${m} min`;
}

function formatTick(v: number): string {
  const abs = Math.abs(v);
  if (abs >= 1000) return v.toFixed(0);
  if (abs >= 10) return v.toFixed(1);
  return v.toFixed(2);
}

function formatClock(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

/** Resolve slot bounds: fixed when configured, otherwise min/max of visible data. */
function slotBounds(slot: ChannelSlot, visible: GraphSample[]): { min: number; max: number } {
  if (!slot.auto) return { min: slot.min, max: slot.max };
  let min = Infinity;
  let max = -Infinity;
  if (slot.channel) {
    for (const s of visible) {
      const v = s.values[slot.channel];
      if (v === undefined) continue;
      if (v < min) min = v;
      if (v > max) max = v;
    }
  }
  if (!isFinite(min) || !isFinite(max)) return { min: 0, max: 100 };
  if (min === max) {
    const pad = Math.abs(min) * 0.1 || 1;
    return { min: min - pad, max: max + pad };
  }
  const pad = (max - min) * 0.05;
  return { min: min - pad, max: max + pad };
}

/** Last recorded value of a channel within the visible window */
function lastValue(channel: string | null, visible: GraphSample[]): number | undefined {
  if (!channel) return undefined;
  for (let i = visible.length - 1; i >= 0; i--) {
    const v = visible[i].values[channel];
    if (v !== undefined) return v;
  }
  return undefined;
}

/** Binary search for the sample index nearest to time t */
function nearestIndex(data: GraphSample[], t: number): number {
  return nearestIndexGeneric(data, t, (s) => s.t);
}

interface PaneCanvasProps {
  pane: GraphPane;
  visible: GraphSample[];
  windowMs: number;
  windowEnd: number;
  width: number;
  height: number;
  cursorPosition?: number | null;
  /** Hovered position 0..1 across the plot area, or null */
  hoverFrac?: number | null;
  /** Persistent data cursor sample (arrow-key navigable), or null */
  cursorSample?: GraphSample | null;
  onOpenConfig: () => void;
  /** Channels offered by the per-track pickers on the pane itself. */
  availableChannels: string[];
  onPickChannel: (side: AxisSide, channel: string | null) => void;
}

const PaneCanvas: React.FC<PaneCanvasProps> = ({
  pane,
  visible,
  windowMs,
  windowEnd,
  width,
  height,
  cursorPosition,
  hoverFrac = null,
  cursorSample = null,
  onOpenConfig,
  availableChannels,
  onPickChannel,
}) => {
  const groups = useMemo(() => groupChannels(availableChannels), [availableChannels]);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx.scale(dpr, dpr);

    const padL = PAD_L;
    const padR = PAD_R;
    const padT = 6;
    const padB = 4;
    const plotW = Math.max(10, width - padL - padR);
    const plotH = Math.max(10, height - padT - padB);

    const styles = getComputedStyle(canvas);
    const bg = styles.getPropertyValue('--graphlog-pane-bg').trim() || '#141824';
    const gridColor = styles.getPropertyValue('--graphlog-grid').trim() || 'rgba(128,140,160,0.15)';

    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, width, height);

    // Grid
    ctx.strokeStyle = gridColor;
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let i = 0; i <= AXIS_TICKS - 1; i++) {
      const y = padT + (plotH * i) / (AXIS_TICKS - 1);
      ctx.moveTo(padL, y);
      ctx.lineTo(padL + plotW, y);
    }
    const vLines = 6;
    for (let i = 0; i <= vLines; i++) {
      const x = padL + (plotW * i) / vLines;
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + plotH);
    }
    ctx.stroke();

    const sides: AxisSide[] = ['left', 'right'];
    for (const side of sides) {
      const slot = pane[side];
      const { min, max } = slotBounds(slot, visible);
      const range = max - min || 1;

      // Axis tick labels
      ctx.fillStyle = slot.color;
      ctx.font = '10px monospace';
      ctx.textAlign = side === 'left' ? 'right' : 'left';
      ctx.textBaseline = 'middle';
      if (slot.channel) {
        for (let i = 0; i < AXIS_TICKS; i++) {
          const frac = i / (AXIS_TICKS - 1);
          const y = padT + plotH * (1 - frac);
          const v = min + range * frac;
          const x = side === 'left' ? padL - 6 : padL + plotW + 6;
          ctx.fillText(formatTick(v), x, y);
        }
      }

      // Trace, drawn as raw sample-and-hold steps — this is measurement data,
      // not a smoothed presentation graph. When there are more samples than
      // pixels, collapse each pixel column to its min/max span (oscilloscope
      // style) so dense logs stay fast and peaks stay visible.
      if (slot.channel && visible.length >= 2) {
        ctx.strokeStyle = slot.color;
        ctx.lineWidth = 1;
        ctx.beginPath();
        if (visible.length > plotW * 2) {
          const colMin = new Float64Array(Math.ceil(plotW)).fill(Infinity);
          const colMax = new Float64Array(Math.ceil(plotW)).fill(-Infinity);
          for (const s of visible) {
            const v = s.values[slot.channel];
            if (v === undefined) continue;
            const col = Math.min(
              Math.ceil(plotW) - 1,
              Math.max(0, Math.floor(plotW * (1 - (windowEnd - s.t) / windowMs))),
            );
            if (v < colMin[col]) colMin[col] = v;
            if (v > colMax[col]) colMax[col] = v;
          }
          let prevYMid: number | null = null;
          for (let col = 0; col < colMin.length; col++) {
            if (colMin[col] === Infinity) {
              prevYMid = null;
              continue;
            }
            const x = padL + col;
            const yLo = padT + plotH * (1 - (colMin[col] - min) / range);
            const yHi = padT + plotH * (1 - (colMax[col] - min) / range);
            // connect columns so sparse spikes don't float detached
            if (prevYMid !== null) {
              ctx.moveTo(x - 1, prevYMid);
              ctx.lineTo(x, yHi);
            }
            ctx.moveTo(x, yHi);
            ctx.lineTo(x, yLo + 0.5);
            prevYMid = (yLo + yHi) / 2;
          }
        } else {
          let started = false;
          let prevY = 0;
          for (const s of visible) {
            const v = s.values[slot.channel];
            if (v === undefined) {
              started = false;
              continue;
            }
            const x = padL + plotW * (1 - (windowEnd - s.t) / windowMs);
            const y = padT + plotH * (1 - (v - min) / range);
            if (!started) {
              ctx.moveTo(x, y);
              started = true;
            } else {
              ctx.lineTo(x, prevY);
              ctx.lineTo(x, y);
            }
            prevY = y;
          }
        }
        ctx.stroke();
      }

      // Peak marker: flag the highest value in the visible window so a pull's
      // peak (e.g. 7500 rpm) is readable without hovering
      if (slot.channel && visible.length >= 2) {
        let peak: GraphSample | null = null;
        let peakV = -Infinity;
        for (const s of visible) {
          const v = s.values[slot.channel];
          if (v !== undefined && v > peakV) {
            peakV = v;
            peak = s;
          }
        }
        if (peak && isFinite(peakV) && range > 0) {
          const x = padL + plotW * (1 - (windowEnd - peak.t) / windowMs);
          const y = padT + plotH * (1 - (peakV - min) / range);
          ctx.fillStyle = slot.color;
          ctx.globalAlpha = 0.75;
          ctx.beginPath();
          ctx.arc(x, y, 2.5, 0, Math.PI * 2);
          ctx.fill();
          const text = formatTick(peakV);
          ctx.font = '11px monospace';
          const tw = ctx.measureText(text).width;
          const tx = Math.min(Math.max(x - tw / 2, padL + 2), padL + plotW - tw - 2);
          const ty = y > padT + 16 ? y - 15 : y + 6;
          ctx.fillStyle = bg;
          ctx.fillRect(tx - 2, ty - 1, tw + 4, 13);
          ctx.fillStyle = slot.color;
          ctx.textAlign = 'left';
          ctx.textBaseline = 'top';
          ctx.fillText(text, tx, ty);
          ctx.globalAlpha = 1;
        }
      }

      // Channel label + latest value in the window
      if (slot.channel) {
        const current = lastValue(slot.channel, visible);
        const label = `${slot.channel}${current !== undefined ? `: ${formatTick(current)}` : ''}`;
        ctx.font = 'bold 11px sans-serif';
        ctx.textBaseline = 'top';
        ctx.textAlign = side === 'left' ? 'left' : 'right';
        const x = side === 'left' ? padL + 6 : padL + plotW - 6;
        ctx.fillText(label, x, padT + 4);
      }
    }

    // Playback cursor
    if (cursorPosition !== null && cursorPosition !== undefined) {
      const x = padL + plotW * cursorPosition;
      ctx.strokeStyle = 'rgba(255,80,80,0.9)';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + plotH);
      ctx.stroke();
    }

    // Vertical marker at a sample with dots and value labels for both slots.
    const drawSampleMarker = (sample: GraphSample, lineStyle: string, lineW: number) => {
      const x = padL + plotW * (1 - (windowEnd - sample.t) / windowMs);
      if (x < padL - 1 || x > padL + plotW + 1) return;

      ctx.strokeStyle = lineStyle;
      ctx.lineWidth = lineW;
      ctx.beginPath();
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + plotH);
      ctx.stroke();

      for (const side of sides) {
        const slot = pane[side];
        if (!slot.channel) continue;
        const v = sample.values[slot.channel];
        if (v === undefined) continue;
        const { min, max } = slotBounds(slot, visible);
        const range = max - min || 1;
        const y = padT + plotH * (1 - (v - min) / range);

        // Marker dot on the line
        ctx.fillStyle = slot.color;
        ctx.beginPath();
        ctx.arc(x, y, 3, 0, Math.PI * 2);
        ctx.fill();

        // Value label beside the cursor, flipped near the right edge
        const text = formatTick(v);
        ctx.font = 'bold 13px monospace';
        const tw = ctx.measureText(text).width;
        const onLeft = x > padL + plotW - tw - 14;
        const tx = onLeft ? x - 6 - tw : x + 6;
        const ty = Math.min(Math.max(y - 8, padT + 2), padT + plotH - 16);
        ctx.fillStyle = bg;
        ctx.fillRect(tx - 2, ty - 1, tw + 4, 16);
        ctx.fillStyle = slot.color;
        ctx.textAlign = 'left';
        ctx.textBaseline = 'top';
        ctx.fillText(text, tx, ty);
      }
    };

    // Persistent data cursor (arrow-key navigable)
    if (cursorSample) {
      drawSampleMarker(cursorSample, 'rgba(79,143,232,0.95)', 1.5);
    }

    // Hover cursor: vertical bar snapped to the nearest sample
    if (hoverFrac !== null && visible.length > 0) {
      const tHover = windowEnd - windowMs * (1 - hoverFrac);
      const nearest = visible[nearestIndex(visible, tHover)];
      drawSampleMarker(nearest, 'rgba(220,225,235,0.6)', 1);
    }
  }, [pane, visible, windowMs, windowEnd, width, height, cursorPosition, hoverFrac, cursorSample]);

  const picker = (side: AxisSide) => (
    <select
      className={`graphlog-track-pick graphlog-track-${side}`}
      style={{ color: pane[side].color, borderColor: pane[side].color }}
      value={pane[side].channel ?? ''}
      title={`${side === 'left' ? 'Left' : 'Right'} trace`}
      aria-label={`${side === 'left' ? 'Left' : 'Right'} trace`}
      // Stops the click reaching the panes below, which would drop the data
      // cursor every time someone opened the list.
      onClick={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
      onChange={(e) => onPickChannel(side, e.target.value || null)}
    >
      <option value="">— none —</option>
      {groups.map((g) => (
        <optgroup key={g.label} label={g.label}>
          {g.channels.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </optgroup>
      ))}
    </select>
  );

  return (
    <div className="graphlog-pane" style={{ height }}>
      <canvas ref={canvasRef} style={{ width, height }} />
      {picker('left')}
      {picker('right')}
      <button
        type="button"
        className="graphlog-pane-config"
        title="Configure pane scales"
        onClick={onOpenConfig}
      >
        <Settings2 size={13} />
      </button>
    </div>
  );
};

interface SlotConfigProps {
  label: string;
  slot: ChannelSlot;
  availableChannels: string[];
  onChange: (patch: Partial<ChannelSlot>) => void;
}

/** Name-pattern channel groups so the picker isn't one 1000-entry list.
 *  First matching group wins; unmatched channels land in Other. */
const CHANNEL_GROUPS: Array<[string, RegExp]> = [
  ['Common', /^(rpm|map|tps|lambda|afr|coolant|iat|advance|pulseWidth|ve|boost|baro|battery|vBatt|afrTarget|targetLambda|vehicleSpeed|dwell|fuelLoad|ignitionLoad)$/i],
  ['Fueling', /fuel|inj|pulse|lambda|afr|^ve|enrich|wall|charge|stoich|flex/i],
  ['Ignition', /ign|spark|dwell|advance|timing|knock|coil/i],
  ['Boost / VVT', /boost|wastegate|ewg|vvt|turbo/i],
  ['Launch / ALS', /als|launch|antilag|shift|traction|torque/i],
  ['Wideband', /^wb\d|wideband|ego/i],
  ['Sensors', /temp|sens|clt|iat|baro|press|volt|batt|egt|oil|level|speed|gear/i],
];

function groupChannels(channels: string[]): Array<{ label: string; channels: string[] }> {
  const buckets = new Map<string, string[]>(CHANNEL_GROUPS.map(([l]) => [l, []]));
  buckets.set('Other', []);
  for (const c of channels) {
    const group = CHANNEL_GROUPS.find(([, re]) => re.test(c))?.[0] ?? 'Other';
    buckets.get(group)!.push(c);
  }
  const sort = (a: string, b: string) => a.toLowerCase().localeCompare(b.toLowerCase());
  return [...buckets.entries()]
    .filter(([, list]) => list.length > 0)
    .map(([label, list]) => ({ label, channels: list.sort(sort) }));
}

const SlotConfig: React.FC<SlotConfigProps> = ({ label, slot, availableChannels, onChange }) => {
  const groups = useMemo(() => groupChannels(availableChannels), [availableChannels]);
  return (
  <fieldset className="graphlog-slot-config">
    <legend style={{ color: slot.color }}>{label}</legend>
    <label>
      Channel
      <select
        value={slot.channel ?? ''}
        onChange={(e) => onChange({ channel: e.target.value || null })}
      >
        <option value="">— none —</option>
        {groups.map((g) => (
          <optgroup key={g.label} label={g.label}>
            {g.channels.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </optgroup>
        ))}
      </select>
    </label>
    <label className="graphlog-slot-auto">
      <input
        type="checkbox"
        checked={slot.auto}
        onChange={(e) => onChange({ auto: e.target.checked })}
      />
      Auto scale
    </label>
    <div className="graphlog-slot-range">
      <label>
        Min
        <input
          type="number"
          step="any"
          value={slot.min}
          disabled={slot.auto}
          onChange={(e) => {
            const v = parseFloat(e.target.value);
            if (!isNaN(v)) onChange({ min: v });
          }}
        />
      </label>
      <label>
        Max
        <input
          type="number"
          step="any"
          value={slot.max}
          disabled={slot.auto}
          onChange={(e) => {
            const v = parseFloat(e.target.value);
            if (!isNaN(v)) onChange({ max: v });
          }}
        />
      </label>
    </div>
  </fieldset>
  );
};

export const GraphLog: React.FC<GraphLogProps> = ({
  samples,
  availableChannels,
  cursorPosition = null,
}) => {
  const tabs = useGraphLogStore((s) => s.tabs);
  const activeTab = useGraphLogStore(selectActiveTab);
  const timeWindowSec = useGraphLogStore((s) => s.timeWindowSec);
  const { addTab, removeTab, renameTab, setActiveTab, setTimeWindow, updateSlot } =
    useGraphLogStore();

  const [renamingTabId, setRenamingTabId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [configPane, setConfigPane] = useState<number | null>(null);
  const [size, setSize] = useState({ width: 800, height: 480 });
  const [hoverFrac, setHoverFrac] = useState<number | null>(null);
  /** Right edge of the view in log time; null = follow the latest sample */
  const [viewEnd, setViewEnd] = useState<number | null>(null);
  /** Persistent data cursor position in log time; arrow keys step it */
  const [cursorT, setCursorT] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Refs so the zoom handlers (bound once for hotkeys) see current values
  const hoverFracRef = useRef<number | null>(null);
  const viewEndRef = useRef<number | null>(null);
  const cursorTRef = useRef<number | null>(null);
  const samplesRef = useRef<GraphSample[]>(samples);
  hoverFracRef.current = hoverFrac;
  viewEndRef.current = viewEnd;
  cursorTRef.current = cursorT;
  samplesRef.current = samples;

  /** Zoom keeping the time under the hover cursor fixed; anchors the right
   *  edge when the mouse isn't over the graphs. */
  const zoomBy = useCallback(
    (factor: number) => {
      const oldWin = useGraphLogStore.getState().timeWindowSec * 1000;
      const newWin = Math.min(600, Math.max(2, (oldWin / 1000) * factor)) * 1000;
      const data = samplesRef.current;
      const frac = hoverFracRef.current;
      if (data.length > 0 && frac !== null) {
        const lastT = data[data.length - 1].t;
        const curEnd = viewEndRef.current ?? lastT;
        const tCursor = curEnd - oldWin * (1 - frac);
        const newEnd = tCursor + (1 - frac) * newWin;
        setViewEnd(newEnd >= lastT ? null : Math.max(newEnd, data[0].t));
      }
      setTimeWindow(newWin / 1000);
    },
    [setTimeWindow],
  );

  const zoomIn = useCallback(() => zoomBy(ZOOM_FACTOR), [zoomBy]);
  const zoomOut = useCallback(() => zoomBy(1 / ZOOM_FACTOR), [zoomBy]);

  /** Step the data cursor by n samples, panning the view to keep it visible */
  const stepCursor = useCallback((delta: number) => {
    const data = samplesRef.current;
    if (data.length === 0) return;
    const cur = cursorTRef.current;
    let idx = cur === null ? data.length - 1 : nearestIndex(data, cur);
    if (cur !== null) idx += delta;
    idx = Math.min(data.length - 1, Math.max(0, idx));
    const t = data[idx].t;
    setCursorT(t);

    // Keep the cursor inside the view: nudge the window edge past it
    const winMs = useGraphLogStore.getState().timeWindowSec * 1000;
    const lastT = data[data.length - 1].t;
    const end = viewEndRef.current ?? lastT;
    if (t > end) {
      setViewEnd(t >= lastT ? null : t);
    } else if (t < end - winMs) {
      setViewEnd(Math.min(t + winMs, lastT));
    }
  }, []);

  // Q = zoom in, A = zoom out, arrows = step data cursor, Esc = clear cursor
  // (all ignored while typing in a form field)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') return;
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key === 'q' || e.key === 'Q') {
        zoomIn();
      } else if (e.key === 'a' || e.key === 'A') {
        zoomOut();
      } else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        e.preventDefault();
        const step = (e.shiftKey ? 10 : 1) * (e.key === 'ArrowLeft' ? -1 : 1);
        stepCursor(step);
      } else if (e.key === 'Escape') {
        setCursorT(null);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [zoomIn, zoomOut, stepCursor]);

  /** Click on the graphs places the data cursor at the nearest sample */
  const handlePanesClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (draggedRef.current) {
      draggedRef.current = false;
      return;
    }
    const data = samplesRef.current;
    if (data.length === 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const frac = (e.clientX - rect.left - PAD_L) / Math.max(1, rect.width - PAD_L - PAD_R);
    if (frac < 0 || frac > 1) return;
    const winMs = useGraphLogStore.getState().timeWindowSec * 1000;
    const lastT = data[data.length - 1].t;
    const end = viewEndRef.current ?? lastT;
    const t = end - winMs * (1 - frac);
    setCursorT(data[nearestIndex(data, t)].t);
  }, []);

  /** Move the right edge of the view to `next`, clamped to the recorded span.
   *
   *  Scrolling past the newest sample means "follow the live edge" rather than
   *  panning into empty space, so it reverts to `null`. The other end stops
   *  where a full window still has data behind it - and a log shorter than the
   *  window has nowhere to scroll, so it stays following rather than showing a
   *  Latest button that would do nothing.
   */
  const setViewEndClamped = useCallback((next: number) => {
    const data = samplesRef.current;
    if (data.length === 0) return;
    const winMs = useGraphLogStore.getState().timeWindowSec * 1000;
    const lastT = data[data.length - 1].t;
    const earliestEnd = Math.min(lastT, data[0].t + winMs);
    const clamped = Math.max(next, earliestEnd);
    setViewEnd(clamped >= lastT ? null : clamped);
  }, []);

  /** Shift the view by a fraction of the visible window. */
  const panByFraction = useCallback(
    (frac: number) => {
      const data = samplesRef.current;
      if (data.length === 0) return;
      const winMs = useGraphLogStore.getState().timeWindowSec * 1000;
      const end = viewEndRef.current ?? data[data.length - 1].t;
      setViewEndClamped(end + frac * winMs);
    },
    [setViewEndClamped],
  );

  /** Drag origin: where the pointer went down, and the view edge at that moment. */
  const dragRef = useRef<{ x: number; end: number; width: number; moved: boolean } | null>(null);

  const handlePanesMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0 || samplesRef.current.length === 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const data = samplesRef.current;
    dragRef.current = {
      x: e.clientX,
      end: viewEndRef.current ?? data[data.length - 1].t,
      width: Math.max(1, rect.width - PAD_L - PAD_R),
      moved: false,
    };
  }, []);

  const handlePanesMouseMove = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const frac = (e.clientX - rect.left - PAD_L) / Math.max(1, rect.width - PAD_L - PAD_R);
    setHoverFrac(frac >= 0 && frac <= 1 ? frac : null);

    const drag = dragRef.current;
    if (!drag) return;
    const dx = e.clientX - drag.x;
    // A few pixels of slop, so a click that wobbles still places the cursor
    // rather than being swallowed as a pan.
    if (!drag.moved && Math.abs(dx) < 4) return;
    drag.moved = true;
    const winMs = useGraphLogStore.getState().timeWindowSec * 1000;
    // Drag right pulls earlier time into view, the way dragging paper does.
    setViewEndClamped(drag.end - (dx / drag.width) * winMs);
  }, [setViewEndClamped]);

  /** Set on mouseup when the gesture turned out to be a pan, so the click that
   *  follows does not also drop the data cursor where the drag ended. */
  const draggedRef = useRef(false);

  const endDrag = useCallback(() => {
    draggedRef.current = dragRef.current?.moved ?? false;
    dragRef.current = null;
  }, []);

  // Wheel over the graphs zooms about the pointer; Shift makes it scroll along
  // the time axis instead. Registered non-passively because the default action
  // would scroll whatever container the graphs happen to sit in — here, the Log
  // Analyze tab.
  const panesRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = panesRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (samplesRef.current.length === 0) return;
      e.preventDefault();
      const delta = e.deltaY !== 0 ? e.deltaY : e.deltaX;
      if (e.shiftKey) {
        panByFraction(delta > 0 ? 0.15 : -0.15);
      } else {
        zoomBy(delta < 0 ? ZOOM_FACTOR : 1 / ZOOM_FACTOR);
      }
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, [zoomBy, panByFraction]);

  // A real scrollbar for the time axis. Built on a native overflow container
  // rather than a drawn widget, so it gets the platform's thumb, click-the-
  // track paging and keyboard behaviour for free — and shows how much of the
  // log is on screen, which zoom and pan alone never say.
  const hScrollRef = useRef<HTMLDivElement>(null);
  /** Set while syncing scrollLeft from the view, so the resulting scroll event
   *  is not fed back in as a user pan. */
  const syncingScroll = useRef(false);

  const handleHScroll = useCallback(() => {
    if (syncingScroll.current) return;
    const el = hScrollRef.current;
    const data = samplesRef.current;
    if (!el || data.length === 0) return;
    const spanMs = data[data.length - 1].t - data[0].t;
    const scrollable = el.scrollWidth - el.clientWidth;
    if (scrollable <= 0 || spanMs <= 0) return;
    const winMs = useGraphLogStore.getState().timeWindowSec * 1000;
    const frac = el.scrollLeft / scrollable;
    // The thumb spans the window, so the reachable start range is what is left
    // of the log once a window is subtracted.
    setViewEndClamped(data[0].t + frac * Math.max(0, spanMs - winMs) + winMs);
  }, [setViewEndClamped]);

  // Track container size
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      const rect = entries[0].contentRect;
      setSize({ width: Math.max(300, rect.width), height: Math.max(200, rect.height) });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const windowMs = timeWindowSec * 1000;
  const latestT = samples.length > 0 ? samples[samples.length - 1].t : 0;
  const windowEnd = viewEnd !== null ? Math.min(viewEnd, latestT) : latestT;
  const windowStart = windowEnd - windowMs;
  const isFollowing = viewEnd === null;
  const visible = useMemo(() => {
    const startIdx = samples.findIndex((s) => s.t >= windowStart);
    if (startIdx < 0) return [];
    let endIdx = samples.length;
    while (endIdx > startIdx && samples[endIdx - 1].t > windowEnd) endIdx--;
    return samples.slice(startIdx, endIdx);
  }, [samples, windowStart, windowEnd]);

  const cursorSample = useMemo(
    () => (cursorT !== null && samples.length > 0 ? samples[nearestIndex(samples, cursorT)] : null),
    [cursorT, samples],
  );

  // How long the log is, in windows. 100% means it all fits and there is
  // nothing to scroll, so the bar sits inert at full width.
  const logSpanMs = samples.length > 1 ? samples[samples.length - 1].t - samples[0].t : 0;
  const scrollStripPercent = Math.max(100, (logSpanMs / Math.max(1, windowMs)) * 100);

  // Keep the thumb where the view is, whoever moved it — wheel, drag, keys or
  // the Latest button.
  useEffect(() => {
    const el = hScrollRef.current;
    if (!el || logSpanMs <= 0) return;
    const scrollable = el.scrollWidth - el.clientWidth;
    if (scrollable <= 0) return;
    const startFrac = (windowStart - samples[0].t) / Math.max(1, logSpanMs - windowMs);
    const target = Math.max(0, Math.min(1, startFrac)) * scrollable;
    if (Math.abs(el.scrollLeft - target) < 1) return;
    syncingScroll.current = true;
    el.scrollLeft = target;
    // Cleared after the scroll event this triggers has been delivered.
    requestAnimationFrame(() => {
      syncingScroll.current = false;
    });
  }, [windowStart, windowMs, logSpanMs, samples]);

  const visiblePanes = activeTab.panes.filter((p) => !p.hidden);
  const timeAxisHeight = 22;
  const paneHeight = Math.max(
    PANE_MIN_HEIGHT,
    Math.floor((size.height - timeAxisHeight) / Math.max(1, visiblePanes.length)),
  );

  const commitRename = () => {
    if (renamingTabId) renameTab(renamingTabId, renameValue);
    setRenamingTabId(null);
  };

  // Time axis labels (relative to the start of the log)
  const timeLabels = useMemo(() => {
    const labels: Array<{ frac: number; text: string }> = [];
    const steps = 6;
    const base = samples.length > 0 ? samples[0].t : windowStart;
    for (let i = 0; i <= steps; i++) {
      const frac = i / steps;
      const t = windowStart + windowMs * frac;
      labels.push({ frac, text: formatClock(t - base) });
    }
    return labels;
  }, [windowStart, windowMs, samples]);

  return (
    <div className="graphlog" ref={containerRef}>
      <div className="graphlog-tabbar">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            className={`graphlog-tab${tab.id === activeTab.id ? ' active' : ''}`}
            onClick={() => setActiveTab(tab.id)}
            onDoubleClick={() => {
              setRenamingTabId(tab.id);
              setRenameValue(tab.name);
            }}
          >
            {renamingTabId === tab.id ? (
              <input
                autoFocus
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                onBlur={commitRename}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') commitRename();
                  if (e.key === 'Escape') setRenamingTabId(null);
                }}
                onClick={(e) => e.stopPropagation()}
              />
            ) : (
              <span>{tab.name}</span>
            )}
            {tabs.length > 1 && (
              <button
                type="button"
                className="graphlog-tab-close"
                title="Remove tab"
                onClick={(e) => {
                  e.stopPropagation();
                  removeTab(tab.id);
                }}
              >
                <X size={11} />
              </button>
            )}
          </div>
        ))}
        <button type="button" className="graphlog-tab-add" title="Add tab" onClick={addTab}>
          <Plus size={13} />
        </button>
        <div className="graphlog-window-select">
          {cursorSample && samples.length > 0 && (
            <span className="graphlog-cursor-time" title="Data cursor (←/→ to step, Esc to clear)">
              ▸ {formatClock(cursorSample.t - samples[0].t)}
            </span>
          )}
          {!isFollowing && (
            <button
              type="button"
              className="graphlog-latest-btn"
              onClick={() => setViewEnd(null)}
              title="Jump back to the newest data"
            >
              Latest
            </button>
          )}
          <button type="button" onClick={zoomIn} title="Zoom in (Q)">
            <ZoomIn size={14} />
          </button>
          <span className="graphlog-window-label">{formatWindow(timeWindowSec)}</span>
          <button type="button" onClick={zoomOut} title="Zoom out (A)">
            <ZoomOut size={14} />
          </button>
        </div>
      </div>

      <div
        ref={panesRef}
        className="graphlog-panes"
        title="Drag to scroll - wheel to zoom - Shift+wheel to scroll - click to place the cursor"
        onMouseDown={handlePanesMouseDown}
        onMouseMove={handlePanesMouseMove}
        onMouseUp={endDrag}
        onMouseLeave={() => {
          setHoverFrac(null);
          endDrag();
        }}
        onClick={handlePanesClick}
      >
        {samples.length === 0 && (
          <div className="graphlog-empty-hint">Press Record to start logging</div>
        )}
        {visiblePanes.map((pane) => {
          const paneIndex = activeTab.panes.indexOf(pane);
          return (
            <PaneCanvas
              key={paneIndex}
              pane={pane}
              visible={visible}
              windowMs={windowMs}
              windowEnd={windowEnd}
              width={size.width}
              height={paneHeight}
              cursorPosition={cursorPosition}
              hoverFrac={hoverFrac}
              cursorSample={cursorSample}
              onOpenConfig={() => setConfigPane(paneIndex)}
              availableChannels={availableChannels}
              onPickChannel={(side, channel) =>
                updateSlot(activeTab.id, paneIndex, side, { channel })
              }
            />
          );
        })}
        <div className="graphlog-timeaxis" style={{ height: timeAxisHeight }}>
          {timeLabels.map((l) => (
            <span key={l.frac} style={{ left: `calc(52px + (100% - 104px) * ${l.frac})` }}>
              {l.text}
            </span>
          ))}
        </div>
      </div>

      {/* Width of the inner strip sets the thumb size: the track is one window
          wide, so the strip is as many windows long as the log lasts. */}
      <div
        ref={hScrollRef}
        className="graphlog-hscroll"
        onScroll={handleHScroll}
        title="Scroll through the log"
      >
        <div style={{ width: `${scrollStripPercent}%` }} />
      </div>

      <Dialog
        open={configPane !== null}
        onClose={() => setConfigPane(null)}
        title={`Graph ${configPane !== null ? configPane + 1 : ''} — channels & scales`}
        size="sm"
        className="graphlog-config-dialog"
      >
        <Dialog.Body>
          {configPane !== null && (
            <>
              <SlotConfig
                label="Left axis"
                slot={activeTab.panes[configPane].left}
                availableChannels={availableChannels}
                onChange={(patch) => updateSlot(activeTab.id, configPane, 'left', patch)}
              />
              <SlotConfig
                label="Right axis"
                slot={activeTab.panes[configPane].right}
                availableChannels={availableChannels}
                onChange={(patch) => updateSlot(activeTab.id, configPane, 'right', patch)}
              />
            </>
          )}
        </Dialog.Body>
      </Dialog>
    </div>
  );
};

export default GraphLog;
