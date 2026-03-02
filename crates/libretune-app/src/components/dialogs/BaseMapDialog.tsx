import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./BaseMapDialog.css";

interface BaseMapDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called when user applies — passes the generated map JSON for the caller to use */
  onApply: (baseMap: BaseMapResult) => void;
  /** Whether a project is already open (affects the "apply" button label) */
  hasProject: boolean;
}

export interface BaseMapResult {
  engine_spec: EngineSpecValues;
  rpm_bins: number[];
  load_bins: number[];
  ve_table: number[][];
  ignition_table: number[][];
  afr_table: number[][];
  cranking_enrichment: [number, number][];
  warmup_enrichment: [number, number][];
  accel_enrichment: { tps_threshold: number; enrichment_pct: number; duration_cycles: number; taper_pct: number };
  iac: { cold_start_pct: number; warm_idle_pct: number; warm_threshold_c: number };
  prime_pulse_ms: number;
  req_fuel: number;
  scalars: Record<string, number>;
}

interface EngineSpecValues {
  cylinder_count: number;
  displacement_cc: number;
  injector_size_cc: number;
  fuel_type: string;
  aspiration: string;
  stroke_type: string;
  injection_mode: string;
  ignition_mode: string;
  idle_rpm: number;
  redline_rpm: number;
  boost_target_kpa?: number;
  target_wot_afr?: number;
}

