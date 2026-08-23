/**
 * Tooth Logger Visualization Component
 * 
 * Displays captured tooth timing data from the ECU's crank/cam trigger wheel.
 * Shows tooth timing patterns to diagnose trigger wheel issues, missing teeth,
 * and timing irregularities.
 */

import React, { useState, useEffect, useRef, useCallback } from "react";
import { Square, Play, Download, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./ToothLoggerView.css";

interface ToothLogEntry {
  tooth_number: number;
  tooth_time_us: number;
  crank_angle?: number;
}

/** A record as `start_tooth_capture` emits it: INI field names to scaled values. */
/** Most recent records kept for display; a long capture exceeds any useful chart. */
const MAX_POINTS = 20000;

/**
 * Map INI-named fields onto the view's entries.
 *
 * Different ECUs name the same thing differently - Speeduino's tooth logger
 * declares `toothTime` in microseconds, while rusEFI's composite logger
 * declares `time` in milliseconds and expects the per-tooth interval to be
 * derived from consecutive values. Matching by name rather than position is
 * what lets one view serve both.
 */
function toEntries(records: LoggerRecord[]): ToothLogEntry[] {
  const out: ToothLogEntry[] = [];
  let prevTime: number | null = null;
  for (const r of records) {
    const direct = r.fields.toothTime;
    if (direct !== undefined) {
      out.push({ tooth_number: r.index, tooth_time_us: direct });
      continue;
    }
    // No direct interval: derive it from a running timestamp (ms -> us).
    const t = r.fields.refTime ?? r.fields.time;
    if (t === undefined) continue;
    if (prevTime !== null) {
      out.push({ tooth_number: r.index, tooth_time_us: (t - prevTime) * 1000 });
    }
    prevTime = t;
  }
  return out;
}

interface CaptureStatus {
  logger: string;
  records: number;
  reads: number;
  emptyReads: number;
  running: boolean;
  note: string | null;
}

interface LoggerRecord {
  index: number;
  fields: Record<string, number>;
}

interface ToothLoggerViewProps {
  onClose?: () => void;
}

export const ToothLoggerView: React.FC<ToothLoggerViewProps> = ({ onClose }) => {
  const [logData, setLogData] = useState<ToothLogEntry[]>([]);
  const [isCapturing, setIsCapturing] = useState(false);
  const [detectedRpm, setDetectedRpm] = useState<number | null>(null);
  const [teethPerRev, setTeethPerRev] = useState<number>(36);
  const [error, setError] = useState<string | null>(null);
  // Long enough to hold a gear and reach the rpm band under investigation.
  const [captureSeconds, setCaptureSeconds] = useState<number>(20);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Listen for real-time tooth data
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      // `tooth-log-records` streams batches while a capture runs. Records
      // carry the field names the INI declares (`toothTime` on Speeduino,
      // `time` on rusEFI), so the mapping is by name rather than by position.
      unlisten = await listen<LoggerRecord[]>("tooth-log-records", (event) => {
        const mapped = toEntries(event.payload);
        // APPEND. Batches arrive throughout the capture; replacing would show
        // only the last 256 records and hide everything that came before.
        setLogData((prev) => {
          const next = prev.concat(mapped);
          // Cap the view. A minute at 4500 rpm is on the order of a hundred
          // thousand teeth, which no canvas needs and no browser enjoys.
          return next.length > MAX_POINTS ? next.slice(-MAX_POINTS) : next;
        });
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // RPM from the teeth themselves. The old code took it from the single-shot
  // reply, so it was one number for one buffer; derived here it tracks the
  // whole capture, which is the point of capturing continuously.
  useEffect(() => {
    if (logData.length < teethPerRev) {
      setDetectedRpm(null);
      return;
    }
    const recent = logData.slice(-teethPerRev);
    const totalUs = recent.reduce((a, e) => a + e.tooth_time_us, 0);
    if (totalUs <= 0) {
      setDetectedRpm(null);
      return;
    }
    // One revolution is teethPerRev teeth; 60e6 us per minute.
    setDetectedRpm(60_000_000 / totalUs);
  }, [logData, teethPerRev]);

  // Draw tooth timing chart
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || logData.length === 0) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const padding = 40;

    // Clear canvas
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, width, height);

    // Find min/max tooth times for scaling
    const times = logData.map((t) => t.tooth_time_us);
    const minTime = Math.min(...times) * 0.9;
    const maxTime = Math.max(...times) * 1.1;
    const timeRange = maxTime - minTime;

    // Calculate average time for reference line
    const avgTime = times.reduce((a, b) => a + b, 0) / times.length;

    // Draw grid
    ctx.strokeStyle = "#333";
    ctx.lineWidth = 1;
    ctx.setLineDash([5, 5]);

    // Horizontal grid lines
    for (let i = 0; i <= 4; i++) {
      const y = padding + (height - 2 * padding) * (i / 4);
      ctx.beginPath();
      ctx.moveTo(padding, y);
      ctx.lineTo(width - padding, y);
      ctx.stroke();
    }

    // Vertical grid lines
    for (let i = 0; i <= 10; i++) {
      const x = padding + (width - 2 * padding) * (i / 10);
      ctx.beginPath();
      ctx.moveTo(x, padding);
      ctx.lineTo(x, height - padding);
      ctx.stroke();
    }

    ctx.setLineDash([]);

    // Draw average reference line
    const avgY = padding + (height - 2 * padding) * (1 - (avgTime - minTime) / timeRange);
    ctx.strokeStyle = "#666";
    ctx.lineWidth = 1;
    ctx.setLineDash([10, 5]);
    ctx.beginPath();
    ctx.moveTo(padding, avgY);
    ctx.lineTo(width - padding, avgY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Draw tooth timing bars
    const barWidth = (width - 2 * padding) / logData.length - 2;

    logData.forEach((tooth, i) => {
      const x = padding + (width - 2 * padding) * (i / logData.length);
      const normalizedTime = (tooth.tooth_time_us - minTime) / timeRange;
      const barHeight = normalizedTime * (height - 2 * padding);
      const y = height - padding - barHeight;

      // Color based on deviation from average
      const deviation = Math.abs(tooth.tooth_time_us - avgTime) / avgTime;
      let color: string;
      if (deviation < 0.05) {
        color = "#4ade80"; // Green - normal
      } else if (deviation < 0.15) {
        color = "#fbbf24"; // Yellow - warning
      } else {
        color = "#ef4444"; // Red - problem
      }

      ctx.fillStyle = color;
      ctx.fillRect(x, y, barWidth, barHeight);

      // Draw tooth number on hover area (would need event handling)
    });

    // Draw axes
    ctx.strokeStyle = "#888";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(padding, padding);
    ctx.lineTo(padding, height - padding);
    ctx.lineTo(width - padding, height - padding);
    ctx.stroke();

    // Y-axis labels (time in µs)
    ctx.fillStyle = "#aaa";
    ctx.font = "12px monospace";
    ctx.textAlign = "right";
    for (let i = 0; i <= 4; i++) {
      const y = padding + (height - 2 * padding) * (i / 4);
      const value = maxTime - (timeRange * i) / 4;
      ctx.fillText(`${value.toFixed(0)}µs`, padding - 5, y + 4);
    }

    // X-axis labels (tooth number)
    ctx.textAlign = "center";
    const labelStep = Math.ceil(logData.length / 10);
    for (let i = 0; i < logData.length; i += labelStep) {
      const x = padding + (width - 2 * padding) * (i / logData.length);
      ctx.fillText(`${i}`, x, height - padding + 15);
    }

    // Title
    ctx.font = "14px sans-serif";
    ctx.fillStyle = "#fff";
    ctx.textAlign = "center";
    ctx.fillText("Tooth Timing (µs per tooth)", width / 2, 20);

  }, [logData]);

  const handleCapture = useCallback(async () => {
    setIsCapturing(true);
    setError(null);
    setLogData([]);

    try {
      // Runs until stopped, or the limit. The ECU's buffer holds 127 records;
      // it refills between reads, so a capture is as long as you let it be -
      // the old single-read call mistook one bufferful for the hardware limit.
      const status = await invoke<CaptureStatus>("start_tooth_capture", {
        maxSeconds: captureSeconds,
      });
      if (status.note) setError(status.note);
      if (status.records === 0) {
        setError(
          status.note ??
            "No records captured. The logger needs the engine turning."
        );
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsCapturing(false);
    }
  }, [captureSeconds]);

  const handleStop = useCallback(async () => {
    try {
      await invoke("stop_tooth_capture");
    } catch (err) {
      console.error("Failed to stop tooth capture:", err);
    }
    setIsCapturing(false);
  }, []);

  const handleExport = useCallback(() => {
    if (logData.length === 0) return;

    // Create CSV content
    const lines = ["Tooth Number,Time (µs),Crank Angle (deg)"];
    logData.forEach((tooth) => {
      lines.push(`${tooth.tooth_number},${tooth.tooth_time_us},${tooth.crank_angle || ""}`);
    });

    // Download as file
    const blob = new Blob([lines.join("\n")], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "tooth_log.csv";
    a.click();
    URL.revokeObjectURL(url);
  }, [logData]);

  // Calculate statistics
  const stats = React.useMemo(() => {
    if (logData.length < 2) return null;

    const times = logData.map((t) => t.tooth_time_us);
    const avg = times.reduce((a, b) => a + b, 0) / times.length;
    const variance = times.reduce((sum, t) => sum + Math.pow(t - avg, 2), 0) / times.length;
    const stdDev = Math.sqrt(variance);
    const min = Math.min(...times);
    const max = Math.max(...times);

    // Detect missing tooth (should be ~2x average)
    const missingToothIndex = times.findIndex((t) => t > avg * 1.8);

    return {
      avg,
      stdDev,
      min,
      max,
      variability: (stdDev / avg) * 100,
      missingToothIndex,
    };
  }, [logData]);

  return (
    <div className="tooth-logger-view">
      <div className="tooth-logger-header">
        <h2>Tooth Logger</h2>
        <div className="tooth-logger-controls">
          <label className="tl-field" title="How long to keep reading. The ECU refills its buffer, so this is a real duration, not a buffer size.">
            for
            <input
              type="number"
              min={1}
              max={600}
              value={captureSeconds}
              disabled={isCapturing}
              onChange={(e) =>
                setCaptureSeconds(Math.max(1, parseInt(e.target.value, 10) || 1))
              }
            />
            s
          </label>
          <label className="tl-field" title="Teeth per crank revolution, used to derive rpm from the tooth intervals.">
            teeth/rev
            <input
              type="number"
              min={1}
              max={360}
              value={teethPerRev}
              onChange={(e) =>
                setTeethPerRev(Math.max(1, parseInt(e.target.value, 10) || 1))
              }
            />
          </label>
          <button
            className={`capture-btn ${isCapturing ? "capturing" : ""}`}
            onClick={isCapturing ? handleStop : handleCapture}
          >
            {isCapturing ? <><Square size={14} fill="currentColor" /> Stop</> : <><Play size={14} fill="currentColor" /> Capture</>}
          </button>
          <button
            className="export-btn"
            onClick={handleExport}
            disabled={logData.length === 0}
          >
            <Download size={14} /> Export CSV
          </button>
          {onClose && (
            <button className="close-btn" onClick={onClose} aria-label="Close">
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      {error && <div className="tooth-logger-error">{error}</div>}

      <div className="tooth-logger-stats">
        {detectedRpm !== null && (
          <div className="stat">
            <span className="stat-label">Detected RPM:</span>
            <span className="stat-value">{detectedRpm.toFixed(0)}</span>
          </div>
        )}
        <div className="stat">
          <span className="stat-label">Teeth/Rev:</span>
          <span className="stat-value">{teethPerRev}</span>
        </div>
        {stats && (
          <>
            <div className="stat">
              <span className="stat-label">Avg Time:</span>
              <span className="stat-value">{stats.avg.toFixed(0)} µs</span>
            </div>
            <div className="stat">
              <span className="stat-label">Variability:</span>
              <span className={`stat-value ${stats.variability > 5 ? "warning" : ""}`}>
                {stats.variability.toFixed(1)}%
              </span>
            </div>
            <div className="stat">
              <span className="stat-label">Min/Max:</span>
              <span className="stat-value">
                {stats.min.toFixed(0)} / {stats.max.toFixed(0)} µs
              </span>
            </div>
            {stats.missingToothIndex >= 0 && (
              <div className="stat missing-tooth">
                <span className="stat-label">Missing Tooth:</span>
                <span className="stat-value">Tooth #{stats.missingToothIndex}</span>
              </div>
            )}
          </>
        )}
      </div>

      <div className="tooth-logger-canvas-container">
        <canvas
          ref={canvasRef}
          width={800}
          height={400}
          className="tooth-logger-canvas"
        />
      </div>

      {logData.length === 0 && !isCapturing && (
        <div className="tooth-logger-empty">
          <p>No tooth data captured yet.</p>
          <p>Click "Capture" to start recording tooth timing from the ECU.</p>
        </div>
      )}
    </div>
  );
};

export default ToothLoggerView;
