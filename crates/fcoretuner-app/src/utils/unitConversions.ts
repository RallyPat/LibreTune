/**
 * Unit Conversion Layer
 * 
 * Provides bidirectional conversion between unit systems commonly used in ECU tuning:
 * - Temperature: °C ↔ °F ↔ K
 * - Pressure: kPa ↔ PSI ↔ bar ↔ inHg
 * - Air-Fuel Ratio: AFR ↔ Lambda
 * - Speed: km/h ↔ mph
 * - Volume: L ↔ gal (US)
 * - Fuel: L/100km ↔ mpg
 */

// Unit type categories
export type TemperatureUnit = 'C' | 'F' | 'K';
export type PressureUnit = 'kPa' | 'PSI' | 'bar' | 'inHg';
export type AfrUnit = 'AFR' | 'Lambda';
export type SpeedUnit = 'km/h' | 'mph';
export type VolumeUnit = 'L' | 'gal';
export type FuelEconomyUnit = 'L/100km' | 'mpg';

// Stoichiometric AFR for common fuels
export const STOICH_AFR = {
  gasoline: 14.7,
  e85: 9.8,
  methanol: 6.4,
  ethanol: 9.0,
  diesel: 14.5,
} as const;

export type FuelType = keyof typeof STOICH_AFR;

// =============== Temperature Conversions ===============

export function celsiusToFahrenheit(c: number): number {
  return (c * 9/5) + 32;
}

export function fahrenheitToCelsius(f: number): number {
  return (f - 32) * 5/9;
}

export function celsiusToKelvin(c: number): number {
  return c + 273.15;
}

export function kelvinToCelsius(k: number): number {
  return k - 273.15;
}

export function fahrenheitToKelvin(f: number): number {
  return celsiusToKelvin(fahrenheitToCelsius(f));
}

export function kelvinToFahrenheit(k: number): number {
  return celsiusToFahrenheit(kelvinToCelsius(k));
}

export function convertTemperature(value: number, from: TemperatureUnit, to: TemperatureUnit): number {
  if (from === to) return value;
  
  // Convert to Celsius first
  let celsius: number;
  switch (from) {
    case 'C': celsius = value; break;
    case 'F': celsius = fahrenheitToCelsius(value); break;
    case 'K': celsius = kelvinToCelsius(value); break;
  }
  
  // Convert from Celsius to target
  switch (to) {
    case 'C': return celsius;
    case 'F': return celsiusToFahrenheit(celsius);
    case 'K': return celsiusToKelvin(celsius);
  }
}

// =============== Pressure Conversions ===============

const PRESSURE_TO_KPA: Record<PressureUnit, number> = {
  kPa: 1,
  PSI: 6.89476,
  bar: 100,
  inHg: 3.38639,
};

export function convertPressure(value: number, from: PressureUnit, to: PressureUnit): number {
  if (from === to) return value;
  
  // Convert to kPa, then to target
  const kpa = value * PRESSURE_TO_KPA[from];
  return kpa / PRESSURE_TO_KPA[to];
}

export function kpaToPsi(kpa: number): number {
  return kpa / PRESSURE_TO_KPA.PSI;
}

export function psiToKpa(psi: number): number {
  return psi * PRESSURE_TO_KPA.PSI;
}

export function kpaToBar(kpa: number): number {
  return kpa / PRESSURE_TO_KPA.bar;
}

export function barToKpa(bar: number): number {
  return bar * PRESSURE_TO_KPA.bar;
}

export function kpaToInHg(kpa: number): number {
  return kpa / PRESSURE_TO_KPA.inHg;
}

export function inHgToKpa(inHg: number): number {
  return inHg * PRESSURE_TO_KPA.inHg;
}

// =============== AFR / Lambda Conversions ===============

export function afrToLambda(afr: number, stoichAfr: number = STOICH_AFR.gasoline): number {
  return afr / stoichAfr;
}

export function lambdaToAfr(lambda: number, stoichAfr: number = STOICH_AFR.gasoline): number {
  return lambda * stoichAfr;
}