export default function BaseMapDialog({
  isOpen,
  onClose,
  onApply,
  hasProject,
}: BaseMapDialogProps) {
  const [cylinders, setCylinders] = useState(4);
  const [displacement, setDisplacement] = useState(2000);
  const [injectorSize, setInjectorSize] = useState(440);
  const [fuelType, setFuelType] = useState("Gasoline");
  const [aspiration, setAspiration] = useState("NA");
  const [strokeType, setStrokeType] = useState("four_stroke");
  const [injectionMode, setInjectionMode] = useState("Sequential");
  const [ignitionMode, setIgnitionMode] = useState("wasted_spark");
  const [idleRpm, setIdleRpm] = useState(800);
  const [redlineRpm, setRedlineRpm] = useState(6500);
  const [boostTarget, setBoostTarget] = useState(200);
  const [wotAfr, setWotAfr] = useState<string>("");

  const [generating, setGenerating] = useState(false);
  const [result, setResult] = useState<BaseMapResult | null>(null);
  const [error, setError] = useState("");
  const [previewTab, setPreviewTab] = useState<"ve" | "ign" | "afr" | "enrich">("ve");

  const isBoosted = aspiration === "Turbo" || aspiration === "Supercharged";

  async function handleGenerate() {
    setGenerating(true);
    setError("");
    setResult(null);
    try {
      const map = await invoke<BaseMapResult>("generate_base_map", {
        cylinderCount: cylinders,
        displacementCc: displacement,
        injectorSizeCc: injectorSize,
        fuelType,
        aspiration,
        strokeType,
        injectionMode,
        ignitionMode,
        idleRpm,
        redlineRpm,
        boostTargetKpa: isBoosted ? boostTarget : null,
        targetWotAfr: wotAfr ? parseFloat(wotAfr) : null,
      });
      setResult(map);
    } catch (e) {
      setError(`Generation failed: ${e}`);
    } finally {
      setGenerating(false);
    }
  }

  function handleApply() {
    if (result) {
      onApply(result);
    }
  }

  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="basemap-dialog" onClick={(e) => e.stopPropagation()}>
        <h2 className="dialog-title">Generate Base Map</h2>
        <p className="dialog-subtitle">
          Create a safe, driveable starting tune from your engine specifications.
        </p>

        {!result ? (
          /* Engine spec form */
          <div className="basemap-form">
            <div className="form-grid">
              <div className="form-field">
                <label>Cylinders</label>
                <select value={cylinders} onChange={(e) => setCylinders(parseInt(e.target.value))}>
                  {[1, 2, 3, 4, 5, 6, 8, 10, 12].map((n) => (
                    <option key={n} value={n}>{n}</option>
                  ))}
                </select>
              </div>

              <div className="form-field">
                <label>Displacement (cc)</label>
                <input
                  type="number"
                  value={displacement}
                  onChange={(e) => setDisplacement(parseFloat(e.target.value) || 0)}
                  min={50}
                  max={15000}
                />
              </div>

              <div className="form-field">
                <label>Injector Size (cc/min)</label>
                <input
                  type="number"
                  value={injectorSize}
                  onChange={(e) => setInjectorSize(parseFloat(e.target.value) || 0)}
                  min={50}
                  max={5000}
                />
              </div>

              <div className="form-field">
                <label>Fuel Type</label>
                <select value={fuelType} onChange={(e) => setFuelType(e.target.value)}>
                  <option value="Gasoline">Gasoline</option>
                  <option value="E85">E85</option>
                  <option value="E100">E100</option>
                  <option value="Methanol">Methanol</option>
                  <option value="LPG">LPG / Propane</option>
                </select>
              </div>

              <div className="form-field">
                <label>Aspiration</label>
                <select value={aspiration} onChange={(e) => setAspiration(e.target.value)}>
                  <option value="NA">Naturally Aspirated</option>
                  <option value="Turbo">Turbocharged</option>
                  <option value="Supercharged">Supercharged</option>
                </select>
              </div>

              <div className="form-field">
                <label>Stroke Type</label>
                <select value={strokeType} onChange={(e) => setStrokeType(e.target.value)}>
                  <option value="four_stroke">4-Stroke</option>
                  <option value="two_stroke">2-Stroke</option>
                </select>
              </div>

              <div className="form-field">
                <label>Injection Mode</label>
                <select value={injectionMode} onChange={(e) => setInjectionMode(e.target.value)}>
                  <option value="Sequential">Sequential</option>
                  <option value="Batch">Batch</option>
                  <option value="Simultaneous">Simultaneous</option>
                  <option value="throttle_body">Throttle Body</option>
                </select>
              </div>

              <div className="form-field">
                <label>Ignition Mode</label>
                <select value={ignitionMode} onChange={(e) => setIgnitionMode(e.target.value)}>
                  <option value="wasted_spark">Wasted Spark</option>
                  <option value="coil_on_plug">Coil on Plug</option>
                  <option value="distributor">Distributor</option>
                </select>
              </div>

              <div className="form-field">
                <label>Idle RPM</label>
                <input
                  type="number"
                  value={idleRpm}
                  onChange={(e) => setIdleRpm(parseInt(e.target.value) || 500)}
                  min={400}
                  max={2000}
                />
              </div>

              <div className="form-field">
                <label>Redline RPM</label>
                <input
                  type="number"
                  value={redlineRpm}
                  onChange={(e) => setRedlineRpm(parseInt(e.target.value) || 6000)}
                  min={2000}
                  max={20000}
                />
              </div>

              {isBoosted && (
                <div className="form-field">
                  <label>Boost Target (kPa absolute)</label>
                  <input
                    type="number"
                    value={boostTarget}
                    onChange={(e) => setBoostTarget(parseFloat(e.target.value) || 150)}
                    min={120}
                    max={400}
                  />
                  <span className="field-hint">
                    {((boostTarget - 101.325) / 100).toFixed(1)} bar / {((boostTarget - 101.325) * 0.145).toFixed(1)} psi gauge
                  </span>
                </div>
              )}

              <div className="form-field">
                <label>Target WOT AFR (optional)</label>
                <input
                  type="number"
                  value={wotAfr}
                  onChange={(e) => setWotAfr(e.target.value)}
                  placeholder="Auto (safe rich)"
                  step={0.1}
                />
              </div>
            </div>

            {error && <div className="error-msg">{error}</div>}

            <div className="dialog-actions">
              <button className="cancel-btn" onClick={onClose}>Cancel</button>
              <button
                className="generate-btn"
                onClick={handleGenerate}
                disabled={generating || displacement < 50 || injectorSize < 50}
              >
                {generating ? "Generating..." : "Generate Base Map"}
              </button>
            </div>
          </div>
        ) : (
          /* Preview */
          <div className="basemap-preview">
            <div className="preview-summary">
              <span>reqFuel: <b>{result.req_fuel.toFixed(1)} ms</b></span>
              <span>Prime: <b>{result.prime_pulse_ms.toFixed(1)} ms</b></span>
              <span>RPM range: <b>{result.rpm_bins[0]}–{result.rpm_bins[result.rpm_bins.length - 1]}</b></span>
            </div>

            <div className="preview-tabs">
              <button className={previewTab === "ve" ? "active" : ""} onClick={() => setPreviewTab("ve")}>VE Table</button>
              <button className={previewTab === "ign" ? "active" : ""} onClick={() => setPreviewTab("ign")}>Ignition</button>
              <button className={previewTab === "afr" ? "active" : ""} onClick={() => setPreviewTab("afr")}>AFR Target</button>
              <button className={previewTab === "enrich" ? "active" : ""} onClick={() => setPreviewTab("enrich")}>Enrichments</button>
            </div>

            <div className="preview-content">
              {previewTab === "ve" && <TablePreview table={result.ve_table} rpms={result.rpm_bins} loads={result.load_bins} unit="%" />}
              {previewTab === "ign" && <TablePreview table={result.ignition_table} rpms={result.rpm_bins} loads={result.load_bins} unit="°" />}
              {previewTab === "afr" && <TablePreview table={result.afr_table} rpms={result.rpm_bins} loads={result.load_bins} unit="" />}
              {previewTab === "enrich" && (
                <div className="enrich-preview">
                  <h4>Cranking Enrichment</h4>
                  <div className="enrich-curve">
                    {result.cranking_enrichment.map(([temp, pct], i) => (
                      <div key={i} className="curve-point">
                        <span>{temp}°C</span><span>{pct.toFixed(0)}%</span>
                      </div>
                    ))}
                  </div>
                  <h4>Warmup Enrichment</h4>
                  <div className="enrich-curve">
                    {result.warmup_enrichment.map(([temp, pct], i) => (
                      <div key={i} className="curve-point">
                        <span>{temp}°C</span><span>{pct.toFixed(0)}%</span>
                      </div>
                    ))}
                  </div>
                  <h4>Acceleration Enrichment</h4>
                  <div className="enrich-detail">
                    <span>TPS Threshold: {result.accel_enrichment.tps_threshold}%/sec</span>
                    <span>Enrichment: {result.accel_enrichment.enrichment_pct}%</span>
                    <span>Duration: {result.accel_enrichment.duration_cycles} cycles</span>
                  </div>
                  <h4>Idle Air Control</h4>
                  <div className="enrich-detail">
                    <span>Cold start: {result.iac.cold_start_pct}%</span>
                    <span>Warm idle: {result.iac.warm_idle_pct}%</span>
                    <span>Warm threshold: {result.iac.warm_threshold_c}°C</span>
                  </div>
                </div>
              )}
            </div>

            <div className="dialog-actions">
              <button className="cancel-btn" onClick={() => setResult(null)}>
                ← Edit Specs
              </button>
              <button className="cancel-btn" onClick={onClose}>
                Cancel
              </button>
              <button className="generate-btn" onClick={handleApply}>
                {hasProject ? "Apply to Current Tune" : "Use as Starting Tune"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

/** Mini table preview grid */
function TablePreview({
  table,
  rpms,
  loads,
  unit,
}: {
  table: number[][];
  rpms: number[];
  loads: number[];
  unit: string;
}) {
  // Show every other row/col if table is large
  const step = table.length > 12 ? 2 : 1;
  const rowIndices = Array.from({ length: Math.ceil(table.length / step) }, (_, i) => i * step);
  const colIndices = Array.from({ length: Math.ceil(rpms.length / step) }, (_, i) => i * step);

  return (
    <div className="table-preview-container">
      <table className="table-preview">
        <thead>
          <tr>
            <th className="corner">kPa\RPM</th>
            {colIndices.map((c) => (
              <th key={c}>{rpms[c]?.toFixed(0)}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {[...rowIndices].reverse().map((r) => (
            <tr key={r}>
              <th>{loads[r]?.toFixed(0)}</th>
              {colIndices.map((c) => {
                const val = table[r]?.[c] ?? 0;
                return (
                  <td key={c} title={`${val}${unit}`}>
                    {val.toFixed(val % 1 === 0 ? 0 : 1)}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
