/**
 * Graph Log — TunerStudio/ECUMaster-style stacked strip charts.
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
  onOpenConfig: () => void;
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
  onOpenConfig,
}) => {
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

      // Polyline
      if (slot.channel && visible.length >= 2) {
        ctx.strokeStyle = slot.color;
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        let started = false;
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
            ctx.lineTo(x, y);
          }
        }
        ctx.stroke();
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

    // Hover cursor: vertical bar snapped to the nearest sample, with the
    // value of each channel at that moment
    if (hoverFrac !== null && visible.length > 0) {
      const tHover = windowEnd - windowMs * (1 - hoverFrac);
      let nearest = visible[0];
      let bestDist = Math.abs(nearest.t - tHover);
      for (const s of visible) {
        const d = Math.abs(s.t - tHover);
        if (d < bestDist) {
          bestDist = d;
          nearest = s;
        }
      }
      const x = padL + plotW * (1 - (windowEnd - nearest.t) / windowMs);

      ctx.strokeStyle = 'rgba(220,225,235,0.75)';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + plotH);
      ctx.stroke();

      for (const side of sides) {
        const slot = pane[side];
        if (!slot.channel) continue;
        const v = nearest.values[slot.channel];
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
        ctx.font = 'bold 10px monospace';
        const tw = ctx.measureText(text).width;
        const onLeft = x > padL + plotW - tw - 14;
        const tx = onLeft ? x - 6 - tw : x + 6;
        const ty = Math.min(Math.max(y - 6, padT + 2), padT + plotH - 12);
        ctx.fillStyle = bg;
        ctx.fillRect(tx - 2, ty - 1, tw + 4, 12);
        ctx.fillStyle = slot.color;
        ctx.textAlign = 'left';
        ctx.textBaseline = 'top';
        ctx.fillText(text, tx, ty);
      }
    }
  }, [pane, visible, windowMs, windowEnd, width, height, cursorPosition, hoverFrac]);

  return (
    <div className="graphlog-pane" style={{ height }}>
      <canvas ref={canvasRef} style={{ width, height }} />
      <button
        type="button"
        className="graphlog-pane-config"
        title="Configure pane channels and scales"
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

const SlotConfig: React.FC<SlotConfigProps> = ({ label, slot, availableChannels, onChange }) => (
  <fieldset className="graphlog-slot-config">
    <legend style={{ color: slot.color }}>{label}</legend>
    <label>
      Channel
      <select
        value={slot.channel ?? ''}
        onChange={(e) => onChange({ channel: e.target.value || null })}
      >
        <option value="">— none —</option>
        {availableChannels.map((c) => (
          <option key={c} value={c}>
            {c}
          </option>
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
  const containerRef = useRef<HTMLDivElement>(null);

  const zoomIn = useCallback(() => {
    setTimeWindow(useGraphLogStore.getState().timeWindowSec * ZOOM_FACTOR);
  }, [setTimeWindow]);
  const zoomOut = useCallback(() => {
    setTimeWindow(useGraphLogStore.getState().timeWindowSec / ZOOM_FACTOR);
  }, [setTimeWindow]);

  // Q = zoom in, A = zoom out (ignored while typing in a form field)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') return;
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key === 'q' || e.key === 'Q') {
        zoomIn();
      } else if (e.key === 'a' || e.key === 'A') {
        zoomOut();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [zoomIn, zoomOut]);

  const handlePanesMouseMove = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const frac = (e.clientX - rect.left - PAD_L) / Math.max(1, rect.width - PAD_L - PAD_R);
    setHoverFrac(frac >= 0 && frac <= 1 ? frac : null);
  }, []);

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
  const windowEnd = samples.length > 0 ? samples[samples.length - 1].t : 0;
  const windowStart = windowEnd - windowMs;
  const visible = useMemo(() => {
    const startIdx = samples.findIndex((s) => s.t >= windowStart);
    return startIdx < 0 ? [] : samples.slice(startIdx);
  }, [samples, windowStart]);

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
        className="graphlog-panes"
        onMouseMove={handlePanesMouseMove}
        onMouseLeave={() => setHoverFrac(null)}
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
              onOpenConfig={() => setConfigPane(paneIndex)}
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
