//! Performance Fields Dialog Component
//!
//! Implements performance and economy calculations for ECU tuning.
//! Calculates:
//! - Vehicle performance: HP, torque, drag, acceleration
//! - Fuel economy: MPG, MPL, KPL, CC/min, gallons/hour
//! - Uses vehicle specs from user input

export interface VehicleSpecs {
  injector_size_cc?: number;
  weight_lbs?: number;
  weight_kg?: number;
  frontal_area_sqft?: number;
  frontal_area_sqm?: number;
  tire_pressure_psi?: number;
  drag_coefficient?: number;
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
