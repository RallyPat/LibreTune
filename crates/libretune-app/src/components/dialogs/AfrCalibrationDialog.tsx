/**
 * AFR Sensor Calibration dialog (TunerStudio's "Calibrate AFR Sensor").
 *
 * Builds the 1024-entry ADC→AFR transfer curve for Speeduino's O2
 * calibration space and writes it through the dedicated `t` calibration
 * command. Presets are the linear entries from the Speeduino INI's
 * `std_ms2geno2` reference table; the non-linear ones (`table(...)`
 * solutions) need TunerStudio's bundled .inc files and are omitted.
 *
 * The ground-offset field exists for a real failure mode: a wideband whose
 * analog ground sits above ECU ground reads uniformly lean (e.g. a 14Point7
 * Spartan 2 grounded at the wrong point shows +0.125 V ≈ +0.25 AFR). The
 * offset shifts the curve so the ECU decodes the voltage it actually sees.
 */

import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog, Button } from "../common";
import "./AfrCalibrationDialog.css";

interface AfrCalibrationDialogProps {
  isOpen: boolean;
  onClose: () => void;
  connected: boolean;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;
}

/** Linear presets from the Speeduino INI std_ms2geno2 solutions list:
 *  AFR(adc) = afr0 + adc * slopePerAdc. */
const PRESETS: { id: string; name: string; afr0: number; slopePerAdc: number }[] = [
  { id: "14point7", name: "14Point7 (Spartan) 0–5 V = 10–20", afr0: 10.0001, slopePerAdc: 0.0097752 },
  { id: "aem-4200", name: "AEM Linear 30-42xx", afr0: 9.72, slopePerAdc: 0.0096665 },
  { id: "aem-2310", name: "AEM 30-2310 / 30-4900", afr0: 7.3125, slopePerAdc: 0.011608 },
  { id: "autometer", name: "Autometer 0 V=10:1, 4 V=16:1", afr0: 10, slopePerAdc: 0.0073313783 },
  { id: "afr500-916", name: "Ballenger AFR500 0 V=9:1, 5 V=16:1", afr0: 9, slopePerAdc: 0.00684262 },
  { id: "afr500-620", name: "Ballenger AFR500 0 V=6:1, 5 V=20:1", afr0: 6, slopePerAdc: 0.01368524 },
  { id: "daytona", name: "Daytona TwinTec", afr0: 10.01, slopePerAdc: 0.0097752 },
  { id: "dynojet", name: "DynoJet Wideband Commander", afr0: 10, slopePerAdc: 0.00784325 },
  { id: "fast", name: "F.A.S.T. Wideband", afr0: 9.6, slopePerAdc: 0.01357317 },
  { id: "fueltech-gas", name: "Fueltech WB-O2 Nano (Gasoline)", afr0: 8.37391, slopePerAdc: 0.00796111 },
  { id: "lc2", name: "Innovate LC-1 / LC-2 Default", afr0: 7.35, slopePerAdc: 0.01470186 },
  { id: "plx", name: "Innovate / PLX 0–5 V = 10:1–20:1", afr0: 10, slopePerAdc: 0.0097752 },
  { id: "ngk", name: "NGK Powerdex", afr0: 9, slopePerAdc: 0.0068359375 },
  { id: "odg1", name: "ODG Wideband – Faixa 1", afr0: 8.347, slopePerAdc: 0.00795792 },
  { id: "odg2", name: "ODG Wideband – Faixa 2", afr0: 9.1447, slopePerAdc: 0.01013714 },
  { id: "techedge", name: "TechEdge Linear", afr0: 9, slopePerAdc: 0.0097752 },
  { id: "zeitronix", name: "Zeitronix Linear Default", afr0: 9.6, slopePerAdc: 0.0097752 },
];

const STORAGE_KEY = "lt.afrCalibration";

interface StoredSettings {
  presetId: string;
  customV1: number;
  customAfr1: number;
  customV2: number;
  customAfr2: number;
  groundOffsetMv: number;
}

function loadStored(): Partial<StoredSettings> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
  } catch {
    return {};
  }
}

