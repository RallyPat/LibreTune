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
  /** Channel history for strip chart visualization (last 60 seconds, max 300 points) */
  channelHistory: Record<string, number[]>;
  
  // Actions
  /** Update all channels with new data (called from event listener) */
  updateChannels: (data: Record<string, number>) => void;
  /** Clear all channel data */
  clearChannels: () => void;
  /** Get channel history, creating if needed */
  getChannelHistory: (channelName: string) => number[];
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
  subscribeWithSelector((set, get) => ({
    channels: {},
    lastUpdateTime: 0,
    channelHistory: {},
    
    updateChannels: (data) => {
      const state = get();
      const newHistory = { ...state.channelHistory };
      
      // Update history for each channel (max 300 points, ~60s at 5Hz)
      for (const [name, value] of Object.entries(data)) {
        if (!newHistory[name]) {
          newHistory[name] = [];
        }
        const history = newHistory[name];
        history.push(value);
        // Keep only last 300 points (60 seconds at 5 Hz, or 120 seconds at 2.5 Hz)
        if (history.length > 300) {
          history.shift();
        }
      }
      
      set({ 
        channels: data,
        channelHistory: newHistory,
        lastUpdateTime: Date.now() 
      });
    },
    
    clearChannels: () => set({
      channels: {},
      channelHistory: {},
      lastUpdateTime: 0
    }),
    
    getChannelHistory: (channelName: string) => {
      const state = get();
      return state.channelHistory[channelName] ?? [];
    },
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
 * Hook to get channel history for strip chart visualization.
 * Returns history array (up to 300 points, ~60s at 5Hz).
 * Component only re-renders when THIS specific channel's history changes.
 * 
 * @param name - Channel name (e.g., 'rpm', 'afr', 'map')
 * @returns History array (empty array if no history available)
 * 
 * @example
 * function RealtimeTrendChart() {
 *   const history = useChannelHistory('rpm');
 *   return <LineChart data={history} />;
 * }
 */
export const useChannelHistory = (name: string): number[] =>
  useRealtimeStore((state) => state.channelHistory[name] ?? []);

/**
 * Hook to get histories for multiple channels.
 * Component re-renders when ANY of the specified channels' histories change.
 * Uses shallow equality comparison for the returned object.
 * 
 * @param names - Array of channel names
 * @returns Object mapping channel names to history arrays
 * 
 * @example
 * function DashboardWithTrends() {
 *   const histories = useChannelHistories(['rpm', 'map', 'afr']);
 *   return (
 *     <div>
 *       {Object.entries(histories).map(([name, history]) => (
 *         <TrendChart key={name} channel={name} data={history} />
 *       ))}
 *     </div>
 *   );
 * }
 */
export const useChannelHistories = (names: string[]): Record<string, number[]> => {
  // Get the full history object from the store
  const channelHistory = useRealtimeStore((state) => state.channelHistory);
  
  // Create a stable key from names array for dependency comparison
  const namesKey = names.join(',');
  
  // Use useMemo to create a stable result object that only changes when values change
  return useMemo(() => {
    const result: Record<string, number[]> = {};
    for (const name of names) {
      const history = channelHistory[name];
      if (history) {
        result[name] = history;
      }
    }
    return result;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channelHistory, namesKey]); // Use namesKey instead of spreading names
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
