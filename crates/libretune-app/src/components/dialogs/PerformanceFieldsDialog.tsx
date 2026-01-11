//! Performance Fields Dialog Component
//!
//! Implements performance and economy calculations for ECU tuning.
//! Calculates:
//! - Vehicle performance: HP, torque, drag, acceleration
//! - Fuel economy: MPG, MPL, KPL, CC/min, gallons/hour
//! - Uses vehicle specs from user input

import { useState, useEffect, useMemo, useCallback } from 'react';
import { X, Car, Gauge, Fuel, Timer, Calculator, Save, RotateCcw } from 'lucide-react';
import './PerformanceFieldsDialog.css';

export interface VehicleSpecs {
  injector_size_cc?: number;
  weight_lbs?: number;
  weight_kg?: number;
  frontal_area_sqft?: number;
  frontal_area_sqm?: number;
  tire_pressure_psi?: number;
  tire_diameter_in?: number;
  drag_coefficient?: number;
  drivetrain_loss?: number; // percentage
  gear_ratios?: number[];
  final_drive?: number;
}

export type WeightUnit = "LBS" | "KG";

export interface PerformanceCalcs {
  speed_source: SpeedSource;
  speed_units: SpeedUnits;
  fuel_units: FuelUnits;
  
  speed_mph?: number;
  speed_kmh?: number;
  
  fuel_cc_per_min?: number;
  fuel_gallons_per_hour?: number;
  fuel_liters_per_hour?: number;
  
  distance_miles?: number;
  distance_kilometers?: number;
  
  fuel_gallons_consumed?: number;
  fuel_liters_consumed?: number;
  
  mpg?: number;
  mpl?: number;
  kpl?: number;
  
  hp?: number;
  torque?: number;
  drag?: number;
  rolling_drag?: number;
  acceleration?: number;
}

export function createPerformanceCalcs(): PerformanceCalcs {
  return {
    speed_source: "ECU",
    speed_units: "MPH",
    fuel_units: "GallonsUS",
  };
}

export type SpeedSource = "ECU" | "GPS";
export type SpeedUnits = "MPH" | "KM/H";

export type FuelUnits = "GallonsUS" | "GallonsImperial" | "Liters";

// Physics constants
const AIR_DENSITY_KG_M3 = 1.225; // kg/m³ at sea level
const ROLLING_RESISTANCE_COEFF = 0.015; // typical tire on asphalt
const GRAVITY = 9.81; // m/s²
const LBS_TO_KG = 0.453592;
const SQFT_TO_SQM = 0.092903;
const MPH_TO_MS = 0.44704;
const HP_TO_WATTS = 745.7;
const NM_TO_LBFT = 0.737562;

// Performance calculation functions
function calculateDragForce(
  velocity_ms: number,
  frontalArea_sqm: number,
  dragCoeff: number
): number {
  // Fd = 0.5 * ρ * v² * Cd * A
  return 0.5 * AIR_DENSITY_KG_M3 * Math.pow(velocity_ms, 2) * dragCoeff * frontalArea_sqm;
}

function calculateRollingResistance(mass_kg: number): number {
  // Fr = Crr * m * g
  return ROLLING_RESISTANCE_COEFF * mass_kg * GRAVITY;
}

function calculatePowerFromAccel(
  mass_kg: number,
  accel_ms2: number,
  velocity_ms: number,
  frontalArea_sqm: number,
  dragCoeff: number,
  drivetrainLoss: number
): number {
  // Force = ma + Fd + Fr
  const dragForce = calculateDragForce(velocity_ms, frontalArea_sqm, dragCoeff);
  const rollingForce = calculateRollingResistance(mass_kg);
  const accelForce = mass_kg * accel_ms2;
  
  const totalForce = accelForce + dragForce + rollingForce;
  const wheelPower = totalForce * velocity_ms;
  
  // Account for drivetrain losses
  const enginePower = wheelPower / (1 - drivetrainLoss / 100);
  
  return enginePower; // watts
}

function calculateTorque(power_watts: number, rpm: number): number {
  // P = τ * ω, where ω = 2π * rpm / 60
  const angularVelocity = (2 * Math.PI * rpm) / 60;
  return angularVelocity > 0 ? power_watts / angularVelocity : 0;
}

function estimateQuarterMile(hp: number, weight_lbs: number): { time: number; speed: number } {
  // Empirical formula: ET ≈ 5.825 * (W/HP)^(1/3)
  // Trap speed (mph) ≈ 234 * (HP/W)^(1/3)
  const ratio = weight_lbs / hp;
  const time = 5.825 * Math.pow(ratio, 1/3);
  const speed = 234 * Math.pow(hp / weight_lbs, 1/3);
  return { time, speed };
}

