/**
 * Table Y-axis orientation setting.
 *
 * When enabled (Settings → "Table Y axis zero at bottom"), table editors
 * render rows so the origin sits at the bottom-left — the lowest load row
 * at the bottom, increasing upward. Display-only: data
 * coordinates, selection, and clipboard behavior are unchanged.
 */
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export function useTableYAxisBottom(): boolean {
  const [bottom, setBottom] = useState(false);

  useEffect(() => {
    let mounted = true;
    let unlisten: UnlistenFn | null = null;

    const load = () => {
      invoke<{ table_y_axis_bottom?: boolean }>('get_settings')
        .then((s) => {
          if (mounted) setBottom(!!s?.table_y_axis_bottom);
        })
        .catch(() => {});
    };

    load();

    (async () => {
      try {
        unlisten = await listen<string>('settings:changed', (event) => {
          if (event.payload === 'table_y_axis_bottom') load();
        });
      } catch {
        // Not running under Tauri (tests) — setting stays at its default.
      }
    })();

    return () => {
      mounted = false;
      if (unlisten) unlisten();
    };
  }, []);

  return bottom;
}
