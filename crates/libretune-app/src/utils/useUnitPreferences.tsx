/**
 * Unit Preferences Hook
 * 
 * React hook for managing user unit preferences and providing conversion utilities.
 * Persists preferences to localStorage and provides context for the entire app.
 */

import { useState, useEffect, useCallback, useMemo, createContext, useContext, ReactNode } from 'react';
import {
  UnitPreferences,
  DEFAULT_PREFERENCES,
  US_PREFERENCES,
  convertToPreferred,
  convertTemperature,
  convertPressure,
  convertAfr,
  convertSpeed,
  TemperatureUnit,
  PressureUnit,
  AfrUnit,
  SpeedUnit,
  STOICH_AFR,
  getUnitLabel,
} from './unitConversions';

const STORAGE_KEY = 'libretune-unit-preferences';

export interface UnitConversionContext {
  preferences: UnitPreferences;
  updatePreference: <K extends keyof UnitPreferences>(key: K, value: UnitPreferences[K]) => void;
  resetToDefaults: () => void;
  useUSUnits: () => void;
  useMetricUnits: () => void;
  
  // Convenience conversion methods
  convertTemp: (value: number, fromUnit: TemperatureUnit) => number;
  convertPress: (value: number, fromUnit: PressureUnit) => number;
  convertAFR: (value: number, fromUnit: AfrUnit) => number;
  convertSpd: (value: number, fromUnit: SpeedUnit) => number;
  
  // Auto-detect and convert
  autoConvert: (value: number, originalUnit: string) => { value: number; displayUnit: string };
  
  // Get display label for preferred unit
  getPreferredTempLabel: () => string;
  getPreferredPressureLabel: () => string;
  getPreferredAfrLabel: () => string;
  getPreferredSpeedLabel: () => string;
}

const UnitContext = createContext<UnitConversionContext | null>(null);

export function UnitPreferencesProvider({ children }: { children: ReactNode }) {
  const [preferences, setPreferences] = useState<UnitPreferences>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        return { ...DEFAULT_PREFERENCES, ...JSON.parse(stored) };
      }
    } catch {
      // Ignore localStorage errors
    }
    return DEFAULT_PREFERENCES;
  });

  // Persist preferences to localStorage
  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
    } catch {
      // Ignore localStorage errors
    }
  }, [preferences]);

  const updatePreference = useCallback(<K extends keyof UnitPreferences>(
    key: K,
    value: UnitPreferences[K]
  ) => {
    setPreferences(prev => ({ ...prev, [key]: value }));
  }, []);

  const resetToDefaults = useCallback(() => {
    setPreferences(DEFAULT_PREFERENCES);
  }, []);

  const useUSUnits = useCallback(() => {
    setPreferences(US_PREFERENCES);
  }, []);

  const useMetricUnits = useCallback(() => {
    setPreferences(DEFAULT_PREFERENCES);
  }, []);

  const convertTemp = useCallback((value: number, fromUnit: TemperatureUnit) => {
    return convertTemperature(value, fromUnit, preferences.temperature);
  }, [preferences.temperature]);

  const convertPress = useCallback((value: number, fromUnit: PressureUnit) => {
    return convertPressure(value, fromUnit, preferences.pressure);
  }, [preferences.pressure]);

  const convertAFR = useCallback((value: number, fromUnit: AfrUnit) => {
    const stoich = STOICH_AFR[preferences.fuelType];
    return convertAfr(value, fromUnit, preferences.afr, stoich);
  }, [preferences.afr, preferences.fuelType]);

  const convertSpd = useCallback((value: number, fromUnit: SpeedUnit) => {
    return convertSpeed(value, fromUnit, preferences.speed);
  }, [preferences.speed]);

  const autoConvert = useCallback((value: number, originalUnit: string) => {
    return convertToPreferred(value, originalUnit, preferences);
  }, [preferences]);

  const getPreferredTempLabel = useCallback(() => {
    return getUnitLabel(preferences.temperature);
  }, [preferences.temperature]);

  const getPreferredPressureLabel = useCallback(() => {
    return getUnitLabel(preferences.pressure);
  }, [preferences.pressure]);

  const getPreferredAfrLabel = useCallback(() => {
    return getUnitLabel(preferences.afr);
  }, [preferences.afr]);

  const getPreferredSpeedLabel = useCallback(() => {
    return getUnitLabel(preferences.speed);
  }, [preferences.speed]);

  const contextValue = useMemo<UnitConversionContext>(() => ({
    preferences,
    updatePreference,
    resetToDefaults,
    useUSUnits,
    useMetricUnits,
    convertTemp,
    convertPress,
    convertAFR,
    convertSpd,
    autoConvert,
    getPreferredTempLabel,
    getPreferredPressureLabel,
    getPreferredAfrLabel,
    getPreferredSpeedLabel,
  }), [
    preferences,
    updatePreference,
    resetToDefaults,
    useUSUnits,
    useMetricUnits,
    convertTemp,
    convertPress,
    convertAFR,
    convertSpd,
    autoConvert,
    getPreferredTempLabel,
    getPreferredPressureLabel,
    getPreferredAfrLabel,
    getPreferredSpeedLabel,
  ]);

  return (
    <UnitContext.Provider value={contextValue}>
      {children}
    </UnitContext.Provider>
  );
}

export function useUnitPreferences(): UnitConversionContext {
  const context = useContext(UnitContext);
  if (!context) {
    throw new Error('useUnitPreferences must be used within a UnitPreferencesProvider');
  }
  return context;
}

/**
 * Standalone hook for reading unit preferences without the provider
 * (useful for components that may be rendered outside the provider)
 */
export function useLocalUnitPreferences() {
  const [preferences, setPreferences] = useState<UnitPreferences>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        return { ...DEFAULT_PREFERENCES, ...JSON.parse(stored) };
      }
    } catch {
      // Ignore
    }
    return DEFAULT_PREFERENCES;
  });

  const updatePreference = useCallback(<K extends keyof UnitPreferences>(
    key: K,
    value: UnitPreferences[K]
  ) => {
    setPreferences(prev => {
      const updated = { ...prev, [key]: value };
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(updated));
      } catch {
        // Ignore
      }
      return updated;
    });
  }, []);

  return { preferences, updatePreference };
}
