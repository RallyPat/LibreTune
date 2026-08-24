/**
 * Log Analyze — tune a VE table from a recorded drive, offline.
 *
 * A live AutoTune session gives you one pass over whatever samples the drive
 * happened to produce, and no way to ask what a different setting would have
 * done. Here the log is fixed, so a configuration can be changed and re-run
 * against exactly the same samples, and every difference in the result is
 * attributable to the configuration rather than to the driving.
 *
 * Three things this shows that a general log viewer cannot, because they need
 * the tuning filters to answer:
 *
 * - **Why each sample was refused.** The timeline is shaded by verdict, so a
 *   session that collected nothing shows the filter that ate it instead of
 *   looking broken.
 * - **Where the tune actually has evidence.** Coverage counts accepted samples
 *   per cell, which is not the same as where the car went.
 * - **Whether the proposal is any good.** The result is scored against samples
 *   it never trained on, so a configuration that chased noise reports a loss
 *   rather than a confident large change.
 */
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { FolderOpen, Play, Check, AlertTriangle, Info } from 'lucide-react';
import { parseLogFile, type LogSample } from '../../utils/parseLogFile';
import GraphLog, { type GraphSample } from './GraphLog';
import './LogAnalyze.css';

/** Channel name candidates, first match wins. Speeduino/MS naming varies. */
const CHANNEL_ALIASES: Record<string, string[]> = {
  rpm: ['rpm', 'RPM'],
  load: ['map', 'MAP', 'fuelLoad', 'Load'],
  afr: ['afr', 'AFR', 'AFR1', 'O2', 'o2', 'lambda1'],
  ve: ['veCurr', 'VE1', 've', 'VE'],
  clt: ['coolant', 'CLT', 'clt', 'Coolant'],
  tps: ['tps', 'TPS'],
  tps_rate: ['TPSdot', 'tpsDOT', 'TPS DOT'],
  fuel_cut: ['DFCOOn', 'dfco', 'DFCO'],
  accel_enrich: ['tpsaccaen', 'AEamount', 'accelEnrich'],
};

interface CellResult {
  x: number; y: number; rpm: number; load: number;
  current_ve: number; proposed_ve: number; delta: number;
  hits: number; weight: number; confidence: number;
  target_afr: number; mean_afr: number;
}
interface SampleVerdict { rejected_because: string | null; cell: [number, number] | null }
interface ValidationScore { gain_pct: number; worsened_pct: number; scored: number; folds: number }
interface ReplayReport {
  cells: CellResult[];
  total_samples: number;
  rejections: [string, number][];
  verdicts: SampleVerdict[];
  validation: ValidationScore | null;
  coverage: number[][];
}
interface TableData {
  name: string; title: string;
  x_bins: number[]; y_bins: number[]; z_values: number[][];
}

/** Weighting choices, described by what they do rather than by tool name. */
const WEIGHTINGS = [
  { v: 'uniform', label: 'None — every sample counts fully for its nearest cell' },
  { v: 'cell_proximity', label: 'Soft — a sample is shared with the cell it sits nearest to' },
  { v: 'cell_proximity_squared', label: 'Medium — sharing falls away faster, so cells stay distinct' },
  { v: 'cell_centre_only', label: 'Hard — only samples near a cell centre count at all' },
];

export interface LogAnalyzeProps {
  tableName?: string;
  isConnected: boolean;
}

