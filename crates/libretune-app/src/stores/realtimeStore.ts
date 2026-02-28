/**
 * Zustand store for realtime ECU data with optimized selectors.
 * 
 * This store uses subscribeWithSelector middleware to enable efficient
 * per-channel subscriptions. Components only re-renders when their
 * specific channel values change, not on every realtime update.
 */
import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import { useShallow } from 'zustand/react/shallow';

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
 * Non-reactive channel history buffer.
 * Stored outside Zustand state so history mutations never trigger React re-renders.
 * Uses circular buffer pattern (fixed-size Float64Array + write index) to avoid
 * Array.shift() which is O(n) per call — with 500 channels that meant 150K element
 * copies per event (3M/sec at 20Hz), enough to freeze the UI.
 *
 * Components that need history (e.g. LineGraph gauges) read this imperatively
 * each animation frame via getChannelHistoryBuffer().
 */
const HISTORY_SIZE = 300;

interface CircularBuffer {
  data: Float64Array;
  /** Next write position (0..HISTORY_SIZE-1, wraps around) */
  writeIdx: number;
  /** Number of valid entries (0..HISTORY_SIZE) */
  count: number;
}

const _channelHistoryBuffer: Record<string, CircularBuffer> = {};

/** Read channel history imperatively (never triggers re-renders).
 *  Returns an array ordered oldest→newest.
 */
export function getChannelHistoryBuffer(channelName: string): number[] {
  const buf = _channelHistoryBuffer[channelName];
  if (!buf || buf.count === 0) return EMPTY_HISTORY;
  // Unroll circular buffer into a plain array (oldest first)
  const result = new Array<number>(buf.count);
  const start = (buf.writeIdx - buf.count + HISTORY_SIZE) % HISTORY_SIZE;
  for (let i = 0; i < buf.count; i++) {
    result[i] = buf.data[(start + i) % HISTORY_SIZE];
  }
  return result;
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
    
    updateChannels: (data) => {
      // Update non-reactive circular history buffer (O(1) per channel, no GC pressure)
      for (const [name, value] of Object.entries(data)) {
        let buf = _channelHistoryBuffer[name];
        if (!buf) {
          buf = { data: new Float64Array(HISTORY_SIZE), writeIdx: 0, count: 0 };
          _channelHistoryBuffer[name] = buf;
        }
        buf.data[buf.writeIdx] = value;
        buf.writeIdx = (buf.writeIdx + 1) % HISTORY_SIZE;
        if (buf.count < HISTORY_SIZE) buf.count++;
      }
      
      // Only reactive state update: channel values + timestamp (lightweight)
      set({ 
        channels: data,
        lastUpdateTime: Date.now() 
      });
    },
    
    clearChannels: () => {
      // Clear non-reactive circular history buffers
      for (const key of Object.keys(_channelHistoryBuffer)) {
        delete _channelHistoryBuffer[key];
      }
      set({
        channels: {},
        lastUpdateTime: 0
      });
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
  // useShallow wraps the selector so Zustand uses shallow comparison on the
  // returned object.  The component only re-renders when a requested channel's
  // numeric VALUE actually changes — not on every store set() call.
  // This is critical because App.tsx calls this hook; without shallow equality
  // the 3500-line App component would re-render 20x/sec and freeze the UI.
  return useRealtimeStore(
    useShallow((state: RealtimeState) => {
      const result: Record<string, number> = {};
      for (const name of names) {
        const value = state.channels[name];
        if (value !== undefined) {
          result[name] = value;
        }
      }
      return result;
    }),
  );
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
const EMPTY_HISTORY: number[] = [];

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
