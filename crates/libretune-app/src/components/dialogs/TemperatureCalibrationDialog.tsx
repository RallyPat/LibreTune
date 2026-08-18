/**
 * Temperature Sensor Calibration dialog (TunerStudio's "Calibrate
 * Thermistor Tables").
 *
 * Hosts the ThermistorWizard (Steinhart–Hart fit from datasheet points) and
 * turns its result into Speeduino's 32-point calibration curve: the fitted
 * sensor curve is sampled at the exact ADC bins the connected firmware will
 * assign (they differ between the legacy and CRC protocol paths), converted
 * through the ECU's bias-resistor divider, and written to the calibration
 * space with the dedicated `t` command.
 */

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog } from "../common";
import ThermistorWizard, {
  resistanceToTemperature,
  type SteinhartCoefficients,
} from "../wizards/ThermistorWizard";
import "./TemperatureCalibrationDialog.css";

interface TemperatureCalibrationDialogProps {
  isOpen: boolean;
  onClose: () => void;
  connected: boolean;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;
}

interface CalibrationBinsInfo {
  bins: number[];
  modern_protocol: boolean;
  connected: boolean;
}

type SensorKind = "clt" | "iat";

/** Temperature at one ADC count, via the ECU's divider (pull-up bias to 5 V,
 *  thermistor to ground: ADC = 1023·R/(R+bias)) and the fitted curve. The
 *  Steinhart–Hart extrapolation degenerates toward 0 K at both extremes of
 *  the ADC range, so out-of-range results clamp to the hot end below
 *  mid-scale and the cold end above it. */
function temperatureAtAdc(
  adc: number,
  bias: number,
  coeffs: SteinhartCoefficients,
): number {
  const a = Math.min(1022, Math.max(1, adc));
  const resistance = (bias * a) / (1023 - a);
  const t = resistanceToTemperature(resistance, coeffs);
  if (!Number.isFinite(t) || t < -60 || t > 300) {
    return adc < 512 ? 300 : -60;
  }
  return t;
}

export default function TemperatureCalibrationDialog({
  isOpen,
  onClose,
  connected,
  showToast,
}: TemperatureCalibrationDialogProps) {
  const [sensor, setSensor] = useState<SensorKind>("clt");
  const [writing, setWriting] = useState(false);

  async function handleComplete(
    coeffs: SteinhartCoefficients,
    _lookupTable: number[][],
    biasResistor: number,
  ) {
    if (!connected) {
      showToast("Not connected to an ECU — calibration not written.", "warning");
      return;
    }
    setWriting(true);
    try {
      const info = await invoke<CalibrationBinsInfo>("get_temperature_calibration_bins");
      const tempsC = info.bins.map((adc) => temperatureAtAdc(adc, biasResistor, coeffs));
      await invoke("write_temperature_calibration", { sensor, tempsC });
      showToast(
        `${sensor === "clt" ? "Coolant" : "Intake air"} sensor calibration written to ECU`,
        "success",
      );
      onClose();
    } catch (e) {
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
        <p className="tempcal-note">
          "Apply Calibration" samples the fitted curve at the 32 ADC points the
          connected ECU uses and writes them with the engine off. On the legacy
          serial protocol there is no acknowledgement; verify the gauge reads
          sensibly afterwards.
        </p>
      </Dialog.Body>
    </Dialog>
  );
}