export const LogAnalyze: React.FC<LogAnalyzeProps> = ({ tableName, isConnected }) => {
  const [samples, setSamples] = useState<LogSample[]>([]);
  const [logName, setLogName] = useState<string>('');
  const [channels, setChannels] = useState<string[]>([]);
  const [tables, setTables] = useState<string[]>([]);
  const [table, setTable] = useState(tableName || '');
  const [tableData, setTableData] = useState<TableData | null>(null);
  const [report, setReport] = useState<ReplayReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<[number, number] | null>(null);
  const [view, setView] = useState<'analyse' | 'traces'>('analyse');

  // Config
  const [weighting, setWeighting] = useState('cell_proximity');
  const [baseWeight, setBaseWeight] = useState(20);
  const [minChange, setMinChange] = useState(1);
  const [minSteadyMs, setMinSteadyMs] = useState(800);
  const [minClt, setMinClt] = useState(71);
  const [maxTpsRate, setMaxTpsRate] = useState(50);
  const [delayMs, setDelayMs] = useState(0);

  useEffect(() => {
    invoke<string[]>('list_tunable_tables')
      .then((t) => {
        setTables(t);
        if (!table) {
          setTable(t.find((n) => /ve/i.test(n)) || t[0] || '');
        }
      })
      .catch(() => setTables([]));
    // Only on mount: re-running would fight the user's own choice of table.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!table) return;
    invoke<TableData>('get_table_data', { tableName: table })
      .then(setTableData)
      .catch((e) => setError(String(e)));
  }, [table]);

  const loadLog = useCallback(async () => {
    setError(null);
    try {
      const path = await open({
        multiple: false,
        filters: [
          { name: 'Data logs', extensions: ['csv', 'msl', 'txt'] },
          { name: 'All files', extensions: ['*'] },
        ],
      });
      if (!path || typeof path !== 'string') return;
      const text = await invoke<string>('read_file_contents', { path });
      const parsed = parseLogFile(text);
      if (!parsed.data.length) {
        setError('No samples could be read from that file.');
        return;
      }
      setSamples(parsed.data);
      setChannels(parsed.channels);
      setLogName(path.split(/[\\/]/).pop() || path);
      setReport(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /** Which log column is standing in for each channel we need. */
  const mapping = useMemo(() => {
    const m: Record<string, string | null> = {};
    for (const [key, names] of Object.entries(CHANNEL_ALIASES)) {
      m[key] = names.find((n) => channels.includes(n)) ?? null;
    }
    return m;
  }, [channels]);

  const missing = useMemo(
    () => ['rpm', 'load', 'afr'].filter((k) => !mapping[k]),
    [mapping],
  );

  const run = useCallback(async () => {
    if (!samples.length || !table) return;
    setBusy(true);
    setError(null);
    try {
      const col = (key: string): number[] => {
        const name = mapping[key];
        if (!name) return [];
        return samples.map((s) => s.values[name] ?? NaN);
      };
      // Timestamps are re-based: an .msl exported from a longer recording
      // carries offsets from the original, which would put every sample in one
      // validation block.
      const t0 = Math.min(...samples.map((s) => s.x));
      const report = await invoke<ReplayReport>('analyse_log', {
        tableName: table,
        log: {
          time_ms: samples.map((s) => s.x - t0),
          rpm: col('rpm'), load: col('load'), afr: col('afr'),
          ve: col('ve'), clt: col('clt'), tps: col('tps'),
          tps_rate: col('tps_rate'),
          fuel_cut: col('fuel_cut'), accel_enrich: col('accel_enrich'),
        },
        config: {
          settings: {
            target_afr: 14.7,
            algorithm: 'simple',
            update_rate_ms: 100,
            lambda_delay_ms: delayMs,
            lambda_delay_flow_scaled: false,
            lambda_delay_floor_ms: 120,
            hit_weighting: weighting,
            base_weight: baseWeight,
            min_change: minChange,
          },
          filters: {
            min_rpm: 1000, max_rpm: 8000,
            min_y_axis: null, max_y_axis: null,
            min_clt: minClt, custom_filter: null,
            max_tps_rate: maxTpsRate,
            exclude_accel_enrich: true,
            min_steady_ms: minSteadyMs,
          },
          authority: {
            max_cell_value_change: 10,
            max_cell_percentage_change: 20,
            min_cell_value: 0,
            max_cell_value: 255,
          },
          strict_lambda_match: true,
          validate: true,
        },
        targetAfrTableName: null,
        lambdaDelayTableName: null,
      });
      setReport(report);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [samples, table, mapping, weighting, baseWeight, minChange, minSteadyMs, minClt, maxTpsRate, delayMs]);

  const apply = useCallback(async () => {
    if (!report || !tableData) return;
    const z = tableData.z_values.map((r) => [...r]);
    for (const c of report.cells) {
      if (c.delta !== 0 && z[c.y]?.[c.x] !== undefined) z[c.y][c.x] = c.proposed_ve;
    }
    setBusy(true);
    try {
      await invoke('update_table_data', { tableName: table, zValues: z });
      const fresh = await invoke<TableData>('get_table_data', { tableName: table });
      setTableData(fresh);
      setReport(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [report, tableData, table]);

  const byCell = useMemo(() => {
    const m = new Map<string, CellResult>();
    report?.cells.forEach((c) => m.set(`${c.x},${c.y}`, c));
    return m;
  }, [report]);

  // GraphLog names the timestamp `t`; the parser calls it `x`. Same number.
  const graphSamples: GraphSample[] = useMemo(
    () => samples.map((s) => ({ t: s.x, values: s.values })),
    [samples],
  );

  const changed = report?.cells.filter((c) => c.delta !== 0).length ?? 0;

  return (
    <div className="log-analyze">
      <div className="la-bar">
        <button className="la-btn" onClick={loadLog}>
          <FolderOpen size={15} /> {logName || 'Open log…'}
        </button>
        <select className="la-select" value={table} onChange={(e) => setTable(e.target.value)}>
          {tables.length === 0 && <option value="">no tables</option>}
          {tables.map((t) => <option key={t} value={t}>{t}</option>)}
        </select>
        <button
          className="la-btn la-primary"
          disabled={!samples.length || !table || busy || missing.length > 0}
          onClick={run}
        >
          <Play size={15} /> {busy ? 'Working…' : 'Analyse'}
        </button>
        <button className="la-btn" disabled={!report || changed === 0 || busy} onClick={apply}>
          <Check size={15} /> Apply {changed > 0 ? `${changed} cells` : ''}
        </button>
        {samples.length > 0 && (
          <span className="la-meta">{samples.length.toLocaleString()} samples</span>
        )}
        {!isConnected && <span className="la-meta la-warn">offline — reading the project tune</span>}
      </div>

      {error && <div className="la-error"><AlertTriangle size={14} /> {error}</div>}
      {missing.length > 0 && samples.length > 0 && (
        <div className="la-error">
          <AlertTriangle size={14} /> This log has no {missing.join(', ')} channel — nothing can be
          attributed to a cell without it.
        </div>
      )}

      <div className="la-subtabs" role="tablist">
        <button
          role="tab"
          aria-selected={view === 'analyse'}
          className={view === 'analyse' ? 'on' : ''}
          onClick={() => setView('analyse')}
        >
          Analyse
        </button>
        <button
          role="tab"
          aria-selected={view === 'traces'}
          className={view === 'traces' ? 'on' : ''}
          onClick={() => setView('traces')}
          disabled={!samples.length}
        >
          Traces
        </button>
      </div>

      {view === 'traces' ? (
        samples.length ? (
          // The strip charts the Data Logging tab already uses: assignable
          // channels per pane, Q/A or the buttons to zoom, arrow keys to step
          // the cursor. Reused rather than rebuilt so both places behave the
          // same and a pane layout set in one is the layout in the other.
          <div className="la-traces">
            <GraphLog samples={graphSamples} availableChannels={channels} />
          </div>
        ) : (
          <p className="la-note la-pad">Open a log to plot its channels.</p>
        )
      ) : (
      <div className="la-body">
        <aside className="la-config">
          <h4>Sample weighting</h4>
          <label>
            How much a sample counts for its cell
            <select value={weighting} onChange={(e) => setWeighting(e.target.value)}>
              {WEIGHTINGS.map((w) => <option key={w.v} value={w.v}>{w.label}</option>)}
            </select>
          </label>
          <Num label="Confidence weight" hint="Accumulated weight for a cell to propose its full change. 0 disables the ramp." value={baseWeight} set={setBaseWeight} />
          <Num label="Smallest change (VE)" hint="Below this a cell is left alone rather than twitching on noise." value={minChange} set={setMinChange} step={0.5} />

          <h4>Which samples count</h4>
          <Num label="Steady for (ms)" hint="rpm and load must hold still this long first. 0 disables. Catches load changes the throttle filters cannot see." value={minSteadyMs} set={setMinSteadyMs} step={100} />
          <Num label="Minimum coolant" hint="Match your project's unit system — 71 C, or 160 F." value={minClt} set={setMinClt} />
          <Num label="Max throttle rate (%/s)" value={maxTpsRate} set={setMaxTpsRate} step={5} />
          <Num label="Exhaust delay (ms)" hint="0 uses the per-cell table if the INI has one, otherwise a curve from rpm." value={delayMs} set={setDelayMs} step={10} />

          {report?.validation && <Validation v={report.validation} />}
          {report && report.rejections.length > 0 && (
            <>
              <h4>Why samples were dropped</h4>
              <table className="la-rej">
                <tbody>
                  {report.rejections.slice(0, 8).map(([r, n]) => (
                    <tr key={r}><td>{r}</td><td>{n.toLocaleString()}</td></tr>
                  ))}
                </tbody>
              </table>
              <p className="la-note">
                {report.total_samples.toLocaleString()} samples counted.
              </p>
            </>
          )}
        </aside>

        <main className="la-main">
          {tableData ? (
            <Grid
              table={tableData}
              byCell={byCell}
              coverage={report?.coverage}
              selected={selected}
              onSelect={setSelected}
            />
          ) : (
            <p className="la-note">Pick a table to analyse.</p>
          )}
          {report && (
            <Timeline
              samples={samples}
              verdicts={report.verdicts}
              selected={selected}
            />
          )}
          {selected && byCell.get(`${selected[0]},${selected[1]}`) && (
            <CellDetail c={byCell.get(`${selected[0]},${selected[1]}`)!} />
          )}
        </main>
      </div>
      )}
    </div>
  );
};

const Num: React.FC<{
  label: string; hint?: string; value: number; set: (n: number) => void; step?: number;
}> = ({ label, hint, value, set, step = 1 }) => (
  <label title={hint}>
    {label}
    <input
      type="number"
      step={step}
      value={value}
      onChange={(e) => set(Number(e.target.value))}
    />
  </label>
);

/**
 * The held-out score, stated in words as well as numbers.
 *
 * A large change is not the same as a good one, and this is the only figure on
 * the page that can tell them apart.
 */
const Validation: React.FC<{ v: ValidationScore }> = ({ v }) => {
  const good = v.gain_pct > 0;
  return (
    <div className={`la-score ${good ? 'ok' : 'bad'}`}>
      <h4><Info size={13} /> Checked against unseen samples</h4>
      <div className="la-score-big">{v.gain_pct > 0 ? '+' : ''}{v.gain_pct.toFixed(1)}%</div>
      <p>
        {good
          ? `closer to target AFR on ${v.scored.toLocaleString()} samples this proposal never trained on.`
          : `further from target on ${v.scored.toLocaleString()} unseen samples — this configuration is fitting noise, not the tune.`}
      </p>
      <p className="la-note">{v.worsened_pct.toFixed(0)}% of them get worse, over {v.folds} folds.</p>
    </div>
  );
};

/** Colour for a VE change: red removes fuel, blue adds it. */
function deltaColour(d: number, max: number): string {
  if (d === 0) return 'transparent';
  const f = Math.min(1, Math.abs(d) / (max || 1));
  return d > 0
    ? `rgba(64, 140, 255, ${0.15 + f * 0.65})`
    : `rgba(255, 86, 64, ${0.15 + f * 0.65})`;
}

const Grid: React.FC<{
  table: TableData;
  byCell: Map<string, CellResult>;
  coverage?: number[][];
  selected: [number, number] | null;
  onSelect: (c: [number, number] | null) => void;
}> = ({ table, byCell, coverage, selected, onSelect }) => {
  const [mode, setMode] = useState<'delta' | 'coverage'>('delta');
  const maxDelta = useMemo(
    () => Math.max(0.1, ...[...byCell.values()].map((c) => Math.abs(c.delta))),
    [byCell],
  );
  const maxCov = useMemo(
    () => Math.max(1, ...(coverage ?? []).flat()),
    [coverage],
  );

  return (
    <div className="la-grid-wrap">
      <div className="la-grid-head">
        <strong>{table.title || table.name}</strong>
        <div className="la-toggle">
          <button className={mode === 'delta' ? 'on' : ''} onClick={() => setMode('delta')}>Change</button>
          <button className={mode === 'coverage' ? 'on' : ''} onClick={() => setMode('coverage')}>Coverage</button>
        </div>
        <span className="la-note">
          {mode === 'delta'
            ? 'blue adds fuel, red removes it'
            : 'accepted samples per cell — where the tune has evidence'}
        </span>
      </div>
      <div className="la-grid-scroll">
        <table className="la-grid">
          <tbody>
            {/* Highest load at the top, the way a tuner reads a fuel map. */}
            {table.y_bins.map((_, yi) => {
              const y = table.y_bins.length - 1 - yi;
              return (
                <tr key={y}>
                  <th>{table.y_bins[y]}</th>
                  {table.x_bins.map((_, x) => {
                    const c = byCell.get(`${x},${y}`);
                    const cov = coverage?.[y]?.[x] ?? 0;
                    const sel = selected?.[0] === x && selected?.[1] === y;
                    const bg = mode === 'delta'
                      ? deltaColour(c?.delta ?? 0, maxDelta)
                      : cov > 0 ? `rgba(120, 220, 140, ${0.12 + (cov / maxCov) * 0.7})` : 'transparent';
                    return (
                      <td
                        key={x}
                        className={sel ? 'sel' : ''}
                        style={{ background: bg }}
                        title={c ? `${c.hits} samples, confidence ${(c.confidence * 100).toFixed(0)}%` : 'no data'}
                        onClick={() => onSelect(sel ? null : [x, y])}
                      >
                        {mode === 'delta'
                          ? (c && c.delta !== 0
                            ? <>{c.proposed_ve.toFixed(0)}<sub>{c.delta > 0 ? '+' : ''}{c.delta.toFixed(1)}</sub></>
                            : table.z_values[y]?.[x]?.toFixed(0))
                          : (cov || '')}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
            <tr className="la-xaxis">
              <th />
              {table.x_bins.map((rpm) => <th key={rpm}>{rpm}</th>)}
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
};

/** Colours for the verdict strip. Accepted is the only bright one. */
const VERDICT_COLOURS: Record<string, string> = {
  accepted: '#3ddc84',
  'rpm/load not steady': '#8a6fd0',
  'overrun fuel cut': '#4a6b8a',
  'clt below min_clt': '#c98a3a',
  'rpm out of range': '#7a7a7a',
  'afr at sensor rail': '#c04a6a',
  'tps_rate above max_tps_rate': '#b5643c',
  'accel enrichment active': '#9a7b4f',
};

/**
 * Every sample in the log, shaded by whether it counted and why not.
 *
 * The strip is drawn to a canvas one pixel column at a time: a log of tens of
 * thousands of samples has more points than the strip has pixels, so each
 * column shows the verdict that dominates it rather than whichever sample
 * happened to land last.
 */
const Timeline: React.FC<{
  samples: LogSample[];
  verdicts: SampleVerdict[];
  selected: [number, number] | null;
}> = ({ samples, verdicts, selected }) => {
  const ref = useRef<HTMLCanvasElement>(null);
  const [hover, setHover] = useState<string | null>(null);

  useEffect(() => {
    const cv = ref.current;
    if (!cv) return;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    const w = cv.width, h = cv.height;
    ctx.clearRect(0, 0, w, h);
    const n = Math.min(samples.length, verdicts.length);
    if (!n) return;

    for (let px = 0; px < w; px++) {
      const from = Math.floor((px / w) * n);
      const to = Math.max(from + 1, Math.floor(((px + 1) / w) * n));
      const tally: Record<string, number> = {};
      let inCell = false;
      for (let i = from; i < to && i < n; i++) {
        const v = verdicts[i];
        const key = v.rejected_because ?? 'accepted';
        tally[key] = (tally[key] ?? 0) + 1;
        if (selected && v.cell && v.cell[0] === selected[0] && v.cell[1] === selected[1]) {
          inCell = true;
        }
      }
      const top = Object.entries(tally).sort((a, b) => b[1] - a[1])[0];
      if (!top) continue;
      ctx.fillStyle = VERDICT_COLOURS[top[0]] ?? '#555';
      ctx.fillRect(px, 0, 1, h);
      if (inCell) {
        ctx.fillStyle = '#ffd54a';
        ctx.fillRect(px, h - 4, 1, 4);
      }
    }
  }, [samples, verdicts, selected]);

  const legend = useMemo(() => {
    const tally: Record<string, number> = {};
    verdicts.forEach((v) => {
      const k = v.rejected_because ?? 'accepted';
      tally[k] = (tally[k] ?? 0) + 1;
    });
    return Object.entries(tally).sort((a, b) => b[1] - a[1]);
  }, [verdicts]);

  return (
    <div className="la-timeline">
      <div className="la-grid-head">
        <strong>Every sample, and whether it counted</strong>
        <span className="la-note">
          {selected ? 'yellow ticks mark the selected cell' : 'click a cell above to locate its samples'}
        </span>
      </div>
      <canvas
        ref={ref}
        width={1200}
        height={34}
        onMouseMove={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          const f = (e.clientX - r.left) / r.width;
          const i = Math.min(verdicts.length - 1, Math.max(0, Math.floor(f * verdicts.length)));
          setHover(verdicts[i]?.rejected_because ?? 'counted');
        }}
        onMouseLeave={() => setHover(null)}
      />
      <div className="la-legend">
        {legend.map(([k, n]) => (
          <span key={k} className={hover === k ? 'on' : ''}>
            <i style={{ background: VERDICT_COLOURS[k] ?? '#555' }} />
            {k === 'accepted' ? 'counted' : k} ({n.toLocaleString()})
          </span>
        ))}
      </div>
    </div>
  );
};

const CellDetail: React.FC<{ c: CellResult }> = ({ c }) => (
  <div className="la-detail">
    <strong>{c.rpm} rpm, load {c.load}</strong>
    <dl>
      <div><dt>VE now</dt><dd>{c.current_ve.toFixed(1)}</dd></div>
      <div><dt>Proposed</dt><dd>{c.proposed_ve.toFixed(1)}</dd></div>
      <div><dt>Change</dt><dd>{c.delta > 0 ? '+' : ''}{c.delta.toFixed(1)}</dd></div>
      <div><dt>Samples</dt><dd>{c.hits}</dd></div>
      <div><dt>Confidence</dt><dd>{(c.confidence * 100).toFixed(0)}%</dd></div>
      <div><dt>Measured AFR</dt><dd>{c.mean_afr.toFixed(2)}</dd></div>
      <div><dt>Target AFR</dt><dd>{c.target_afr.toFixed(2)}</dd></div>
    </dl>
    {c.confidence < 1 && (
      <p className="la-note">
        Below full confidence, so the change is scaled down in proportion — this cell
        has seen {c.hits} samples.
      </p>
    )}
  </div>
);

export default LogAnalyze;
