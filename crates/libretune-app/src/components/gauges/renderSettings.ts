/**
 * `renderSettings` — imperative view of the two dashboard rendering settings
 * (issue #82):
 *
 *  - `dashboard_refresh_hz`  — per-gauge canvas redraw cap (10/15/20/25/30)
 *  - `gauge_right_align_values` — right-justify numeric value text so that
 *    digit-count / sign changes don't shift glyphs
 *
 * The gauge rAF loop and the pure painter functions are not React — they read
 * this module directly instead of going through props or state. `init()`
 * performs the initial `get_settings` fetch and then keeps the values current
 * via the `settings:changed` event, so changes apply live without an app
 * restart. When not running under Tauri (unit tests), the defaults apply and
 * tests can override them with `setForTest()`.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export const ALLOWED_REFRESH_HZ = [10, 15, 20, 25, 30] as const;
export const DEFAULT_REFRESH_HZ = 30;

interface RenderSettings {
  refreshHz: number;
  rightAlignValues: boolean;
}

const state: RenderSettings = {
  refreshHz: DEFAULT_REFRESH_HZ,
  rightAlignValues: false,
};

/** Current per-gauge redraw cap in Hz. */
export function getRefreshHz(): number {
  return state.refreshHz;
}

/** Current per-gauge redraw interval in ms (derived from `getRefreshHz()`). */
export function getDrawIntervalMs(): number {
  return 1000 / state.refreshHz;
}

/** Whether numeric value text should be right-aligned in a fixed region. */
export function getRightAlignValues(): boolean {
  return state.rightAlignValues;
}

function apply(settings: {
  dashboard_refresh_hz?: number;
  gauge_right_align_values?: boolean;
}): void {
  const hz = settings.dashboard_refresh_hz;
  if (typeof hz === 'number' && (ALLOWED_REFRESH_HZ as readonly number[]).includes(hz)) {
    state.refreshHz = hz;
  }
  if (settings.gauge_right_align_values !== undefined) {
    state.rightAlignValues = !!settings.gauge_right_align_values;
  }
}

let initPromise: Promise<void> | null = null;

/**
 * Load current settings and subscribe to `settings:changed`. Idempotent —
 * safe to call from App startup and again from pop-out windows.
 */
export function initRenderSettings(): Promise<void> {
  if (initPromise) return initPromise;
  initPromise = (async () => {
    const load = () =>
      invoke<{ dashboard_refresh_hz?: number; gauge_right_align_values?: boolean }>(
        'get_settings',
      )
        .then(apply)
        .catch(() => {
          // Not running under Tauri, or settings unreadable — keep defaults.
        });
    await load();
    try {
      const unlisten: UnlistenFn = await listen<string>('settings:changed', (e) => {
        if (e.payload === 'dashboard_refresh_hz' || e.payload === 'gauge_right_align_values') {
          void load();
        }
      });
      // Listener lives for the app lifetime; nothing to clean up.
      void unlisten;
    } catch {
      // Not running under Tauri (tests) — defaults stay in effect.
    }
  })();
  return initPromise;
}

/** Test hook: override values directly (no Tauri). */
export function setForTest(patch: Partial<RenderSettings>): void {
  if (patch.refreshHz !== undefined) state.refreshHz = patch.refreshHz;
  if (patch.rightAlignValues !== undefined) state.rightAlignValues = patch.rightAlignValues;
}

/** Test hook: restore defaults. */
export function resetForTest(): void {
  state.refreshHz = DEFAULT_REFRESH_HZ;
  state.rightAlignValues = false;
}
