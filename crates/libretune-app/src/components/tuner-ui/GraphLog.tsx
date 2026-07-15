/**
 * Graph Log — TunerStudio/ECUMaster-style stacked strip charts.
 *
 * Renders the active graph-log tab as PANES_PER_TAB stacked panes sharing a
 * time axis. Each pane plots one channel against the left axis and one against
 * the right, each with its own fixed or auto scale.
 *
 * Data source: when a `samples` array is supplied (recording or log playback)
 * it is drawn as-is; otherwise the component samples the realtime store itself
 * so the view scrolls live without requiring a recording.
 */
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Plus, X, Settings2 } from 'lucide-react';
import {
  useGraphLogStore,
  selectActiveTab,
  ChannelSlot,
  GraphPane,
  AxisSide,
} from '../../stores/graphLogStore';
import { useRealtimeStore } from '../../stores/realtimeStore';
import './GraphLog.css';

export interface GraphSample {
  /** Timestamp in milliseconds (epoch or log-relative — only deltas matter) */
  t: number;
  values: Record<string, number>;
}

export interface GraphLogProps {
  /** Recorded/playback samples. When empty, the component live-samples the realtime store. */
  samples?: GraphSample[];
  /** Channels the user may assign to slots */
  availableChannels: string[];
  /** Cursor position 0..1 across the visible window (playback), or null */
  cursorPosition?: number | null;
}

/** Live sampling rate for the self-fed buffer (ms) */
const LIVE_SAMPLE_MS = 50;
/** Maximum retained live history (ms) */
const LIVE_RETENTION_MS = 5 * 60 * 1000;

const AXIS_TICKS = 5;
const PANE_MIN_HEIGHT = 70;

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

interface PaneCanvasProps {
  pane: GraphPane;
  visible: GraphSample[];
  windowMs: number;
  windowEnd: number;
  width: number;
  height: number;
  liveValues: Record<string, number>;
  cursorPosition?: number | null;
  onOpenConfig: () => void;
}

const PaneCanvas: React.FC<PaneCanvasProps> = ({
  pane,
  visible,
  windowMs,
  windowEnd,
  width,
  height,
  liveValues,
  cursorPosition,
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

    const padL = 52;
    const padR = 52;
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

      // Channel label + current value
      if (slot.channel) {
        const current = liveValues[slot.channel];
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
  }, [pane, visible, windowMs, windowEnd, width, height, liveValues, cursorPosition]);

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
  const containerRef = useRef<HTMLDivElement>(null);

  // Live self-sampling buffer, used when no external samples are provided.
  const liveBufferRef = useRef<GraphSample[]>([]);
  const [liveTick, setLiveTick] = useState(0);
  const external = samples !== undefined && samples.length > 0;

  const neededChannels = useMemo(() => {
    const set = new Set<string>();
    for (const pane of activeTab.panes) {
      if (pane.left.channel) set.add(pane.left.channel);
      if (pane.right.channel) set.add(pane.right.channel);
    }
    return Array.from(set);
  }, [activeTab]);

  useEffect(() => {
    if (external) return;
    const interval = window.setInterval(() => {
      const channels = useRealtimeStore.getState().channels;
      const values: Record<string, number> = {};
      let any = false;
      for (const name of neededChannels) {
        const v = channels[name];
        if (v !== undefined) {
          values[name] = v;
          any = true;
        }
      }
      if (!any) return;
      const buf = liveBufferRef.current;
      const now = Date.now();
      buf.push({ t: now, values });
      const cutoff = now - LIVE_RETENTION_MS;
      while (buf.length > 0 && buf[0].t < cutoff) buf.shift();
      setLiveTick((n) => n + 1);
    }, LIVE_SAMPLE_MS);
    return () => window.clearInterval(interval);
  }, [external, neededChannels]);

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

  const data: GraphSample[] = external ? samples! : liveBufferRef.current;

  const windowMs = timeWindowSec * 1000;
  const windowEnd = data.length > 0 ? data[data.length - 1].t : Date.now();
  const windowStart = windowEnd - windowMs;
  const visible = useMemo(() => {
    const startIdx = data.findIndex((s) => s.t >= windowStart);
    return startIdx < 0 ? [] : data.slice(startIdx);
  }, [data, windowStart, liveTick]); // eslint-disable-line react-hooks/exhaustive-deps

  const liveValues = useRealtimeStore((s) => s.channels);

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

  // Time axis labels (relative to window end)
  const timeLabels = useMemo(() => {
    const labels: Array<{ frac: number; text: string }> = [];
    const steps = 6;
    const base = data.length > 0 ? data[0].t : windowStart;
    for (let i = 0; i <= steps; i++) {
      const frac = i / steps;
      const t = windowStart + windowMs * frac;
      labels.push({ frac, text: formatClock(t - base) });
    }
    return labels;
  }, [windowStart, windowMs, data]);

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
          <label>
            Window
            <select
              value={timeWindowSec}
              onChange={(e) => setTimeWindow(parseInt(e.target.value, 10))}
            >
              <option value={10}>10 s</option>
              <option value={30}>30 s</option>
              <option value={60}>1 min</option>
              <option value={120}>2 min</option>
              <option value={300}>5 min</option>
            </select>
          </label>
        </div>
      </div>

      <div className="graphlog-panes">
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
              liveValues={liveValues}
              cursorPosition={cursorPosition}
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

      {configPane !== null && (
        <div className="graphlog-config-overlay" onClick={() => setConfigPane(null)}>
          <div className="graphlog-config" onClick={(e) => e.stopPropagation()}>
            <div className="graphlog-config-header">
              <h3>Graph {configPane + 1} — channels &amp; scales</h3>
              <button type="button" onClick={() => setConfigPane(null)} aria-label="Close">
                <X size={14} />
              </button>
            </div>
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
          </div>
        </div>
      )}
    </div>
  );
};

export default GraphLog;