export default function AfrCalibrationDialog({
  isOpen,
  onClose,
  connected,
  showToast,
}: AfrCalibrationDialogProps) {
  const stored = useMemo(loadStored, [isOpen]);
  const [presetId, setPresetId] = useState(stored.presetId ?? "14point7");
  const [customV1, setCustomV1] = useState(stored.customV1 ?? 0);
  const [customAfr1, setCustomAfr1] = useState(stored.customAfr1 ?? 10);
  const [customV2, setCustomV2] = useState(stored.customV2 ?? 5);
  const [customAfr2, setCustomAfr2] = useState(stored.customAfr2 ?? 20);
  const [groundOffsetMv, setGroundOffsetMv] = useState(stored.groundOffsetMv ?? 0);
  const [writing, setWriting] = useState(false);

  // Re-hydrate when the dialog reopens (stored recomputes on isOpen).
  useEffect(() => {
    if (!isOpen) return;
    const s = loadStored();
    if (s.presetId) setPresetId(s.presetId);
    if (s.customV1 !== undefined) setCustomV1(s.customV1);
    if (s.customAfr1 !== undefined) setCustomAfr1(s.customAfr1);
    if (s.customV2 !== undefined) setCustomV2(s.customV2);
    if (s.customAfr2 !== undefined) setCustomAfr2(s.customAfr2);
    if (s.groundOffsetMv !== undefined) setGroundOffsetMv(s.groundOffsetMv);
  }, [isOpen]);

  const isCustom = presetId === "custom";
  const customValid = !isCustom || Math.abs(customV2 - customV1) > 0.01;

  /** AFR as a function of the voltage at the sensor's output pin. */
  const afrOfVolts = useMemo(() => {
    if (isCustom) {
      const slope = (customAfr2 - customAfr1) / (customV2 - customV1 || 1);
      return (v: number) => customAfr1 + (v - customV1) * slope;
    }
    const p = PRESETS.find((p) => p.id === presetId) ?? PRESETS[0];
    // Preset formulas are per ADC count; 1 V = 1023/5 counts.
    return (v: number) => p.afr0 + (v * 1023) / 5 * p.slopePerAdc;
  }, [presetId, isCustom, customV1, customAfr1, customV2, customAfr2]);

  /** The full 1024-entry curve, ground offset applied: the ECU's ADC sees
   *  the sensor voltage plus the offset, so the AFR that voltage really
   *  means is the curve evaluated at (measured − offset). */
  const curve = useMemo(() => {
    const offsetV = groundOffsetMv / 1000;
    const out = new Array<number>(1024);
    for (let adc = 0; adc < 1024; adc++) {
      const measuredV = (adc * 5) / 1023;
      const afr = afrOfVolts(measuredV - offsetV);
      out[adc] = Math.min(25.5, Math.max(0, afr));
    }
    return out;
  }, [afrOfVolts, groundOffsetMv]);

  const previewPath = useMemo(() => {
    const w = 360;
    const h = 120;
    const afrMin = 5;
    const afrMax = 26;
    const pts: string[] = [];
    for (let adc = 0; adc < 1024; adc += 16) {
      const x = (adc / 1023) * w;
      const y = h - ((curve[adc] - afrMin) / (afrMax - afrMin)) * h;
      pts.push(`${x.toFixed(1)},${Math.min(h, Math.max(0, y)).toFixed(1)}`);
    }
    return pts.join(" ");
  }, [curve]);

  async function handleWrite() {
    setWriting(true);
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          presetId,
          customV1,
          customAfr1,
          customV2,
          customAfr2,
          groundOffsetMv,
        } satisfies StoredSettings),
      );
      await invoke("write_afr_calibration", { afrValues: curve });
      showToast("AFR sensor calibration written to ECU", "success");
      onClose();
    } catch (e) {
      showToast("Failed to write AFR calibration: " + e, "error");
    } finally {
      setWriting(false);
    }
  }

  const fmt = (v: number) => v.toFixed(2);

  return (
    <Dialog open={isOpen} onClose={onClose} title="Calibrate AFR Sensor" size="md">
      <Dialog.Body className="afrcal-body">
        <p className="afrcal-subtitle">
          Maps the wideband controller's 0–5 V analog output to AFR for the
          ECU's O2 input. Written to the Speeduino calibration space
          (separate from the tune pages).
        </p>

        <div className="afrcal-field">
          <label>Wideband controller</label>
          <select value={presetId} onChange={(e) => setPresetId(e.target.value)}>
            {PRESETS.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
            <option value="custom">Custom Linear WB…</option>
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
          <label>
            Ground offset (mV) — voltage the ECU reads <em>above</em> the
            sensor's true output
          </label>
          <input
            type="number"
            step="5"
            value={groundOffsetMv}
            onChange={(e) => setGroundOffsetMv(parseFloat(e.target.value) || 0)}
          />
          <span className="afrcal-hint">
            Non-zero only when the controller is grounded away from the ECU
            ground point. Measure it by logging the controller's test/sequencer
            plateaus, or fix the wiring and leave this at 0.
          </span>
        </div>

        <div className="afrcal-preview">
          <svg viewBox="0 0 360 120" preserveAspectRatio="none" aria-hidden>
            <polyline points={previewPath} fill="none" strokeWidth="2" className="afrcal-line" />
          </svg>
          <div className="afrcal-endpoints">
            <span>0 V → {fmt(curve[0])}</span>
            <span>2.5 V → {fmt(curve[512])}</span>
            <span>5 V → {fmt(curve[1023])}</span>
          </div>
        </div>

        <p className="afrcal-warning">
          Write with the engine off. On the legacy serial protocol the ECU
          sends no acknowledgement, so re-check a logged AFR value afterwards;
          on the CRC protocol the write is verified automatically.
        </p>
        {!connected && <p className="afrcal-error">Not connected to an ECU.</p>}
        {!customValid && (
          <p className="afrcal-error">The two custom points need different voltages.</p>
        )}
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose}>
          Cancel
        </Button>
        <Button
          variant="primary"
          onClick={handleWrite}
          disabled={!connected || writing || !customValid}
        >
          {writing ? "Writing…" : "Write Calibration"}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
