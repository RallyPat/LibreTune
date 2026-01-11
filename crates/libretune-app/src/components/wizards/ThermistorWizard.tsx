/**
 * Thermistor Wizard Component
 * 
 * Provides a step-by-step wizard for calibrating NTC thermistor sensors.
 * Users can:
 * - Enter resistance/temperature data points (from datasheet or measured)
 * - Calculate Steinhart-Hart coefficients (A, B, C)
 * - Preview the calibration curve
 * - Generate lookup table data for ECU configuration
 * 
 * The Steinhart-Hart equation: 1/T = A + B*ln(R) + C*(ln(R))^3
 * where T is in Kelvin and R is resistance in Ohms
 */

import { useState, useMemo, useCallback } from 'react';
import { 
  Plus, 
  Trash2, 
  Download, 
  ChevronRight,
  ChevronLeft,
  Thermometer,
  AlertCircle,
  Check
} from 'lucide-react';
import './ThermistorWizard.css';

interface DataPoint {
  id: string;
  temperature: number; // °C
  resistance: number;  // Ohms
}

interface SteinhartCoefficients {
  a: number;
  b: number;
  c: number;
}

interface ThermistorWizardProps {
  onComplete?: (coefficients: SteinhartCoefficients, lookupTable: number[][]) => void;
  onCancel?: () => void;
  initialPoints?: DataPoint[];
  biasResistor?: number; // Pullup/pulldown resistor value for ADC calculation
}

// Convert Celsius to Kelvin
const toKelvin = (celsius: number): number => celsius + 273.15;

// Convert Kelvin to Celsius
const toCelsius = (kelvin: number): number => kelvin - 273.15;

// Calculate Steinhart-Hart coefficients from 3 data points
function calculateSteinhartHart(points: DataPoint[]): SteinhartCoefficients | null {
  if (points.length < 3) return null;
  
  // Use first, middle, and last points for best accuracy
  const sortedPoints = [...points].sort((a, b) => a.temperature - b.temperature);
  const p1 = sortedPoints[0];
  const p2 = sortedPoints[Math.floor(sortedPoints.length / 2)];
  const p3 = sortedPoints[sortedPoints.length - 1];
  
  // Convert to Kelvin
  const T1 = toKelvin(p1.temperature);
  const T2 = toKelvin(p2.temperature);
  const T3 = toKelvin(p3.temperature);
  
  // Natural log of resistances
  const L1 = Math.log(p1.resistance);
  const L2 = Math.log(p2.resistance);
  const L3 = Math.log(p3.resistance);
  
  // Intermediate calculations
  const Y1 = 1 / T1;
  const Y2 = 1 / T2;
  const Y3 = 1 / T3;
  
  const g2 = (Y2 - Y1) / (L2 - L1);
  const g3 = (Y3 - Y1) / (L3 - L1);
  
  // Steinhart-Hart coefficients
  const c = ((g3 - g2) / (L3 - L2)) / (L1 + L2 + L3);
  const b = g2 - c * (L1 * L1 + L1 * L2 + L2 * L2);
  const a = Y1 - (b + c * L1 * L1) * L1;
  
  return { a, b, c };
}

// Calculate temperature from resistance using Steinhart-Hart
function resistanceToTemperature(resistance: number, coeffs: SteinhartCoefficients): number {
  const lnR = Math.log(resistance);
  const invT = coeffs.a + coeffs.b * lnR + coeffs.c * Math.pow(lnR, 3);
  return toCelsius(1 / invT);
}

// Generate lookup table (resistance -> temperature)
function generateLookupTable(
  coeffs: SteinhartCoefficients, 
  minTemp: number, 
  maxTemp: number, 
  steps: number
): number[][] {
  const table: number[][] = [];
  const tempStep = (maxTemp - minTemp) / (steps - 1);
  
  for (let i = 0; i < steps; i++) {
    const temp = minTemp + i * tempStep;
    // Invert Steinhart-Hart to get resistance from temperature
    // This is an approximation using Newton-Raphson or binary search
    let resistance = 10000; // Initial guess
    for (let iter = 0; iter < 20; iter++) {
      const calcTemp = resistanceToTemperature(resistance, coeffs);
      const error = calcTemp - temp;
      if (Math.abs(error) < 0.01) break;
      // Adjust resistance (thermistors have negative temperature coefficient)
      resistance *= Math.exp(error * 0.01);
    }
    table.push([Math.round(temp), Math.round(resistance)]);
  }
  
  return table;
}

