/**
 * AFR Sensor Calibration dialog (TunerStudio's "Calibrate AFR Sensor").
 *
 * Builds the 1024-entry ADC→AFR transfer curve for Speeduino's O2
 * calibration space and writes it through the dedicated `t` calibration
 * command. Everything numeric happens on the backend: the preset list, the
 * curve and the auto-calibration all come from the loaded INI via
 * `list_calibration_presets` / `preview_afr_calibration`, so a different
 * firmware — or a MegaSquirt INI — brings its own presets instead of a list
 * hardcoded here.
 *
 * "Detect from sensor power-on" is the thing TunerStudio cannot do. A
 * 14Point7 Spartan 2 drives two known voltages for ~5 s each at power-on;
 * recording the AFR channel across a sensor power cycle and handing it to
 * `auto_calibrate_afr` recovers the wiring error (mostly ground offset) and
 * fills the two-point editor with the corrected calibration.
 */

import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog, Button } from "../common";
import { useRealtimeStore } from "../../stores/realtimeStore";
import "./AfrCalibrationDialog.css";

interface AfrCalibrationDialogProps {
  isOpen: boolean;
  onClose: () => void;
  connected: boolean;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;
}

/** One `solution` from the INI's O2 reference table. */
interface AfrPreset {
  label: string;
  expression: string | null;
  generator: string | null;
  usable: boolean;
  note: string | null;
}

interface CalibrationPresets {
  afr_label: string | null;
  afr_presets: AfrPreset[];
  /** `linearGenerator` bounds: (xLow, xHigh, yLow, yHigh) = volts/AFR. */
  linear_defaults: [number, number, number, number] | null;
}

interface AfrCurve {
  /** AFR at each of the 1024 ADC counts. */
  afr: number[];
  /** Sparse (volts, AFR) points for plotting. */
  preview: [number, number][];
  transfer_function: string;
  clips: boolean;
}

interface AutoCalResult {
  point1: [number, number];
  point2: [number, number];
  ground_offset_volts: number;
  offset_consistency_volts: number;
  description: string;
}

interface CalibrationWriteResult {
  table: string;
  verified: boolean;
  verify_note: string | null;
  transfer_function: string | null;
}

const CUSTOM = "custom";

/** Seconds of live AFR recorded for the sensor's power-on self-test. The two
 *  plateaus are ~5 s each; the rest is slack for reaching the key. */
const CAPTURE_SECONDS = 20;

/** Live channel carrying the wideband reading. */
const AFR_CHANNEL = "afr";

const STORAGE_KEY = "lt.afrCalibration";

interface StoredSettings {
  presetId: string;
  customV1: number;
  customAfr1: number;
  customV2: number;
  customAfr2: number;
  corrMode: "off" | "one" | "two";
  corrM1: number;
  corrE1: number;
  corrM2: number;
  corrE2: number;
}