export function convertAfr(value: number, from: AfrUnit, to: AfrUnit, stoichAfr: number = STOICH_AFR.gasoline): number {
  if (from === to) return value;
  
  if (from === 'AFR' && to === 'Lambda') {
    return afrToLambda(value, stoichAfr);
  } else {
    return lambdaToAfr(value, stoichAfr);
  }
}

// =============== Speed Conversions ===============

export function kmhToMph(kmh: number): number {
  return kmh * 0.621371;
}

export function mphToKmh(mph: number): number {
  return mph / 0.621371;
}

export function convertSpeed(value: number, from: SpeedUnit, to: SpeedUnit): number {
  if (from === to) return value;
  return from === 'km/h' ? kmhToMph(value) : mphToKmh(value);
}

// =============== Volume Conversions ===============

const LITERS_PER_GALLON = 3.78541;

export function litersToGallons(liters: number): number {
  return liters / LITERS_PER_GALLON;
}

export function gallonsToLiters(gallons: number): number {
  return gallons * LITERS_PER_GALLON;
}

export function convertVolume(value: number, from: VolumeUnit, to: VolumeUnit): number {
  if (from === to) return value;
  return from === 'L' ? litersToGallons(value) : gallonsToLiters(value);
}

// =============== Fuel Economy Conversions ===============

export function litersPerHundredKmToMpg(lp100km: number): number {
  // MPG = 235.214583 / L/100km
  if (lp100km <= 0) return Infinity;
  return 235.214583 / lp100km;
}

export function mpgToLitersPerHundredKm(mpg: number): number {
  if (mpg <= 0) return Infinity;
  return 235.214583 / mpg;
}

export function convertFuelEconomy(value: number, from: FuelEconomyUnit, to: FuelEconomyUnit): number {
  if (from === to) return value;
  return from === 'L/100km' ? litersPerHundredKmToMpg(value) : mpgToLitersPerHundredKm(value);
}

// =============== Generic Conversion Interface ===============

export type UnitType = 
  | { category: 'temperature'; unit: TemperatureUnit }
  | { category: 'pressure'; unit: PressureUnit }
  | { category: 'afr'; unit: AfrUnit; stoichAfr?: number }
  | { category: 'speed'; unit: SpeedUnit }
  | { category: 'volume'; unit: VolumeUnit }
  | { category: 'fuel_economy'; unit: FuelEconomyUnit };

export function convertValue(value: number, from: UnitType, to: UnitType): number {
  if (from.category !== to.category) {
    throw new Error(`Cannot convert between different unit categories: ${from.category} → ${to.category}`);
  }
  
  switch (from.category) {
    case 'temperature':
      return convertTemperature(value, from.unit, (to as { category: 'temperature'; unit: TemperatureUnit }).unit);
    case 'pressure':
      return convertPressure(value, from.unit, (to as { category: 'pressure'; unit: PressureUnit }).unit);
    case 'afr':
      return convertAfr(value, from.unit, (to as { category: 'afr'; unit: AfrUnit }).unit, from.stoichAfr);
    case 'speed':
      return convertSpeed(value, from.unit, (to as { category: 'speed'; unit: SpeedUnit }).unit);
    case 'volume':
      return convertVolume(value, from.unit, (to as { category: 'volume'; unit: VolumeUnit }).unit);
    case 'fuel_economy':
      return convertFuelEconomy(value, from.unit, (to as { category: 'fuel_economy'; unit: FuelEconomyUnit }).unit);
    default:
      return value;
  }
}

// =============== Unit Display Helpers ===============

export const UNIT_LABELS: Record<string, string> = {
  // Temperature
  'C': '°C',
  'F': '°F',
  'K': 'K',
  // Pressure
  'kPa': 'kPa',
  'PSI': 'PSI',
  'bar': 'bar',
  'inHg': 'inHg',
  // AFR
  'AFR': 'AFR',
  'Lambda': 'λ',
  // Speed
  'km/h': 'km/h',
  'mph': 'mph',
  // Volume
  'L': 'L',
  'gal': 'gal',
  // Fuel Economy
  'L/100km': 'L/100km',
  'mpg': 'mpg',
};

