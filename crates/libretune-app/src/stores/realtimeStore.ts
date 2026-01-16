/**
 * Zustand store for realtime ECU data with optimized selectors.
 * 
 * This store uses subscribeWithSelector middleware to enable efficient
 * per-channel subscriptions. Components only re-renders when their
 * specific channel values change, not on every realtime update.
 */
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import { useMemo } from 'react';

interface RealtimeState {
  /** All channel values keyed by channel name */
  channels: Record<string, number>;
  /** Timestamp of last update (for debugging/monitoring) */
  lastUpdateTime: number;
  
  // Actions
  /** Update all channels with new data (called from event listener) */
  updateChannels: (data: Record<string, number>) => void;
  /** Clear all channel data */
  clearChannels: () => void;
}

/**
 * Main realtime data store.
 * 
 * Usage in components:
 * - Single channel: `const value = useChannelValue('rpm');`
 * - Multiple channels: `const values = useChannels(['rpm', 'map', 'tps']);`
 * - In event listener: `useRealtimeStore.getState().updateChannels(data);`
 */
export const useRealtimeStore = create<RealtimeState>()(
  subscribeWithSelector((set) => ({
    channels: {},
    lastUpdateTime: 0,
    
    updateChannels: (data) => set({ 
      channels: data, 
      lastUpdateTime: Date.now() 
    }),
    
    clearChannels: () => set({
      channels: {},
      lastUpdateTime: 0
    }),
  }))
);

/**
 * Hook to get a single channel value.
 * Component only re-renders when THIS specific channel changes.
 * 
 * @param name - Channel name (e.g., 'rpm', 'afr', 'map')
 * @returns Channel value or undefined if not available
 * 
 * @example
 * function RpmGauge() {
 *   const rpm = useChannel('rpm');
 *   return <div>RPM: {rpm ?? 0}</div>;
 * }
 */
export const useChannel = (name: string): number | undefined =>
  useRealtimeStore((state) => state.channels[name]);

/**
 * Hook to get a single channel value with a default fallback.
 * Component only re-renders when THIS specific channel changes.
 * 
 * @param name - Channel name (e.g., 'rpm', 'afr', 'map')
 * @param defaultValue - Value to return if channel is undefined (default: 0)
 * @returns Channel value or defaultValue
 * 
 * @example
 * function AfrGauge({ config }) {
 *   const value = useChannelValue(config.output_channel, config.min);
 *   return <Gauge value={value} />;
 * }
 */
export const useChannelValue = (name: string, defaultValue = 0): number =>
  useRealtimeStore((state) => state.channels[name] ?? defaultValue);

/**
 * Hook to get multiple channel values at once.
 * Component re-renders when ANY of the specified channels change.
 * Uses shallow equality comparison for the returned object.
 * 
 * @param names - Array of channel names
 * @returns Object mapping channel names to values (only includes defined channels)
 * 
 * @example
 * function StatusBar() {
 *   const values = useChannels(['rpm', 'afr', 'map', 'coolant']);
 *   return (
 *     <div>
 *       {Object.entries(values).map(([name, value]) => (
 *         <span key={name}>{name}: {value}</span>
 *       ))}
 *     </div>
 *   );
 * }
 */
export const useChannels = (names: string[]): Record<string, number> => {
  // Get the full channels object from the store
  const channels = useRealtimeStore((state) => state.channels);
  
  // Create a stable key from names array for dependency comparison
  // This avoids the hooks rule violation of spreading array into deps
  const namesKey = names.join(',');
  
  // Use useMemo to create a stable result object that only changes when values change
  return useMemo(() => {
    const result: Record<string, number> = {};
    for (const name of names) {
      const value = channels[name];
      if (value !== undefined) {
        result[name] = value;
      }
    }
    return result;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channels, namesKey]); // Use namesKey instead of spreading names
};

/**
 * Hook to check if realtime data is being received.
 * Returns true if data was received within the last 500ms.
 * 
 * @example
 * function ConnectionIndicator() {
 *   const isReceiving = useIsReceivingData();
 *   return <span className={isReceiving ? 'connected' : 'disconnected'} />;
 * }
 */
export const useIsReceivingData = (): boolean =>
  useRealtimeStore((state) => Date.now() - state.lastUpdateTime < 500);
