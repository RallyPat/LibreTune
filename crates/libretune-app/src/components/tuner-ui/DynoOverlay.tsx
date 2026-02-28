/**
 * Dyno Data Overlay Component
 *
 * Import dyno run CSV files, visualize power/torque curves,
 * compare before/after runs, and overlay data on VE tables.
 */
import { useState, useCallback, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './DynoOverlay.css';

interface DynoDataPoint {
  rpm: number;
  hp: number | null;
  torque: number | null;
  afr: number | null;
  boost: number | null;
  time: number | null;
}

interface DynoRun {
  name: string;
  data: DynoDataPoint[];
  peak_hp: [number, number] | null;
  peak_torque: [number, number] | null;
  color: string;
  source_file: string | null;
}

interface DynoComparison {
  run_a: DynoRun;
  run_b: DynoRun;
  hp_diff: [number, number][];
  torque_diff: [number, number][];
  total_hp_change: number | null;
  total_torque_change: number | null;
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

const RUN_COLORS = ['#4fc3f7', '#ff7043', '#66bb6a', '#ab47bc', '#ffa726', '#26c6da'];

export default function DynoOverlay({ isOpen, onClose }: Props) {
  const [runs, setRuns] = useState<DynoRun[]>([]);
  const [comparison, setComparison] = useState<DynoComparison | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showChannel, setShowChannel] = useState<'hp' | 'torque' | 'both'>('both');
  const [compareMode, setCompareMode] = useState(false);
  const [compareA, setCompareA] = useState(0);
  const [compareB, setCompareB] = useState(1);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const handleImport = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const path = await open({
        filters: [
          { name: 'CSV Files', extensions: ['csv', 'txt', 'tsv'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      if (!path || typeof path !== 'string') return;

      setLoading(true);
      setError(null);

      const name = `Run ${runs.length + 1}`;
      const run = await invoke<DynoRun>('load_dyno_run', { path, name });
      run.color = RUN_COLORS[runs.length % RUN_COLORS.length];
      setRuns(prev => [...prev, run]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [runs.length]);

  const handleRemoveRun = useCallback((idx: number) => {
    setRuns(prev => prev.filter((_, i) => i !== idx));
    setComparison(null);
  }, []);

  const handleCompare = useCallback(async () => {
    if (runs.length < 2) return;
    setLoading(true);
    try {
      const result = await invoke<DynoComparison>('compare_dyno_runs', {
        runA: runs[compareA],
        runB: runs[compareB],
      });
      setComparison(result);
      setCompareMode(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [runs, compareA, compareB]);

  // Draw power/torque charts
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || runs.length === 0) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const w = rect.width;
    const h = rect.height;
    const pad = { top: 30, right: 60, bottom: 40, left: 60 };
    const chartW = w - pad.left - pad.right;
    const chartH = h - pad.top - pad.bottom;

    // Clear
    ctx.fillStyle = 'rgba(20, 20, 30, 0.95)';
    ctx.fillRect(0, 0, w, h);

    // Find data ranges
    let minRpm = Infinity, maxRpm = -Infinity;
    let maxHp = 0, maxTq = 0;

    const activeRuns = compareMode && comparison ? [comparison.run_a, comparison.run_b] : runs;

    for (const run of activeRuns) {
      for (const pt of run.data) {
        minRpm = Math.min(minRpm, pt.rpm);
        maxRpm = Math.max(maxRpm, pt.rpm);
        if (pt.hp !== null) maxHp = Math.max(maxHp, pt.hp);
        if (pt.torque !== null) maxTq = Math.max(maxTq, pt.torque);
      }
    }

    if (minRpm >= maxRpm) return;

    // Round ranges
    minRpm = Math.floor(minRpm / 500) * 500;
    maxRpm = Math.ceil(maxRpm / 500) * 500;
    maxHp = Math.ceil(maxHp / 50) * 50;
    maxTq = Math.ceil(maxTq / 50) * 50;
    const maxVal = Math.max(maxHp, maxTq);

    const xScale = (rpm: number) => pad.left + ((rpm - minRpm) / (maxRpm - minRpm)) * chartW;
    const yScaleHp = (hp: number) => pad.top + chartH - (hp / maxVal) * chartH;
    const yScaleTq = (tq: number) => pad.top + chartH - (tq / maxVal) * chartH;

    // Grid
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.06)';
    ctx.lineWidth = 1;
    for (let rpm = minRpm; rpm <= maxRpm; rpm += 500) {
      const x = xScale(rpm);
      ctx.beginPath();
      ctx.moveTo(x, pad.top);
      ctx.lineTo(x, pad.top + chartH);
      ctx.stroke();
    }
    for (let v = 0; v <= maxVal; v += 50) {
      const y = yScaleHp(v);
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(pad.left + chartW, y);
      ctx.stroke();
    }

    // Axes labels
    ctx.fillStyle = '#888';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'center';
    for (let rpm = minRpm; rpm <= maxRpm; rpm += 1000) {
      ctx.fillText(`${rpm}`, xScale(rpm), pad.top + chartH + 16);
    }
    ctx.textAlign = 'right';
    for (let v = 0; v <= maxVal; v += 50) {
      ctx.fillText(`${v}`, pad.left - 6, yScaleHp(v) + 4);
    }

    // Axis titles
    ctx.fillStyle = '#aaa';
    ctx.font = '11px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('RPM', pad.left + chartW / 2, h - 4);

    ctx.save();
    ctx.translate(12, pad.top + chartH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText('HP / Torque', 0, 0);
    ctx.restore();

    // Draw runs
    for (const run of activeRuns) {
      const color = run.color || '#4fc3f7';

      // HP line
      if (showChannel !== 'torque') {
        ctx.strokeStyle = color;
        ctx.lineWidth = 2;
        ctx.beginPath();
        let started = false;
        for (const pt of run.data) {
          if (pt.hp === null) continue;
          const x = xScale(pt.rpm);
          const y = yScaleHp(pt.hp);
          if (!started) { ctx.moveTo(x, y); started = true; }
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
      }

      // Torque line (dashed)
      if (showChannel !== 'hp') {
        ctx.strokeStyle = color;
        ctx.lineWidth = 2;
        ctx.setLineDash([6, 4]);
        ctx.beginPath();
        let started = false;
        for (const pt of run.data) {
          if (pt.torque === null) continue;
          const x = xScale(pt.rpm);
          const y = yScaleTq(pt.torque);
          if (!started) { ctx.moveTo(x, y); started = true; }
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
        ctx.setLineDash([]);
      }
    }

    // Peak markers
    for (const run of activeRuns) {
      const color = run.color || '#4fc3f7';

      if (showChannel !== 'torque' && run.peak_hp) {
        const [hp, rpm] = run.peak_hp;
        const x = xScale(rpm);
        const y = yScaleHp(hp);
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.arc(x, y, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#fff';
        ctx.font = 'bold 10px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(`${hp.toFixed(0)} HP`, x, y - 10);
      }

      if (showChannel !== 'hp' && run.peak_torque) {
        const [tq, rpm] = run.peak_torque;
        const x = xScale(rpm);
        const y = yScaleTq(tq);
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.arc(x, y, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#fff';
        ctx.font = 'bold 10px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(`${tq.toFixed(0)} ft-lb`, x, y - 10);
      }
    }

    // Legend
    ctx.textAlign = 'left';
    let legendY = pad.top + 4;
    for (const run of activeRuns) {
      ctx.fillStyle = run.color || '#4fc3f7';
      ctx.fillRect(pad.left + 8, legendY, 12, 3);
      ctx.fillStyle = '#ccc';
      ctx.font = '11px sans-serif';
      ctx.fillText(run.name, pad.left + 24, legendY + 5);
      legendY += 16;
    }

    // Comparison diff overlay
    if (compareMode && comparison) {
      // Draw gain/loss area between curves
      if (comparison.hp_diff.length > 1 && showChannel !== 'torque') {
        ctx.globalAlpha = 0.15;
        for (let i = 0; i < comparison.hp_diff.length - 1; i++) {
          const [rpm1, diff1] = comparison.hp_diff[i];
          const [rpm2, diff2] = comparison.hp_diff[i + 1];
          const x1 = xScale(rpm1);
          const x2 = xScale(rpm2);

          ctx.fillStyle = diff1 >= 0 ? '#66bb6a' : '#ef5350';
          const baseY = yScaleHp(0);
          ctx.fillRect(x1, Math.min(baseY, yScaleHp(diff1)), x2 - x1, Math.abs(yScaleHp(diff1) - baseY));
          void diff2; // used for fill extent
        }
        ctx.globalAlpha = 1;
      }
    }

  }, [runs, showChannel, compareMode, comparison]);

  if (!isOpen) return null;

  return (
    <div className="dyno-overlay-panel">
      <div className="dyno-header">
        <h3>Dyno Data Overlay</h3>
        <div className="dyno-controls">
          <button className="dyno-import-btn" onClick={handleImport} disabled={loading}>
            {loading ? 'Loading...' : '+ Import CSV'}
          </button>
          <div className="dyno-channel-toggle">
            {(['hp', 'torque', 'both'] as const).map(ch => (
              <button key={ch} className={showChannel === ch ? 'active' : ''} onClick={() => setShowChannel(ch)}>
                {ch === 'hp' ? 'HP' : ch === 'torque' ? 'Torque' : 'Both'}
              </button>
            ))}
          </div>
          <button className="dyno-close-btn" onClick={onClose}>×</button>
        </div>
      </div>

      {error && <div className="dyno-error">{error}</div>}

      <div className="dyno-chart-area">
        <canvas ref={canvasRef} className="dyno-canvas" />
        {runs.length === 0 && (
          <div className="dyno-empty">
            Import a dyno CSV file to get started.<br />
            Supported columns: RPM, HP/Power, Torque, AFR, Boost
          </div>
        )}
      </div>

      {/* Run list */}
      {runs.length > 0 && (
        <div className="dyno-runs">
          {runs.map((run, i) => (
            <div key={i} className="dyno-run-card">
              <div className="dyno-run-color" style={{ background: run.color }} />
              <div className="dyno-run-info">
                <span className="dyno-run-name">{run.name}</span>
                <span className="dyno-run-stats">
                  {run.data.length} pts
                  {run.peak_hp && ` · ${run.peak_hp[0].toFixed(0)} HP @ ${run.peak_hp[1].toFixed(0)}`}
                  {run.peak_torque && ` · ${run.peak_torque[0].toFixed(0)} ft-lb @ ${run.peak_torque[1].toFixed(0)}`}
                </span>
              </div>
              <button className="dyno-run-remove" onClick={() => handleRemoveRun(i)}>×</button>
            </div>
          ))}

          {/* Compare controls */}
          {runs.length >= 2 && (
            <div className="dyno-compare">
              <select value={compareA} onChange={e => setCompareA(Number(e.target.value))}>
                {runs.map((r, i) => <option key={i} value={i}>{r.name} (Base)</option>)}
              </select>
              <span>vs</span>
              <select value={compareB} onChange={e => setCompareB(Number(e.target.value))}>
                {runs.map((r, i) => <option key={i} value={i}>{r.name}</option>)}
              </select>
              <button onClick={handleCompare} disabled={loading || compareA === compareB}>
                Compare
              </button>
            </div>
          )}

          {/* Comparison results */}
          {comparison && (
            <div className="dyno-comparison-results">
              <div className="dyno-comp-stat">
                <span className="label">Avg HP Change</span>
                <span className={`value ${(comparison.total_hp_change || 0) >= 0 ? 'gain' : 'loss'}`}>
                  {comparison.total_hp_change !== null
                    ? `${comparison.total_hp_change >= 0 ? '+' : ''}${comparison.total_hp_change.toFixed(1)} HP`
                    : 'N/A'}
                </span>
              </div>
              <div className="dyno-comp-stat">
                <span className="label">Avg Torque Change</span>
                <span className={`value ${(comparison.total_torque_change || 0) >= 0 ? 'gain' : 'loss'}`}>
                  {comparison.total_torque_change !== null
                    ? `${comparison.total_torque_change >= 0 ? '+' : ''}${comparison.total_torque_change.toFixed(1)} ft-lb`
                    : 'N/A'}
                </span>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
