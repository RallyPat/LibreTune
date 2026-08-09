/**
 * useGaugeRangeSync — keeps gauge ranges in sync with the active INI's
 * GaugeConfigurations table. Listens for `ini:changed`,
 * `definition:loaded`, `definition:changed`, and `settings:changed`
 * events; auto-syncs once on dash load (when enabled) and again on any
 * subsequent INI/definition change.
 *
 * Exposes the manual `syncGaugeRanges()` action and the live
 * `autoSyncEnabled` flag (sourced from app settings).
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { DashFile, isGauge } from '../dashTypes';

interface GaugeInfo {
  name: string;
  channel: string;
  title: string;
  units: string;
  lo: number;
  hi: number;
  low_warning: number;
  high_warning: number;
  low_danger: number;
  high_danger: number;
  digits: number;
}

export function useGaugeRangeSync(
  dashFile: DashFile | null,
  setDashFile: (file: DashFile) => void,
) {
  const [autoSyncEnabled, setAutoSyncEnabled] = useState(true);
  const [syncToken, setSyncToken] = useState(0);
  const initialSyncDoneRef = useRef(false);

  const syncGaugeRanges = useCallback(async () => {
    if (!dashFile) return;
    try {
      const gauges = await invoke<GaugeInfo[]>('get_gauge_configs');
      const byChannel = new Map(gauges.map(g => [g.channel.toLowerCase(), g]));
      const byName = new Map(gauges.map(g => [g.name.toLowerCase(), g]));

      const updatedComponents = dashFile.gauge_cluster.components.map((comp) => {
        if (!isGauge(comp)) return comp;
        const gauge = comp.Gauge;
        const channelKey = (gauge.output_channel || '').toLowerCase();
        const nameKey = (gauge.title || '').toLowerCase();
        const info = byChannel.get(channelKey) || byName.get(nameKey);
        if (!info) return comp;

        // Only adopt the INI range when it's actually usable. Unresolvable
        // INI range expressions arrive as null/NaN (e.g. `hi = {rpmhigh}`
        // with no tune loaded), and a degenerate hi <= lo would peg the
        // gauge; in both cases keep the dashboard's own range.
        const rangeOk =
          Number.isFinite(info.lo) && Number.isFinite(info.hi) && info.hi > info.lo;

        return {
          Gauge: {
            ...gauge,
            title: info.title || gauge.title,
            min: rangeOk ? info.lo : gauge.min,
            max: rangeOk ? info.hi : gauge.max,
            units: info.units,
            low_warning: Number.isFinite(info.low_warning) ? info.low_warning : gauge.low_warning,
            high_warning: Number.isFinite(info.high_warning) ? info.high_warning : gauge.high_warning,
            low_critical: Number.isFinite(info.low_danger) ? info.low_danger : gauge.low_critical,
            high_critical: Number.isFinite(info.high_danger) ? info.high_danger : gauge.high_critical,
            value_digits: Number.isFinite(info.digits) ? info.digits : gauge.value_digits,
          },
        };
      });

      setDashFile({
        ...dashFile,
        gauge_cluster: { ...dashFile.gauge_cluster, components: updatedComponents },
      });
    } catch (e) {
      console.warn('[useGaugeRangeSync] Failed to sync gauge ranges from INI:', e);
    }
  }, [dashFile, setDashFile]);

  // Keep a ref to the latest sync fn / flags so the INI-change effect below can
  // call them WITHOUT listing them as deps. Listing `dashFile`/`syncGaugeRanges`
  // as deps there creates an infinite loop: syncing creates a new `dashFile`
  // object → recreates `syncGaugeRanges` → retriggers the effect → re-sync...
  // (the trigger should be `syncToken` ONLY).
  const syncStateRef = useRef({ syncGaugeRanges, autoSyncEnabled, dashFile });
  useEffect(() => {
    syncStateRef.current = { syncGaugeRanges, autoSyncEnabled, dashFile };
  });

  // Auto-sync once on initial dashboard load
  useEffect(() => {
    if (!dashFile) return;
    if (!autoSyncEnabled) return;
    if (initialSyncDoneRef.current) return;
    initialSyncDoneRef.current = true;
    syncGaugeRanges();
  }, [dashFile, syncGaugeRanges, autoSyncEnabled]);

  // Auto-sync on INI/definition changes. Depends on `syncToken` ONLY (the event
  // counter); reads the latest sync fn/flags/dashFile from the ref so it doesn't
  // re-run every time syncing mutates `dashFile`.
  useEffect(() => {
    if (syncToken === 0) return;
    const { syncGaugeRanges: doSync, autoSyncEnabled: enabled, dashFile: file } = syncStateRef.current;
    if (!file) return;
    if (!enabled) return;
    doSync();
  }, [syncToken]);

  // Load auto-sync preference once
  useEffect(() => {
    invoke<{ auto_sync_gauge_ranges?: boolean }>('get_settings')
      .then((settings) => {
        if (settings.auto_sync_gauge_ranges !== undefined) {
          setAutoSyncEnabled(!!settings.auto_sync_gauge_ranges);
        }
      })
      .catch((e) => console.warn('[useGaugeRangeSync] get_settings failed:', e));
  }, []);

  // Listen for INI / definition / settings changes
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      const subscribe = async (event: string) => {
        try {
          const u = await listen(event, () => setSyncToken((v) => v + 1));
          if (typeof u === 'function') unlisteners.push(u);
        } catch (e) {
          console.warn(`[useGaugeRangeSync] Failed to listen for ${event}:`, e);
        }
      };

      await Promise.all([
        subscribe('ini:changed'),
        subscribe('definition:loaded'),
        subscribe('definition:changed'),
      ]);

      try {
        const u = await listen<string>('settings:changed', (event) => {
          if (event.payload === 'auto_sync_gauge_ranges') {
            invoke<{ auto_sync_gauge_ranges?: boolean }>('get_settings')
              .then((settings) => {
                if (settings.auto_sync_gauge_ranges !== undefined) {
                  setAutoSyncEnabled(!!settings.auto_sync_gauge_ranges);
                }
              })
              .catch((e) => console.warn('[useGaugeRangeSync] get_settings failed:', e));
          }
        });
        if (typeof u === 'function') unlisteners.push(u);
      } catch (e) {
        console.warn('[useGaugeRangeSync] Failed to listen for settings:changed:', e);
      }
    };

    setup();

    return () => {
      unlisteners.forEach((u) => {
        if (typeof u === 'function') u();
      });
    };
  }, []);

  return { syncGaugeRanges, autoSyncEnabled };
}
