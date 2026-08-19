/**
 * Temperature Sensor Calibration dialog (TunerStudio's "Calibrate
 * Thermistor Tables").
 *
 * Hosts the ThermistorWizard (Steinhart–Hart fit from datasheet points) and
 * turns its result into Speeduino's 32-point calibration curve. The fit is
 * redone on the backend by `build_thermistor_curve`, which samples it at the
 * exact ADC bins the connected firmware will assign — the legacy and CRC
 * protocol paths assign different bins, and a curve sampled at the wrong ones
 * is written to the wrong ADC points.
 */

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog } from "../common";
import ThermistorWizard from "../wizards/ThermistorWizard";
import "./TemperatureCalibrationDialog.css";

interface TemperatureCalibrationDialogProps {
  isOpen: boolean;
  onClose: () => void;
  connected: boolean;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;
}

interface CalibrationWriteResult {
  table: string;
  verified: boolean;
  verify_note: string | null;
  transfer_function: string | null;
}

type SensorKind = "clt" | "iat";

export default function TemperatureCalibrationDialog({
  isOpen,
  onClose,
  connected,
  showToast,
}: TemperatureCalibrationDialogProps) {
  const [sensor, setSensor] = useState<SensorKind>("clt");
  const [writing, setWriting] = useState(false);
  const [writeResult, setWriteResult] = useState<CalibrationWriteResult | null>(null);
  /** Last command failure. Shown in the dialog, never only toasted away. */
  const [error, setError] = useState<string | null>(null);

  async function handleComplete(
    _coeffs: unknown,
    _lookupTable: number[][],
    biasResistor: number,
    fitPoints: [number, number][],
  ) {
    if (!connected) {
      setError("Not connected to an ECU — calibration not written.");
      showToast("Not connected to an ECU — calibration not written.", "warning");
      return;
    }
    setWriting(true);
    setError(null);
    setWriteResult(null);
    try {
      const tempsC = await invoke<number[]>("build_thermistor_curve", {
        biasResistor,
        points: fitPoints,
      });
      const result = await invoke<CalibrationWriteResult>("write_temperature_calibration", {
        sensor,
        tempsC,
      });
      setWriteResult(result);
      const name = sensor === "clt" ? "Coolant" : "Intake air";
      showToast(
        result.verified
          ? `${name} sensor calibration written and verified`
          : `${name} sensor calibration written, but not verified`,
        result.verified ? "success" : "warning",
      );
    } catch (e) {
      setError(String(e));
      showToast("Failed to write calibration: " + e, "error");
    } finally {
      setWriting(false);
    }
  }

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="Calibrate Temperature Sensors"
      size="lg"
      className="tempcal-dialog"
    >
      <Dialog.Body className="tempcal-body">
        <div className="tempcal-sensor-row">
          <label>Apply to sensor:</label>
          <select value={sensor} onChange={(e) => setSensor(e.target.value as SensorKind)}>
            <option value="clt">Coolant Temperature (CLT)</option>
            <option value="iat">Intake Air Temperature (IAT)</option>
          </select>
          {!connected && <span className="tempcal-offline">Not connected — preview only</span>}
          {writing && <span className="tempcal-writing">Writing…</span>}
        </div>
        <ThermistorWizard onComplete={handleComplete} onCancel={onClose} />
        {error && <p className="tempcal-error">{error}</p>}
        {writeResult && (
          <p className={writeResult.verified ? "tempcal-note" : "tempcal-error"}>
            {writeResult.verified
              ? `Written and verified by read-back${writeResult.transfer_function ? `: ${writeResult.transfer_function}` : ""}.`
              : `Written, but NOT verified. ${writeResult.verify_note ?? ""}`}
          </p>
        )}
        <p className="tempcal-note">
          "Apply Calibration" refits the curve on the ECU's own 32 ADC points
          and writes it with the engine off. On the legacy serial protocol
          there is no read-back; verify the gauge reads sensibly afterwards.
        </p>
      </Dialog.Body>
    </Dialog>
  );
}