function loadStored(): Partial<StoredSettings> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
  } catch {
    return {};
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export default function AfrCalibrationDialog({
  isOpen,
  onClose,
  connected,
  showToast,
}: AfrCalibrationDialogProps) {
  const stored = useMemo(loadStored, []);
  const [presets, setPresets] = useState<AfrPreset[]>([]);
  const [customLabel, setCustomLabel] = useState("Custom Linear WB");
  // Empty until the INI's presets arrive, so the first preview is the one the
  // user will actually see rather than a throwaway.
  const [presetId, setPresetId] = useState(stored.presetId ?? "");
  const [customV1, setCustomV1] = useState(stored.customV1 ?? 0);
  const [customAfr1, setCustomAfr1] = useState(stored.customAfr1 ?? 10);
  const [customV2, setCustomV2] = useState(stored.customV2 ?? 5);
  const [customAfr2, setCustomAfr2] = useState(stored.customAfr2 ?? 20);
  /** Reference correction over the selected base curve. "off" hides the
   *  section; one point corrects offset only; two solve gain and offset. */
  const [corrMode, setCorrMode] = useState<"off" | "one" | "two">(
    stored.corrMode ?? "off",
  );
  const [corrM1, setCorrM1] = useState(stored.corrM1 ?? 14.2);
  const [corrE1, setCorrE1] = useState(stored.corrE1 ?? 14.7);
  const [corrM2, setCorrM2] = useState(stored.corrM2 ?? 19.0);
  const [corrE2, setCorrE2] = useState(stored.corrE2 ?? 18.25);
  const [curve, setCurve] = useState<AfrCurve | null>(null);
  const [autoCal, setAutoCal] = useState<AutoCalResult | null>(null);
  const [captureLeft, setCaptureLeft] = useState(0);
  const [writing, setWriting] = useState(false);
  const [writeResult, setWriteResult] = useState<CalibrationWriteResult | null>(null);
  /** Command failures. Never swallowed: a dialog that reports success while
   *  doing nothing is this project's worst failure mode. Kept apart from
   *  `error` so a preview that succeeds cannot hide a preset list that did
   *  not load. */
  const [presetsError, setPresetsError] = useState<string | null>(null);
  /** Last preview / auto-cal / write failure. */
  const [error, setError] = useState<string | null>(null);

  const isCustom = presetId === CUSTOM;

  // The INI's own preset list. Presets that defer to an interactive editor
  // are not selectable as presets — the linearGenerator one *is* the
  // two-point editor below, and there is nothing else to point it at.
  const selectable = useMemo(() => presets.filter((p) => !p.generator), [presets]);

  useEffect(() => {
    if (!isOpen) return;
    let live = true;
    setWriteResult(null);
    invoke<CalibrationPresets>("list_calibration_presets")
      .then((p) => {
        if (!live) return;
        setPresets(p.afr_presets);
        setPresetsError(null);
        const gen = p.afr_presets.find((x) => x.generator === "linearGenerator");
        if (gen) setCustomLabel(gen.label);
        const s = loadStored();
        if (p.linear_defaults && s.customV1 === undefined) {
          const [xLow, xHigh, yLow, yHigh] = p.linear_defaults;
          setCustomV1(xLow);
          setCustomAfr1(yLow);
          setCustomV2(xHigh);
          setCustomAfr2(yHigh);
        }
        // A stored preset the current INI does not declare must not silently
        // stay selected — it would preview and write something else.
        setPresetId((cur) =>
          cur === CUSTOM || p.afr_presets.some((x) => x.label === cur && x.usable)
            ? cur
            : (p.afr_presets.find((x) => x.usable && !x.generator)?.label ?? CUSTOM),
        );
      })
      .catch((e) => {
        if (!live) return;
        setPresets([]);
        setPresetId(CUSTOM);
        setPresetsError(`Could not read the INI's calibration presets: ${e}`);
      });
    return () => {
      live = false;
    };
  }, [isOpen]);

  // Live preview. The curve is whatever the backend would actually write, so
  // what is plotted and what is sent can never drift apart.
  useEffect(() => {
    if (!isOpen || !presetId) return;
    let live = true;
    const correction =
      corrMode === "one"
        ? [[corrM1, corrE1]]
        : corrMode === "two"
          ? [
              [corrM1, corrE1],
              [corrM2, corrE2],
            ]
          : undefined;
    const args = isCustom
      ? { linear: [[customV1, customAfr1], [customV2, customAfr2]], correction }
      : { preset: presetId, correction };
    invoke<AfrCurve>("preview_afr_calibration", args)
      .then((c) => {
        if (!live) return;
        setCurve(c);
        setError(null);
      })
      .catch((e) => {
        if (!live) return;
        setCurve(null);
        setError(String(e));
      });
    return () => {
      live = false;
    };
  }, [isOpen, presetId, isCustom, customV1, customAfr1, customV2, customAfr2, corrMode, corrM1, corrE1, corrM2, corrE2]);

  const previewPath = useMemo(() => {
    if (!curve) return "";
    const w = 360;
    const h = 120;
    const afrMin = 5;
    const afrMax = 26;
    return curve.preview
      .map(([volts, afr]) => {
        const x = (volts / 5) * w;
        const y = h - ((afr - afrMin) / (afrMax - afrMin)) * h;
        return `${x.toFixed(1)},${Math.min(h, Math.max(0, y)).toFixed(1)}`;
      })
      .join(" ");
  }, [curve]);

  /** Record the live AFR channel across a sensor power cycle and solve the
   *  corrected two-point calibration from the startup plateaus. */
  async function handleDetect() {
    if (!curve) return;
    setError(null);
    setAutoCal(null);
    const t0 = Date.now();
    const samples: [number, number][] = [];
    // Subscribe on the update timestamp, not on the AFR value: during a
    // plateau the value is constant, and a value-keyed subscription would
    // record one sample for the whole window instead of a flat run.
    const unsubscribe = useRealtimeStore.subscribe(
      (s) => s.lastUpdateTime,
      () => {
        const afr = useRealtimeStore.getState().channels[AFR_CHANNEL];
        if (Number.isFinite(afr)) samples.push([(Date.now() - t0) / 1000, afr]);
      },
    );
    try {
      for (let left = CAPTURE_SECONDS; left > 0; left--) {
        setCaptureLeft(left);
        await sleep(1000);
      }
    } finally {
      unsubscribe();
      setCaptureLeft(0);
    }
    if (samples.length === 0) {
      setError(
        "No AFR samples arrived during the recording — the ECU was not " +
          "streaming, or the log has no 'afr' channel.",
      );
      return;
    }
    try {
      const result = await invoke<AutoCalResult>("auto_calibrate_afr", {
        samples,
        currentCurve: curve.afr,
      });
      setAutoCal(result);
      setPresetId(CUSTOM);
      setCustomV1(result.point1[0]);
      setCustomAfr1(result.point1[1]);
      setCustomV2(result.point2[0]);
      setCustomAfr2(result.point2[1]);
    } catch (e) {
      setError(`Auto-calibration failed: ${e}`);
    }
  }

  async function handleWrite() {
    if (!curve) return;
    setWriting(true);
    setError(null);
    setWriteResult(null);
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          presetId,
          customV1,
          customAfr1,
          customV2,
          customAfr2,
          corrMode,
          corrM1,
          corrE1,
          corrM2,
          corrE2,
        } satisfies StoredSettings),
      );
      const result = await invoke<CalibrationWriteResult>("write_afr_calibration", {
        afrValues: curve.afr,
        transferFunction: curve.transfer_function,
      });
      setWriteResult(result);
      showToast(
        result.verified
          ? "AFR sensor calibration written and verified"
          : "AFR sensor calibration written, but not verified",
        result.verified ? "success" : "warning",
      );
    } catch (e) {
      setError(`Failed to write AFR calibration: ${e}`);
      showToast("Failed to write AFR calibration: " + e, "error");
    } finally {
      setWriting(false);
    }
  }

  const fmt = (v: number) => v.toFixed(2);
  const capturing = captureLeft > 0;

  return (
    <Dialog open={isOpen} onClose={onClose} title="Calibrate AFR Sensor" size="md">
      <Dialog.Body className="afrcal-body">
        <p className="afrcal-subtitle">
          Maps the wideband controller's 0–5 V analog output to AFR for the
          ECU's O2 input. Written to the Speeduino calibration space
          (separate from the tune pages).
        </p>

        <div className="afrcal-field">
          <label htmlFor="afrcal-preset">Wideband controller</label>
          <select
            id="afrcal-preset"
            value={presetId}
            onChange={(e) => setPresetId(e.target.value)}
          >
            {selectable.map((p) => (
              <option
                key={p.label}
                value={p.label}
                disabled={!p.usable}
                title={p.note ?? undefined}
              >
                {p.usable ? p.label : `${p.label} (unavailable)`}
              </option>
            ))}
            <option value={CUSTOM}>{customLabel}…</option>
          </select>
        </div>

        {isCustom && (
          <div className="afrcal-custom-grid">
            <div className="afrcal-field">
              <label>Point 1 voltage (V)</label>
              <input
                type="number"
                step="0.01"
                value={customV1}
                onChange={(e) => setCustomV1(parseFloat(e.target.value) || 0)}
              />
            </div>
            <div className="afrcal-field">
              <label>Point 1 AFR</label>
              <input
                type="number"
                step="0.01"
                value={customAfr1}
                onChange={(e) => setCustomAfr1(parseFloat(e.target.value) || 0)}
              />
            </div>
            <div className="afrcal-field">
              <label>Point 2 voltage (V)</label>
              <input
                type="number"
                step="0.01"
                value={customV2}
                onChange={(e) => setCustomV2(parseFloat(e.target.value) || 0)}
              />
            </div>
            <div className="afrcal-field">
              <label>Point 2 AFR</label>
              <input
                type="number"
                step="0.01"
                value={customAfr2}
                onChange={(e) => setCustomAfr2(parseFloat(e.target.value) || 0)}
              />
            </div>
          </div>
        )}

        <div className="afrcal-field">
          <label htmlFor="afrcal-corr">Correct against a reference</label>
          <select
            id="afrcal-corr"
            value={corrMode}
            onChange={(e) => setCorrMode(e.target.value as "off" | "one" | "two")}
          >
            <option value="off">Off — use the curve as-is</option>
            <option value="one">Single point (offset)</option>
            <option value="two">Two point (gain + offset)</option>
          </select>
          {corrMode !== "off" && (
            <span className="afrcal-hint">
              Measured is what the ECU shows on this curve; expected is what a
              trusted reference reads at the same moment — the controller's own
              gauge, a second meter, or span gas. One point shifts the whole
              curve; two points also correct an error that grows across the
              range. The corrected curve is what previews and writes.
            </span>
          )}
        </div>

        {corrMode !== "off" && (
          <div className="afrcal-custom-grid">
            <div className="afrcal-field">
              <label htmlFor="afrcal-corrm1">{corrMode === "two" ? "Point 1 measured (ECU)" : "Measured (ECU)"}</label>
              <input
                id="afrcal-corrm1"
                type="number"
                step="0.01"
                value={corrM1}
                onChange={(e) => setCorrM1(parseFloat(e.target.value) || 0)}
              />
            </div>
            <div className="afrcal-field">
              <label htmlFor="afrcal-corre1">{corrMode === "two" ? "Point 1 expected (reference)" : "Expected (reference)"}</label>
              <input
                id="afrcal-corre1"
                type="number"
                step="0.01"
                value={corrE1}
                onChange={(e) => setCorrE1(parseFloat(e.target.value) || 0)}
              />
            </div>
            {corrMode === "two" && (
              <>
                <div className="afrcal-field">
                  <label htmlFor="afrcal-corrm2">Point 2 measured (ECU)</label>
                  <input
                    id="afrcal-corrm2"
                    type="number"
                    step="0.01"
                    value={corrM2}
                    onChange={(e) => setCorrM2(parseFloat(e.target.value) || 0)}
                  />
                </div>
                <div className="afrcal-field">
                  <label htmlFor="afrcal-corre2">Point 2 expected (reference)</label>
                  <input
                    id="afrcal-corre2"
                    type="number"
                    step="0.01"
                    value={corrE2}
                    onChange={(e) => setCorrE2(parseFloat(e.target.value) || 0)}
                  />
                </div>
              </>
            )}
          </div>
        )}

        <div className="afrcal-field">
          <Button
            variant="secondary"
            onClick={handleDetect}
            disabled={!connected || !curve || capturing || writing}
          >
            {capturing
              ? `Recording… ${captureLeft}s — power-cycle the sensor now`
              : "Detect from sensor power-on"}
          </Button>
          <span className="afrcal-hint">
            Records {CAPTURE_SECONDS}s of the live AFR channel. Start it, then
            cycle power to the wideband: its two startup plateaus are
            sensor-generated, so the difference between what it sent and what
            the ECU read is the wiring error. Needs the ECU streaming.
          </span>
        </div>

        {autoCal && (
          <p className="afrcal-result">
            {autoCal.description}
            {Math.abs(autoCal.offset_consistency_volts) > 0.05 &&
              " — the two plateaus disagree, so this is not a simple ground offset; check the fit before writing."}
          </p>
        )}

        <div className="afrcal-preview">
          <svg viewBox="0 0 360 120" preserveAspectRatio="none" aria-hidden>
            <polyline points={previewPath} fill="none" strokeWidth="2" className="afrcal-line" />
          </svg>
          <div className="afrcal-endpoints">
            {curve ? (
              <>
                <span>0 V → {fmt(curve.afr[0])}</span>
                <span>2.5 V → {fmt(curve.afr[512])}</span>
                <span>5 V → {fmt(curve.afr[1023])}</span>
              </>
            ) : (
              <span>No curve.</span>
            )}
          </div>
        </div>

        {curve?.clips && (
          <p className="afrcal-error">
            This curve leaves the 0.0–25.5 AFR the wire format can carry; the
            ends will be flat-topped in the ECU.
          </p>
        )}

        <p className="afrcal-warning">
          Write with the engine off. On the legacy serial protocol the ECU
          sends no acknowledgement, so re-check a logged AFR value afterwards;
          on the CRC protocol the write is verified automatically.
        </p>
        {!connected && <p className="afrcal-error">Not connected to an ECU.</p>}
        {presetsError && <p className="afrcal-error">{presetsError}</p>}
        {error && <p className="afrcal-error">{error}</p>}
        {writeResult && (
          <p className={writeResult.verified ? "afrcal-result" : "afrcal-error"}>
            {writeResult.verified
              ? `Written and verified by read-back${writeResult.transfer_function ? `: ${writeResult.transfer_function}` : ""}.`
              : `Written, but NOT verified. ${writeResult.verify_note ?? ""}`}
          </p>
        )}
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose}>
          {writeResult ? "Close" : "Cancel"}
        </Button>
        <Button
          variant="primary"
          onClick={handleWrite}
          disabled={!connected || writing || capturing || !curve}
        >
          {writing ? "Writing…" : "Write Calibration"}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