export function getUnitLabel(unit: string): string {
  return UNIT_LABELS[unit] || unit;
}

// =============== User Preferences ===============

export interface UnitPreferences {
  temperature: TemperatureUnit;
  pressure: PressureUnit;
  afr: AfrUnit;
  speed: SpeedUnit;
  volume: VolumeUnit;
  fuelEconomy: FuelEconomyUnit;
  fuelType: FuelType;
}

export const DEFAULT_PREFERENCES: UnitPreferences = {
  temperature: 'C',
  pressure: 'kPa',
  afr: 'AFR',
  speed: 'km/h',
  volume: 'L',
  fuelEconomy: 'L/100km',
  fuelType: 'gasoline',
};

export const US_PREFERENCES: UnitPreferences = {
  temperature: 'F',
  pressure: 'PSI',
  afr: 'AFR',
  speed: 'mph',
  volume: 'gal',
  fuelEconomy: 'mpg',
  fuelType: 'gasoline',
};

// =============== Auto-Detect Unit from String ===============

const UNIT_PATTERNS: [RegExp, UnitType][] = [
  // Temperature
  [/°?C(elsius)?$/i, { category: 'temperature', unit: 'C' }],
  [/°?F(ahrenheit)?$/i, { category: 'temperature', unit: 'F' }],
  [/°?K(elvin)?$/i, { category: 'temperature', unit: 'K' }],
  // Pressure
  [/kPa$/i, { category: 'pressure', unit: 'kPa' }],
  [/PSI$/i, { category: 'pressure', unit: 'PSI' }],
  [/bar$/i, { category: 'pressure', unit: 'bar' }],
  [/inHg$/i, { category: 'pressure', unit: 'inHg' }],
  // AFR
  [/AFR$/i, { category: 'afr', unit: 'AFR' }],
  [/lambda|λ$/i, { category: 'afr', unit: 'Lambda' }],
  // Speed
  [/km\/?h$/i, { category: 'speed', unit: 'km/h' }],
  [/mph$/i, { category: 'speed', unit: 'mph' }],
];

export function detectUnitType(unitString: string): UnitType | null {
  for (const [pattern, unitType] of UNIT_PATTERNS) {
    if (pattern.test(unitString)) {
      return unitType;
    }
  }
  return null;
}

/**
 * Convert a value from its original unit to the user's preferred unit
 */
export function convertToPreferred(
  value: number,
  originalUnit: string,
  preferences: UnitPreferences
): { value: number; displayUnit: string } {
  const unitType = detectUnitType(originalUnit);
  
  if (!unitType) {
    return { value, displayUnit: originalUnit };
  }
  
  switch (unitType.category) {
    case 'temperature': {
      const preferredUnit = preferences.temperature;
      const converted = convertTemperature(value, unitType.unit, preferredUnit);
      return { value: converted, displayUnit: UNIT_LABELS[preferredUnit] };
    }
    case 'pressure': {
      const preferredUnit = preferences.pressure;
      const converted = convertPressure(value, unitType.unit, preferredUnit);
      return { value: converted, displayUnit: UNIT_LABELS[preferredUnit] };
    }
    case 'afr': {
      const preferredUnit = preferences.afr;
      const stoichAfr = STOICH_AFR[preferences.fuelType];
      const converted = convertAfr(value, unitType.unit, preferredUnit, stoichAfr);
      return { value: converted, displayUnit: UNIT_LABELS[preferredUnit] };
    }
    case 'speed': {
      const preferredUnit = preferences.speed;
      const converted = convertSpeed(value, unitType.unit, preferredUnit);
      return { value: converted, displayUnit: UNIT_LABELS[preferredUnit] };
    }
    default:
      return { value, displayUnit: originalUnit };
  }
}