function estimate0to60(hp: number, weight_lbs: number): number {
  // Empirical formula: 0-60 ≈ 3.7 * (W/HP)^(0.42)
  const ratio = weight_lbs / hp;
  return 3.7 * Math.pow(ratio, 0.42);
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
  realtimeData?: Record<string, number>;
}

const STORAGE_KEY = 'libretune-vehicle-specs';

export default function PerformanceFieldsDialog({ isOpen, onClose, realtimeData }: Props) {
  // Vehicle specs form state
  const [weightLbs, setWeightLbs] = useState(3000);
  const [weightUnit, setWeightUnit] = useState<WeightUnit>('LBS');
  const [injectorSizeCc, setInjectorSizeCc] = useState(440);
  const [frontalAreaSqft, setFrontalAreaSqft] = useState(22);
  const [dragCoefficient, setDragCoefficient] = useState(0.35);
  const [tireDiameterIn, setTireDiameterIn] = useState(26);
  const [tirePressurePsi, setTirePressurePsi] = useState(32);
  const [drivetrainLoss, setDrivetrainLoss] = useState(15);
  const [finalDrive, setFinalDrive] = useState(3.55);
  const [gearRatios, setGearRatios] = useState<string>('2.97, 2.07, 1.43, 1.00, 0.84');
  
  // Realtime values for calculation
  const [manualRpm, setManualRpm] = useState(3000);
  const [manualSpeed, setManualSpeed] = useState(60);
  const [manualAccel, setManualAccel] = useState(0.3); // g
  
  const [useRealtime, setUseRealtime] = useState(true);
  
  // Load saved specs on mount
  useEffect(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const specs = JSON.parse(saved);
        if (specs.weightLbs) setWeightLbs(specs.weightLbs);
        if (specs.weightUnit) setWeightUnit(specs.weightUnit);
        if (specs.injectorSizeCc) setInjectorSizeCc(specs.injectorSizeCc);
        if (specs.frontalAreaSqft) setFrontalAreaSqft(specs.frontalAreaSqft);
        if (specs.dragCoefficient) setDragCoefficient(specs.dragCoefficient);
        if (specs.tireDiameterIn) setTireDiameterIn(specs.tireDiameterIn);
        if (specs.tirePressurePsi) setTirePressurePsi(specs.tirePressurePsi);
        if (specs.drivetrainLoss) setDrivetrainLoss(specs.drivetrainLoss);
        if (specs.finalDrive) setFinalDrive(specs.finalDrive);
        if (specs.gearRatios) setGearRatios(specs.gearRatios);
      }
    } catch {
      // Ignore
    }
  }, []);
  
  const handleSave = useCallback(() => {
    const specs = {
      weightLbs,
      weightUnit,
      injectorSizeCc,
      frontalAreaSqft,
      dragCoefficient,
      tireDiameterIn,
      tirePressurePsi,
      drivetrainLoss,
      finalDrive,
      gearRatios,
    };
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(specs));
    } catch {
      // Ignore
    }
  }, [weightLbs, weightUnit, injectorSizeCc, frontalAreaSqft, dragCoefficient, tireDiameterIn, tirePressurePsi, drivetrainLoss, finalDrive, gearRatios]);
  
  const handleReset = useCallback(() => {
    setWeightLbs(3000);
    setWeightUnit('LBS');
    setInjectorSizeCc(440);
    setFrontalAreaSqft(22);
    setDragCoefficient(0.35);
    setTireDiameterIn(26);
    setTirePressurePsi(32);
    setDrivetrainLoss(15);
    setFinalDrive(3.55);
    setGearRatios('2.97, 2.07, 1.43, 1.00, 0.84');
  }, []);
  
  // Get values from realtime or manual
  const rpm = useRealtime && realtimeData?.rpm ? realtimeData.rpm : manualRpm;
  const speedMph = useRealtime && realtimeData?.vss ? realtimeData.vss : manualSpeed;
  const accelG = useRealtime && realtimeData?.accel ? realtimeData.accel : manualAccel;
  
  // Calculate performance metrics
  const metrics = useMemo(() => {
    const massKg = weightUnit === 'LBS' ? weightLbs * LBS_TO_KG : weightLbs;
    const massLbs = weightUnit === 'LBS' ? weightLbs : weightLbs / LBS_TO_KG;
    const frontalAreaSqm = frontalAreaSqft * SQFT_TO_SQM;
    const velocityMs = speedMph * MPH_TO_MS;
    const accelMs2 = accelG * GRAVITY;
    
    // Power calculation from acceleration
    const powerWatts = calculatePowerFromAccel(
      massKg,
      accelMs2,
      velocityMs,
      frontalAreaSqm,
      dragCoefficient,
      drivetrainLoss
    );
    const hp = powerWatts / HP_TO_WATTS;
    
    // Torque from power and RPM
    const torqueNm = calculateTorque(powerWatts, rpm);
    const torqueLbFt = torqueNm * NM_TO_LBFT;
    
    // Drag forces
    const dragForceN = calculateDragForce(velocityMs, frontalAreaSqm, dragCoefficient);
    const rollingForceN = calculateRollingResistance(massKg);
    
    // Acceleration estimates
    const quarterMile = estimateQuarterMile(hp, massLbs);
    const zeroTo60 = estimate0to60(hp, massLbs);
    
    return {
      hp: Math.max(0, hp),
      torqueNm: Math.max(0, torqueNm),
      torqueLbFt: Math.max(0, torqueLbFt),
      dragForceN,
      dragForceLbs: dragForceN / 4.448,
      rollingForceN,
      rollingForceLbs: rollingForceN / 4.448,
      quarterMileTime: quarterMile.time,
      quarterMileSpeed: quarterMile.speed,
      zeroTo60,
      massKg,
      massLbs,
    };
  }, [weightLbs, weightUnit, frontalAreaSqft, dragCoefficient, drivetrainLoss, speedMph, rpm, accelG]);
  
  if (!isOpen) return null;
  
  return (
    <div className="performance-dialog-overlay" onClick={onClose}>
      <div className="performance-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="performance-dialog-header">
          <h2><Calculator size={20} /> Performance Calculator</h2>
          <button className="close-btn" onClick={onClose}>
            <X size={18} />
          </button>
        </div>
        
        <div className="performance-dialog-content">
          {/* Left Column - Vehicle Specs */}
          <div className="performance-section">
            <h3><Car size={16} /> Vehicle Specifications</h3>
            
            <div className="form-row">
              <label>Vehicle Weight</label>
              <div className="input-with-unit">
                <input
                  type="number"
                  value={weightLbs}
                  onChange={(e) => setWeightLbs(parseFloat(e.target.value) || 0)}
                  step={100}
                />
                <select value={weightUnit} onChange={(e) => setWeightUnit(e.target.value as WeightUnit)}>
                  <option value="LBS">lbs</option>
                  <option value="KG">kg</option>
                </select>
              </div>
            </div>
            
            <div className="form-row">
              <label>Frontal Area (sq ft)</label>
              <input
                type="number"
                value={frontalAreaSqft}
                onChange={(e) => setFrontalAreaSqft(parseFloat(e.target.value) || 0)}
                step={0.5}
              />
            </div>
            
            <div className="form-row">
              <label>Drag Coefficient (Cd)</label>
              <input
                type="number"
                value={dragCoefficient}
                onChange={(e) => setDragCoefficient(parseFloat(e.target.value) || 0)}
                step={0.01}
                min={0.1}
                max={1.0}
              />
            </div>
            
            <div className="form-row">
              <label>Tire Diameter (in)</label>
              <input
                type="number"
                value={tireDiameterIn}
                onChange={(e) => setTireDiameterIn(parseFloat(e.target.value) || 0)}
                step={0.5}
              />
            </div>
            
            <div className="form-row">
              <label>Tire Pressure (PSI)</label>
              <input
                type="number"
                value={tirePressurePsi}
                onChange={(e) => setTirePressurePsi(parseFloat(e.target.value) || 0)}
                step={1}
              />
            </div>
            
            <div className="form-row">
              <label>Drivetrain Loss (%)</label>
              <input
                type="number"
                value={drivetrainLoss}
                onChange={(e) => setDrivetrainLoss(parseFloat(e.target.value) || 0)}
                step={1}
                min={0}
                max={30}
              />
              <span className="form-hint">FWD ~12%, RWD ~15%, AWD ~18%</span>
            </div>
            
            <div className="form-row">
              <label>Final Drive Ratio</label>
              <input
                type="number"
                value={finalDrive}
                onChange={(e) => setFinalDrive(parseFloat(e.target.value) || 0)}
                step={0.01}
              />
            </div>
            
            <div className="form-row">
              <label>Gear Ratios (comma-sep)</label>
              <input
                type="text"
                value={gearRatios}
                onChange={(e) => setGearRatios(e.target.value)}
                placeholder="2.97, 2.07, 1.43, 1.00, 0.84"
              />
            </div>
            
            <div className="form-row">
              <label>Injector Size (cc)</label>
              <input
                type="number"
                value={injectorSizeCc}
                onChange={(e) => setInjectorSizeCc(parseFloat(e.target.value) || 0)}
                step={10}
              />
            </div>
            
            <div className="button-row">
              <button className="secondary" onClick={handleReset}>
                <RotateCcw size={14} /> Reset
              </button>
              <button className="primary" onClick={handleSave}>
                <Save size={14} /> Save Specs
              </button>
            </div>
          </div>
          
          {/* Center Column - Input Values */}
          <div className="performance-section">
            <h3><Gauge size={16} /> Input Values</h3>
            
            <div className="form-row">
              <label>
                <input
                  type="checkbox"
                  checked={useRealtime}
                  onChange={(e) => setUseRealtime(e.target.checked)}
                />
                Use Realtime Data
              </label>
            </div>
            
            {!useRealtime && (
              <>
                <div className="form-row">
                  <label>RPM</label>
                  <input
                    type="number"
                    value={manualRpm}
                    onChange={(e) => setManualRpm(parseFloat(e.target.value) || 0)}
                    step={100}
                  />
                </div>
                
                <div className="form-row">
                  <label>Speed (mph)</label>
                  <input
                    type="number"
                    value={manualSpeed}
                    onChange={(e) => setManualSpeed(parseFloat(e.target.value) || 0)}
                    step={5}
                  />
                </div>
                
                <div className="form-row">
                  <label>Acceleration (g)</label>
                  <input
                    type="number"
                    value={manualAccel}
                    onChange={(e) => setManualAccel(parseFloat(e.target.value) || 0)}
                    step={0.05}
                    min={0}
                    max={2}
                  />
                </div>
              </>
            )}
            
            {useRealtime && (
              <div className="realtime-values">
                <div className="realtime-value">
                  <span className="label">RPM</span>
                  <span className="value">{rpm.toFixed(0)}</span>
                </div>
                <div className="realtime-value">
                  <span className="label">Speed</span>
                  <span className="value">{speedMph.toFixed(1)} mph</span>
                </div>
                <div className="realtime-value">
                  <span className="label">Accel</span>
                  <span className="value">{accelG.toFixed(2)} g</span>
                </div>
              </div>
            )}
            
            <h3 style={{ marginTop: '1.5rem' }}><Fuel size={16} /> Fuel Economy</h3>
            
            <div className="fuel-info">
              <p className="info-note">
                Fuel economy calculations require datalog data with fuel consumption tracking.
                Connect to ECU and start logging to see live values.
              </p>
            </div>
          </div>
          
          {/* Right Column - Results */}
          <div className="performance-section results">
            <h3><Timer size={16} /> Performance Results</h3>
            
            <div className="result-card primary">
              <span className="result-label">Wheel Horsepower</span>
              <span className="result-value">{metrics.hp.toFixed(1)} <small>HP</small></span>
            </div>
            
            <div className="result-card">
              <span className="result-label">Wheel Torque</span>
              <span className="result-value">{metrics.torqueLbFt.toFixed(1)} <small>lb-ft</small></span>
              <span className="result-alt">{metrics.torqueNm.toFixed(1)} Nm</span>
            </div>
            
            <div className="result-divider">Estimated Acceleration</div>
            
            <div className="result-row">
              <span className="result-label">0-60 mph</span>
              <span className="result-value">{metrics.zeroTo60.toFixed(1)} <small>sec</small></span>
            </div>
            
            <div className="result-row">
              <span className="result-label">1/4 Mile ET</span>
              <span className="result-value">{metrics.quarterMileTime.toFixed(2)} <small>sec</small></span>
            </div>
            
            <div className="result-row">
              <span className="result-label">1/4 Mile Trap</span>
              <span className="result-value">{metrics.quarterMileSpeed.toFixed(1)} <small>mph</small></span>
            </div>
            
            <div className="result-divider">Resistance Forces</div>
            
            <div className="result-row">
              <span className="result-label">Aero Drag</span>
              <span className="result-value">{metrics.dragForceLbs.toFixed(1)} <small>lbs</small></span>
            </div>
            
            <div className="result-row">
              <span className="result-label">Rolling Resistance</span>
              <span className="result-value">{metrics.rollingForceLbs.toFixed(1)} <small>lbs</small></span>
            </div>
            
            <div className="result-note">
              * Calculations use physics-based model. Actual results may vary based on conditions.
            </div>
          </div>
        </div>
        
        <div className="performance-dialog-footer">
          <button onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}