// Calculate ADC counts from resistance using voltage divider
function resistanceToAdc(resistance: number, biasResistor: number, adcBits: number = 10): number {
  const vRatio = resistance / (resistance + biasResistor);
  return Math.round(vRatio * (Math.pow(2, adcBits) - 1));
}

export default function ThermistorWizard({
  onComplete,
  onCancel,
  initialPoints = [],
  biasResistor = 2490, // Common value for automotive sensors
}: ThermistorWizardProps) {
  const [step, setStep] = useState(1);
  const [dataPoints, setDataPoints] = useState<DataPoint[]>(
    initialPoints.length > 0 ? initialPoints : [
      { id: '1', temperature: -40, resistance: 100000 },
      { id: '2', temperature: 20, resistance: 2500 },
      { id: '3', temperature: 100, resistance: 200 },
    ]
  );
  const [selectedPreset, setSelectedPreset] = useState<string>('custom');
  const [biasResistorValue, setBiasResistorValue] = useState(biasResistor);
  const [tableRange, setTableRange] = useState({ min: -40, max: 150, steps: 20 });

  // Common thermistor presets
  const presets = [
    { id: 'gm-iat', name: 'GM IAT (2.5kΩ @ 20°C)', points: [
      { id: '1', temperature: -40, resistance: 100700 },
      { id: '2', temperature: -20, resistance: 28680 },
      { id: '3', temperature: 0, resistance: 9820 },
      { id: '4', temperature: 20, resistance: 3900 },
      { id: '5', temperature: 40, resistance: 1180 },
      { id: '6', temperature: 60, resistance: 470 },
    ]},
    { id: 'gm-clt', name: 'GM CLT (10kΩ @ 20°C)', points: [
      { id: '1', temperature: -40, resistance: 100700 },
      { id: '2', temperature: -20, resistance: 28680 },
      { id: '3', temperature: 0, resistance: 9820 },
      { id: '4', temperature: 20, resistance: 3900 },
      { id: '5', temperature: 60, resistance: 470 },
      { id: '6', temperature: 100, resistance: 177 },
    ]},
    { id: 'bosch-ntc', name: 'Bosch NTC (2.5kΩ @ 20°C)', points: [
      { id: '1', temperature: -30, resistance: 24270 },
      { id: '2', temperature: 0, resistance: 5790 },
      { id: '3', temperature: 20, resistance: 2500 },
      { id: '4', temperature: 40, resistance: 1070 },
      { id: '5', temperature: 80, resistance: 270 },
      { id: '6', temperature: 120, resistance: 100 },
    ]},
    { id: 'custom', name: 'Custom / Manual Entry', points: [] },
  ];

  // Calculate coefficients when we have enough data points
  const coefficients = useMemo(() => {
    if (dataPoints.length >= 3) {
      return calculateSteinhartHart(dataPoints);
    }
    return null;
  }, [dataPoints]);

  // Calculate curve fit error
  const fitError = useMemo(() => {
    if (!coefficients || dataPoints.length < 3) return null;
    
    let totalError = 0;
    dataPoints.forEach(point => {
      const calculated = resistanceToTemperature(point.resistance, coefficients);
      totalError += Math.abs(calculated - point.temperature);
    });
    return totalError / dataPoints.length;
  }, [coefficients, dataPoints]);

  // Generate lookup table
  const lookupTable = useMemo(() => {
    if (!coefficients) return [];
    return generateLookupTable(coefficients, tableRange.min, tableRange.max, tableRange.steps);
  }, [coefficients, tableRange]);

  // Add data point
  const addDataPoint = useCallback(() => {
    const newId = Date.now().toString();
    setDataPoints(prev => [...prev, { id: newId, temperature: 25, resistance: 2000 }]);
  }, []);

  // Remove data point
  const removeDataPoint = useCallback((id: string) => {
    setDataPoints(prev => prev.filter(p => p.id !== id));
  }, []);

  // Update data point
  const updateDataPoint = useCallback((id: string, field: 'temperature' | 'resistance', value: number) => {
    setDataPoints(prev => prev.map(p => 
      p.id === id ? { ...p, [field]: value } : p
    ));
  }, []);

  // Apply preset
  const applyPreset = useCallback((presetId: string) => {
    setSelectedPreset(presetId);
    const preset = presets.find(p => p.id === presetId);
    if (preset && preset.points.length > 0) {
      setDataPoints(preset.points);
    }
  }, [presets]);

  // Export as CSV
  const exportCsv = useCallback(() => {
    const lines = ['Temperature (°C),Resistance (Ω),ADC Counts'];
    lookupTable.forEach(([temp, resistance]) => {
      const adc = resistanceToAdc(resistance, biasResistorValue);
      lines.push(`${temp},${resistance},${adc}`);
    });
    
    const blob = new Blob([lines.join('\n')], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'thermistor_calibration.csv';
    a.click();
    URL.revokeObjectURL(url);
  }, [lookupTable, biasResistorValue]);

  // Handle completion
  const handleComplete = useCallback(() => {
    if (coefficients) {
      onComplete?.(coefficients, lookupTable);
    }
  }, [coefficients, lookupTable, onComplete]);

  return (
    <div className="thermistor-wizard">
      {/* Header */}
      <div className="wizard-header">
        <Thermometer size={24} />
        <h2>Thermistor Calibration Wizard</h2>
      </div>

      {/* Step indicator */}
      <div className="wizard-steps">
        <div className={`step ${step >= 1 ? 'active' : ''} ${step > 1 ? 'completed' : ''}`}>
          <span className="step-number">1</span>
          <span className="step-label">Data Entry</span>
        </div>
        <ChevronRight size={16} className="step-arrow" />
        <div className={`step ${step >= 2 ? 'active' : ''} ${step > 2 ? 'completed' : ''}`}>
          <span className="step-number">2</span>
          <span className="step-label">Curve Fit</span>
        </div>
        <ChevronRight size={16} className="step-arrow" />
        <div className={`step ${step >= 3 ? 'active' : ''}`}>
          <span className="step-number">3</span>
          <span className="step-label">Generate Table</span>
        </div>
      </div>

      {/* Step content */}
      <div className="wizard-content">
        {step === 1 && (
          <div className="step-data-entry">
            <h3>Enter Temperature/Resistance Data Points</h3>
            <p className="step-description">
              Enter at least 3 data points from your thermistor datasheet or measurements.
              More points improve accuracy.
            </p>

            {/* Presets */}
            <div className="preset-section">
              <label>Sensor Preset:</label>
              <select 
                value={selectedPreset} 
                onChange={(e) => applyPreset(e.target.value)}
              >
                {presets.map(preset => (
                  <option key={preset.id} value={preset.id}>{preset.name}</option>
                ))}
              </select>
            </div>

            {/* Data points table */}
            <div className="data-points-table">
              <div className="table-header">
                <span>Temperature (°C)</span>
                <span>Resistance (Ω)</span>
                <span></span>
              </div>
              {dataPoints.map(point => (
                <div key={point.id} className="table-row">
                  <input
                    type="number"
                    value={point.temperature}
                    onChange={(e) => updateDataPoint(point.id, 'temperature', parseFloat(e.target.value) || 0)}
                    step="1"
                  />
                  <input
                    type="number"
                    value={point.resistance}
                    onChange={(e) => updateDataPoint(point.id, 'resistance', parseFloat(e.target.value) || 0)}
                    step="100"
                  />
                  <button 
                    className="btn-icon danger"
                    onClick={() => removeDataPoint(point.id)}
                    disabled={dataPoints.length <= 3}
                    title="Remove point"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}
            </div>

            <button className="btn-add" onClick={addDataPoint}>
              <Plus size={14} />
              Add Data Point
            </button>

            {dataPoints.length < 3 && (
              <div className="warning-message">
                <AlertCircle size={16} />
                <span>At least 3 data points are required for calibration.</span>
              </div>
            )}
          </div>
        )}

        {step === 2 && (
          <div className="step-curve-fit">
            <h3>Curve Fit Results</h3>
            
            {coefficients ? (
              <>
                <div className="coefficients-display">
                  <h4>Steinhart-Hart Coefficients</h4>
                  <div className="coeff-grid">
                    <div className="coeff">
                      <span className="coeff-label">A</span>
                      <span className="coeff-value">{coefficients.a.toExponential(6)}</span>
                    </div>
                    <div className="coeff">
                      <span className="coeff-label">B</span>
                      <span className="coeff-value">{coefficients.b.toExponential(6)}</span>
                    </div>
                    <div className="coeff">
                      <span className="coeff-label">C</span>
                      <span className="coeff-value">{coefficients.c.toExponential(6)}</span>
                    </div>
                  </div>
                  
                  {fitError !== null && (
                    <div className={`fit-error ${fitError < 1 ? 'good' : fitError < 3 ? 'ok' : 'poor'}`}>
                      <span>Average Fit Error: {fitError.toFixed(2)}°C</span>
                      {fitError < 1 && <Check size={14} />}
                    </div>
                  )}
                </div>

                {/* Curve preview (simplified text representation) */}
                <div className="curve-preview">
                  <h4>Calibration Curve Preview</h4>
                  <div className="curve-table">
                    <div className="curve-header">
                      <span>Temp (°C)</span>
                      <span>Resistance (Ω)</span>
                      <span>Calculated (°C)</span>
                      <span>Error</span>
                    </div>
                    {dataPoints.map(point => {
                      const calculated = resistanceToTemperature(point.resistance, coefficients);
                      const error = calculated - point.temperature;
                      return (
                        <div key={point.id} className="curve-row">
                          <span>{point.temperature}</span>
                          <span>{point.resistance.toLocaleString()}</span>
                          <span>{calculated.toFixed(1)}</span>
                          <span className={Math.abs(error) < 1 ? 'good' : Math.abs(error) < 3 ? 'ok' : 'poor'}>
                            {error >= 0 ? '+' : ''}{error.toFixed(1)}°C
                          </span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              </>
            ) : (
              <div className="error-message">
                <AlertCircle size={16} />
                <span>Unable to calculate coefficients. Please check your data points.</span>
              </div>
            )}
          </div>
        )}

        {step === 3 && (
          <div className="step-generate-table">
            <h3>Generate Lookup Table</h3>
            
            {/* Table range settings */}
            <div className="table-settings">
              <div className="setting-group">
                <label>Min Temperature (°C)</label>
                <input
                  type="number"
                  value={tableRange.min}
                  onChange={(e) => setTableRange(prev => ({ ...prev, min: parseInt(e.target.value) || -40 }))}
                />
              </div>
              <div className="setting-group">
                <label>Max Temperature (°C)</label>
                <input
                  type="number"
                  value={tableRange.max}
                  onChange={(e) => setTableRange(prev => ({ ...prev, max: parseInt(e.target.value) || 150 }))}
                />
              </div>
              <div className="setting-group">
                <label>Table Size (rows)</label>
                <input
                  type="number"
                  min={5}
                  max={64}
                  value={tableRange.steps}
                  onChange={(e) => setTableRange(prev => ({ ...prev, steps: parseInt(e.target.value) || 20 }))}
                />
              </div>
              <div className="setting-group">
                <label>Bias Resistor (Ω)</label>
                <input
                  type="number"
                  value={biasResistorValue}
                  onChange={(e) => setBiasResistorValue(parseInt(e.target.value) || 2490)}
                />
              </div>
            </div>

            {/* Generated table preview */}
            <div className="lookup-table-preview">
              <h4>Lookup Table</h4>
              <div className="lookup-table">
                <div className="lookup-header">
                  <span>Temp (°C)</span>
                  <span>Resistance (Ω)</span>
                  <span>ADC (10-bit)</span>
                </div>
                <div className="lookup-body">
                  {lookupTable.map(([temp, resistance], i) => (
                    <div key={i} className="lookup-row">
                      <span>{temp}</span>
                      <span>{resistance.toLocaleString()}</span>
                      <span>{resistanceToAdc(resistance, biasResistorValue)}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            {/* Export button */}
            <button className="btn-export" onClick={exportCsv}>
              <Download size={16} />
              Export as CSV
            </button>
          </div>
        )}
      </div>

      {/* Navigation buttons */}
      <div className="wizard-footer">
        <button className="btn-cancel" onClick={onCancel}>
          Cancel
        </button>
        <div className="nav-buttons">
          {step > 1 && (
            <button className="btn-back" onClick={() => setStep(s => s - 1)}>
              <ChevronLeft size={16} />
              Back
            </button>
          )}
          {step < 3 ? (
            <button 
              className="btn-next" 
              onClick={() => setStep(s => s + 1)}
              disabled={step === 1 && dataPoints.length < 3}
            >
              Next
              <ChevronRight size={16} />
            </button>
          ) : (
            <button className="btn-complete" onClick={handleComplete}>
              <Check size={16} />
              Apply Calibration
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
