/**
 * Heatmap Settings Hook
 *
 * React hook that loads heatmap settings from the backend
 * and provides context-aware color getter functions.
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  HeatmapScheme,
  HeatmapContext,
  HeatmapSettings,
  DEFAULT_CONTEXT_SCHEMES,
  valueToHeatmapColor,
  getHeatmapGradientCSS,
  getSchemeStops,
  getAvailableSchemes,
} from '../utils/heatmapColors';

// Backend settings structure (matches Rust Settings struct)
interface BackendSettings {
  heatmap_value_scheme?: string;
  heatmap_change_scheme?: string;
  heatmap_coverage_scheme?: string;
  heatmap_value_custom?: string[];
  heatmap_change_custom?: string[];
  heatmap_coverage_custom?: string[];
}

// Hook return type
interface UseHeatmapSettingsReturn {
  /** Current settings */
  settings: HeatmapSettings;
  /** Loading state */
  loading: boolean;
  /** Error message if loading failed */
  error: string | null;
  /** Get heatmap color for a value in a specific context */
  getColor: (value: number, min: number, max: number, context: HeatmapContext) => string;
  /** Get CSS gradient string for a context */
  getGradient: (context: HeatmapContext, direction?: string) => string;
  /** Get color stops for a context */
  getStops: (context: HeatmapContext) => string[];
  /** Update scheme for a context */
  setScheme: (context: HeatmapContext, scheme: HeatmapScheme) => Promise<void>;
  /** Update custom color stops for a context */
  setCustomStops: (context: HeatmapContext, stops: string[]) => Promise<void>;
  /** Apply same scheme to all contexts */
  setAllSchemes: (scheme: HeatmapScheme) => Promise<void>;
  /** Available scheme options */
  availableSchemes: ReturnType<typeof getAvailableSchemes>;
  /** Reload settings from backend */
  reload: () => Promise<void>;
}

/**
 * Hook to manage heatmap color settings
 */
export function useHeatmapSettings(): UseHeatmapSettingsReturn {
  const [settings, setSettings] = useState<HeatmapSettings>({
    valueScheme: DEFAULT_CONTEXT_SCHEMES.value,
    changeScheme: DEFAULT_CONTEXT_SCHEMES.change,
    coverageScheme: DEFAULT_CONTEXT_SCHEMES.coverage,
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Load settings from backend
  const loadSettings = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const backendSettings = await invoke<BackendSettings>('get_settings');
      
      setSettings({
        valueScheme: (backendSettings.heatmap_value_scheme as HeatmapScheme) || 'tunerstudio',
        changeScheme: (backendSettings.heatmap_change_scheme as HeatmapScheme) || 'tunerstudio',
        coverageScheme: (backendSettings.heatmap_coverage_scheme as HeatmapScheme) || 'tunerstudio',
        customValueStops: backendSettings.heatmap_value_custom,
        customChangeStops: backendSettings.heatmap_change_custom,
        customCoverageStops: backendSettings.heatmap_coverage_custom,
      });
    } catch (err) {
      console.error('Failed to load heatmap settings:', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  // Load on mount
  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // Get effective scheme/stops for a context
  const getEffectiveScheme = useCallback((context: HeatmapContext): HeatmapScheme | string[] => {
    switch (context) {
      case 'value':
        if (settings.valueScheme === 'custom' && settings.customValueStops?.length) {
          return settings.customValueStops;
        }
        return settings.valueScheme;
      case 'change':
        if (settings.changeScheme === 'custom' && settings.customChangeStops?.length) {
          return settings.customChangeStops;
        }
        return settings.changeScheme;
      case 'coverage':
        if (settings.coverageScheme === 'custom' && settings.customCoverageStops?.length) {
          return settings.customCoverageStops;
        }
        return settings.coverageScheme;
    }
  }, [settings]);

  // Get color for a value
  const getColor = useCallback((value: number, min: number, max: number, context: HeatmapContext): string => {
    const scheme = getEffectiveScheme(context);
    return valueToHeatmapColor(value, min, max, scheme);
  }, [getEffectiveScheme]);

  // Get CSS gradient
  const getGradient = useCallback((context: HeatmapContext, direction: string = 'to right'): string => {
    const scheme = getEffectiveScheme(context);
    return getHeatmapGradientCSS(scheme, direction as 'to right' | 'to left' | 'to top' | 'to bottom');
  }, [getEffectiveScheme]);

  // Get color stops
  const getStops = useCallback((context: HeatmapContext): string[] => {
    const scheme = getEffectiveScheme(context);
    return getSchemeStops(scheme);
  }, [getEffectiveScheme]);

  // Update scheme for a context
  const setScheme = useCallback(async (context: HeatmapContext, scheme: HeatmapScheme): Promise<void> => {
    const key = `heatmap_${context}_scheme`;
    await invoke('update_setting', { key, value: scheme });
    
    // Update local state
    setSettings(prev => {
      switch (context) {
        case 'value':
          return { ...prev, valueScheme: scheme };
        case 'change':
          return { ...prev, changeScheme: scheme };
        case 'coverage':
          return { ...prev, coverageScheme: scheme };
      }
    });
  }, []);

  // Update custom stops for a context
  const setCustomStops = useCallback(async (context: HeatmapContext, stops: string[]): Promise<void> => {
    await invoke('update_heatmap_custom_stops', { context, stops });
    
    // Update local state
    setSettings(prev => {
      switch (context) {
        case 'value':
          return { ...prev, customValueStops: stops };
        case 'change':
          return { ...prev, customChangeStops: stops };
        case 'coverage':
          return { ...prev, customCoverageStops: stops };
      }
    });
  }, []);

  // Apply same scheme to all contexts
  const setAllSchemes = useCallback(async (scheme: HeatmapScheme): Promise<void> => {
    await Promise.all([
      invoke('update_setting', { key: 'heatmap_value_scheme', value: scheme }),
      invoke('update_setting', { key: 'heatmap_change_scheme', value: scheme }),
      invoke('update_setting', { key: 'heatmap_coverage_scheme', value: scheme }),
    ]);
    
    setSettings(prev => ({
      ...prev,
      valueScheme: scheme,
      changeScheme: scheme,
      coverageScheme: scheme,
    }));
  }, []);

  // Memoize available schemes
  const availableSchemes = useMemo(() => getAvailableSchemes(), []);

  return {
    settings,
    loading,
    error,
    getColor,
    getGradient,
    getStops,
    setScheme,
    setCustomStops,
    setAllSchemes,
    availableSchemes,
    reload: loadSettings,
  };
}

/**
 * Simplified hook that just provides color getter functions
 * without the ability to modify settings.
 * Useful for components that only need to display colors.
 */
export function useHeatmapColors() {
  const { getColor, getGradient, getStops, loading } = useHeatmapSettings();
  return { getColor, getGradient, getStops, loading };
}
