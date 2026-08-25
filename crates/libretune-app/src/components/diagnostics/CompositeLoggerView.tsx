/**
 * Composite Logger Visualization Component
 * 
 * Displays combined crank and cam trigger patterns from the ECU.
 * Shows primary trigger, secondary trigger, and sync status over time
 * to diagnose trigger wheel alignment and sync issues.
 */

import React, { useState, useEffect, useRef, useCallback } from "react";
import { Square, Play, Download, X, Check, XCircle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./CompositeLoggerView.css";

interface CompositeLogEntry {
  time_us: number;
  primary: boolean;
  secondary: boolean;
  sync: boolean;
  voltage?: number;
}

/** A record as `start_tooth_capture` emits it: INI field names to scaled values. */
export interface LoggerRecord {
  index: number;
  fields: Record<string, number>;
}

interface CaptureStatus {
  logger: string;
  records: number;
  reads: number;
  emptyReads: number;
  running: boolean;
  note: string | null;
}

interface DiagnosticLogger {
  name: string;
  label: string;
  kind: string;
}

/** A long capture exceeds anything a trigger scope can usefully draw. */
const MAX_POINTS = 20000;

/**
 * Map INI-named record fields onto the view's entries.
 *
 * The INI declares what each bit of a composite record means - Speeduino and
 * rusEFI both name them priLevel/secLevel/sync, with `time` as a scaled
 * running timestamp in milliseconds. Matching by name rather than by position
 * is what lets this work off the definition instead of a guess.
 */
export function toEntries(records: LoggerRecord[]): CompositeLogEntry[] {
  const out: CompositeLogEntry[] = [];
  for (const r of records) {
    // Units differ per field and the INI says which: `time` and `refTime` are
    // declared scale 0.001 in ms, `toothTime` scale 1.0 already in
    // microseconds. Converting all three alike puts toothTime out by 1000x.
    const ms = r.fields.time ?? r.fields.refTime;
    const timeUs = ms !== undefined ? ms * 1000 : r.fields.toothTime;
    if (timeUs === undefined) continue;
    out.push({
      time_us: Math.round(timeUs),
      primary: (r.fields.priLevel ?? 0) !== 0,
      secondary: (r.fields.secLevel ?? 0) !== 0,
      sync: (r.fields.sync ?? 0) !== 0,
      voltage: r.fields.voltage,
    });
  }
  return out;
}

interface CompositeLoggerViewProps {
  onClose?: () => void;
}

export const CompositeLoggerView: React.FC<CompositeLoggerViewProps> = ({ onClose }) => {
  const [logData, setLogData] = useState<CompositeLogEntry[]>([]);
  const [isCapturing, setIsCapturing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [zoomLevel, setZoomLevel] = useState(1);
  const [scrollOffset, setScrollOffset] = useState(0);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Listen for real-time composite data
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      // `tooth-log-records` streams batches while a capture runs, decoded
      // against the record layout the INI declares. Append rather than
      // replace: batches arrive throughout the capture.
      unlisten = await listen<LoggerRecord[]>("tooth-log-records", (event) => {
        const mapped = toEntries(event.payload);
        setLogData((prev) => {
          const next = prev.concat(mapped);
          return next.length > MAX_POINTS ? next.slice(-MAX_POINTS) : next;
        });
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Draw composite waveform
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || logData.length === 0) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const padding = { left: 80, right: 20, top: 20, bottom: 40 };
    const plotWidth = width - padding.left - padding.right;
    const plotHeight = height - padding.top - padding.bottom;

    // Clear canvas
    ctx.fillStyle = "#0f0f1a";
    ctx.fillRect(0, 0, width, height);

    // Calculate visible range based on zoom and scroll
    const totalSamples = logData.length;
    const visibleSamples = Math.floor(totalSamples / zoomLevel);
    const startSample = Math.floor(scrollOffset * (totalSamples - visibleSamples));
    const endSample = Math.min(startSample + visibleSamples, totalSamples);

    // Channel heights
    const channelHeight = plotHeight / 4;
    const channels = [
      { name: "Primary", color: "#22c55e", yOffset: 0 },
      { name: "Secondary", color: "#3b82f6", yOffset: channelHeight },
      { name: "Sync", color: "#a855f7", yOffset: channelHeight * 2 },
      { name: "Voltage", color: "#f59e0b", yOffset: channelHeight * 3 },
    ];

    // Draw channel labels
    ctx.font = "12px sans-serif";
    ctx.textAlign = "right";
    channels.forEach((ch) => {
      const y = padding.top + ch.yOffset + channelHeight / 2;
      ctx.fillStyle = ch.color;
      ctx.fillText(ch.name, padding.left - 10, y + 4);
    });

    // Draw channel separators
    ctx.strokeStyle = "#333";
    ctx.lineWidth = 1;
    channels.forEach((ch) => {
      const y = padding.top + ch.yOffset + channelHeight;
      ctx.beginPath();
      ctx.moveTo(padding.left, y);
      ctx.lineTo(width - padding.right, y);
      ctx.stroke();
    });

    // Draw waveforms
    const samplesPerPixel = visibleSamples / plotWidth;

    // Primary trigger (digital)
    ctx.strokeStyle = channels[0].color;
    ctx.lineWidth = 2;
    ctx.beginPath();
    let lastPrimary = false;
    for (let px = 0; px < plotWidth; px++) {
      const sampleIdx = startSample + Math.floor(px * samplesPerPixel);
      if (sampleIdx >= logData.length) break;

      const entry = logData[sampleIdx];
      const x = padding.left + px;
      const highY = padding.top + channels[0].yOffset + 5;
      const lowY = padding.top + channels[0].yOffset + channelHeight - 5;
      const y = entry.primary ? highY : lowY;

      if (px === 0) {
        ctx.moveTo(x, y);
        lastPrimary = entry.primary;
      } else {
        if (entry.primary !== lastPrimary) {
          // Draw vertical transition
          ctx.lineTo(x, lastPrimary ? highY : lowY);
          ctx.lineTo(x, y);
          lastPrimary = entry.primary;
        }
        ctx.lineTo(x, y);
      }
    }
    ctx.stroke();

    // Secondary trigger (digital)
    ctx.strokeStyle = channels[1].color;
    ctx.beginPath();
    let lastSecondary = false;
    for (let px = 0; px < plotWidth; px++) {
      const sampleIdx = startSample + Math.floor(px * samplesPerPixel);
      if (sampleIdx >= logData.length) break;

      const entry = logData[sampleIdx];
      const x = padding.left + px;
      const highY = padding.top + channels[1].yOffset + 5;
      const lowY = padding.top + channels[1].yOffset + channelHeight - 5;
      const y = entry.secondary ? highY : lowY;

      if (px === 0) {
        ctx.moveTo(x, y);
        lastSecondary = entry.secondary;
      } else {
        if (entry.secondary !== lastSecondary) {
          ctx.lineTo(x, lastSecondary ? highY : lowY);
          ctx.lineTo(x, y);
          lastSecondary = entry.secondary;
        }
        ctx.lineTo(x, y);
      }
    }
    ctx.stroke();

    // Sync indicator (digital)
    ctx.strokeStyle = channels[2].color;
    ctx.beginPath();
    let lastSync = false;
    for (let px = 0; px < plotWidth; px++) {
      const sampleIdx = startSample + Math.floor(px * samplesPerPixel);
      if (sampleIdx >= logData.length) break;

      const entry = logData[sampleIdx];
      const x = padding.left + px;
      const highY = padding.top + channels[2].yOffset + 5;
      const lowY = padding.top + channels[2].yOffset + channelHeight - 5;
      const y = entry.sync ? highY : lowY;

      if (px === 0) {
        ctx.moveTo(x, y);
        lastSync = entry.sync;
      } else {
        if (entry.sync !== lastSync) {
          ctx.lineTo(x, lastSync ? highY : lowY);
          ctx.lineTo(x, y);
          lastSync = entry.sync;
        }
        ctx.lineTo(x, y);
      }
    }
    ctx.stroke();

    // Voltage (analog, if available)
    const hasVoltage = logData.some((e) => e.voltage !== undefined);
    if (hasVoltage) {
      ctx.strokeStyle = channels[3].color;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      const voltageMin = 0;
      const voltageMax = 5; // 0-5V range

      for (let px = 0; px < plotWidth; px++) {
        const sampleIdx = startSample + Math.floor(px * samplesPerPixel);
        if (sampleIdx >= logData.length) break;

        const entry = logData[sampleIdx];
        const x = padding.left + px;
        const voltage = entry.voltage ?? 0;
        const normalizedV = (voltage - voltageMin) / (voltageMax - voltageMin);
        const y = padding.top + channels[3].yOffset + channelHeight - 5 - normalizedV * (channelHeight - 10);

        if (px === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      }
      ctx.stroke();
    }

    // Draw time axis
    ctx.fillStyle = "#888";
    ctx.font = "11px monospace";
    ctx.textAlign = "center";
    const timeStart = logData[startSample]?.time_us || 0;
    const timeEnd = logData[endSample - 1]?.time_us || 0;
    const timeDuration = timeEnd - timeStart;

    for (let i = 0; i <= 10; i++) {
      const x = padding.left + (plotWidth * i) / 10;
      const time = timeStart + (timeDuration * i) / 10;
      ctx.fillText(`${(time / 1000).toFixed(1)}ms`, x, height - padding.bottom + 20);
    }

  }, [logData, zoomLevel, scrollOffset]);

  const handleCapture = useCallback(async () => {
    setIsCapturing(true);
    setError(null);
    setLogData([]);

    try {
      // Ask the definition which logger draws trigger patterns rather than
      // assuming a command. The previous path sent a hardcoded 'O' - which is
      // a different logger's START command on Speeduino - then decoded the
      // reply as one packed byte per sample and stamped it at an invented
      // 10 kHz. Every waveform it drew was fiction.
      const loggers = await invoke<DiagnosticLogger[]>("list_diagnostic_loggers");
      const composite = loggers.find((l) => l.kind === "composite");
      if (!composite) {
        setError("This ECU definition declares no composite (trigger) logger.");
        return;
      }

      const status = await invoke<CaptureStatus>("start_tooth_capture", {
        loggerName: composite.name,
      });
      if (status.note) setError(status.note);
      if (status.records === 0) {
        setError(
          status.note ?? "No records captured. The logger needs the engine turning."
        );
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsCapturing(false);
    }
  }, []);

  const handleStop = useCallback(async () => {
    try {
      await invoke("stop_tooth_capture");
    } catch (err) {
      console.error("Failed to stop composite capture:", err);
    }
    setIsCapturing(false);
  }, []);

  const handleExport = useCallback(() => {
    if (logData.length === 0) return;

    const lines = ["Time (µs),Primary,Secondary,Sync,Voltage"];
    logData.forEach((entry) => {
      lines.push(
        `${entry.time_us},${entry.primary ? 1 : 0},${entry.secondary ? 1 : 0},${entry.sync ? 1 : 0},${entry.voltage ?? ""}`
      );
    });

    const blob = new Blob([lines.join("\n")], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "composite_log.csv";
    a.click();
    URL.revokeObjectURL(url);
  }, [logData]);

  // Measured from the timestamps the ECU reported, not assumed. The old path
  // displayed a flat 10 kHz that was never read from anything.
  const sampleRate = React.useMemo(() => {
    if (logData.length < 2) return null;
    const span = logData[logData.length - 1].time_us - logData[0].time_us;
    if (span <= 0) return null;
    return ((logData.length - 1) * 1_000_000) / span;
  }, [logData]);

  // Calculate statistics
  const stats = React.useMemo(() => {
    if (logData.length < 2) return null;

    const duration = (logData[logData.length - 1].time_us - logData[0].time_us) / 1000;
    const primaryRisings = logData.filter((e, i) => i > 0 && e.primary && !logData[i - 1].primary).length;
    const secondaryRisings = logData.filter((e, i) => i > 0 && e.secondary && !logData[i - 1].secondary).length;
    const syncAcquired = logData.some((e) => e.sync);
    const syncLostCount = logData.filter((e, i) => i > 0 && !e.sync && logData[i - 1].sync).length;

    return {
      duration,
      primaryPulses: primaryRisings,
      secondaryPulses: secondaryRisings,
      syncAcquired,
      syncLostCount,
    };
  }, [logData]);

  return (
    <div className="composite-logger-view">
      <div className="composite-logger-header">
        <h2>Composite Logger</h2>
        <div className="composite-logger-controls">
          <button
            className={`capture-btn ${isCapturing ? "capturing" : ""}`}
            onClick={isCapturing ? handleStop : handleCapture}
          >
            {isCapturing ? <><Square size={14} fill="currentColor" /> Stop</> : <><Play size={14} fill="currentColor" /> Capture</>}
          </button>
          <div className="zoom-controls">
            <label>Zoom:</label>
            <input
              type="range"
              min="1"
              max="10"
              step="0.5"
              value={zoomLevel}
              onChange={(e) => setZoomLevel(parseFloat(e.target.value))}
            />
            <span>{zoomLevel.toFixed(1)}x</span>
          </div>
          <button
            className="export-btn"
            onClick={handleExport}
            disabled={logData.length === 0}
          >
            <Download size={14} /> Export
          </button>
          {onClose && (
            <button className="close-btn" onClick={onClose} aria-label="Close">
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      {error && <div className="composite-logger-error">{error}</div>}

      <div className="composite-logger-stats">
        <div className="stat">
          <span className="stat-label">Sample Rate:</span>
          <span className="stat-value">
            {sampleRate === null ? "—" : `${(sampleRate / 1000).toFixed(1)} kHz`}
          </span>
        </div>
        <div className="stat">
          <span className="stat-label">Samples:</span>
          <span className="stat-value">{logData.length.toLocaleString()}</span>
        </div>
        {stats && (
          <>
            <div className="stat">
              <span className="stat-label">Duration:</span>
              <span className="stat-value">{stats.duration.toFixed(1)} ms</span>
            </div>
            <div className="stat">
              <span className="stat-label">Primary Pulses:</span>
              <span className="stat-value">{stats.primaryPulses}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Secondary Pulses:</span>
              <span className="stat-value">{stats.secondaryPulses}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Sync Status:</span>
              <span className={`stat-value ${stats.syncAcquired ? "sync-ok" : "sync-lost"}`} style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                {stats.syncAcquired ? <><Check size={14} /> Acquired</> : <><XCircle size={14} /> Not Acquired</>}
                {stats.syncLostCount > 0 && ` (Lost ${stats.syncLostCount}x)`}
              </span>
            </div>
          </>
        )}
      </div>

      <div className="composite-logger-canvas-container">
        {zoomLevel > 1 && (
          <input
            type="range"
            className="scroll-slider"
            min="0"
            max="1"
            step="0.01"
            value={scrollOffset}
            onChange={(e) => setScrollOffset(parseFloat(e.target.value))}
          />
        )}
        <canvas
          ref={canvasRef}
          width={900}
          height={400}
          className="composite-logger-canvas"
        />
      </div>

      {logData.length === 0 && !isCapturing && (
        <div className="composite-logger-empty">
          <p>No composite data captured yet.</p>
          <p>Click "Capture" to start recording trigger patterns from the ECU.</p>
        </div>
      )}

      <div className="composite-logger-legend">
        <div className="legend-item">
          <span className="legend-color" style={{ background: "#22c55e" }} />
          <span>Primary (Crank)</span>
        </div>
        <div className="legend-item">
          <span className="legend-color" style={{ background: "#3b82f6" }} />
          <span>Secondary (Cam)</span>
        </div>
        <div className="legend-item">
          <span className="legend-color" style={{ background: "#a855f7" }} />
          <span>Sync Status</span>
        </div>
        <div className="legend-item">
          <span className="legend-color" style={{ background: "#f59e0b" }} />
          <span>Voltage</span>
        </div>
      </div>
    </div>
  );
};

export default CompositeLoggerView;
