/**
 * Non-reactive gauge value overrides (startup sweep, gauge demo).
 *
 * Mirrors the realtimeStore's channel-history pattern: module-level state
 * outside React so 20Hz override updates never re-render the dashboard
 * tree. `useGaugeRenderer` consults this before the realtime store each
 * animation frame — no prop threading, no container state.
 */

let active = false;
let values: Record<string, number> = {};

/** Replace the whole override map and/or toggle override mode. */
export function setGaugeOverrides(next: Record<string, number>, isActive: boolean): void {
  values = next;
  active = isActive;
}

export function gaugeOverridesActive(): boolean {
  return active;
}

/** Per-channel override value, or undefined when inactive/absent. */
export function getGaugeOverride(channel: string): number | undefined {
  return active ? values[channel] : undefined;
}
