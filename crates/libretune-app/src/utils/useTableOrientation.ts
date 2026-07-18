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

/** Apply user cursor/trail colors as root CSS vars (call once in App).
 *  Empty settings leave the theme defaults untouched. */
export function useTableAccentColorVars(): void {
  useEffect(() => {
    const apply = (cursor?: string, trail?: string) => {
      const root = document.documentElement.style;
      if (cursor) root.setProperty('--cursor-live', cursor);
      else root.removeProperty('--cursor-live');
      if (trail) root.setProperty('--cursor-trail', trail);
      else root.removeProperty('--cursor-trail');
    };
    const load = () => {
      invoke<{ table_cursor_color?: string; table_trail_color?: string }>('get_settings')
        .then((s) => apply(s?.table_cursor_color, s?.table_trail_color))
        .catch(() => {});
    };
    load();
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<string>('settings:changed', (e) => {
          if (e.payload === 'table_cursor_color' || e.payload === 'table_trail_color') load();
        });
      } catch {
        // Not running under Tauri
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);
}

/** Trail point lifetime in seconds (0 = never expire) */
export function useTrailFadeSec(): number {
  const [sec, setSec] = useState(8);
  useEffect(() => {
    const load = () => {
      invoke<{ table_trail_fade_sec?: number }>('get_settings')
        .then((s) => setSec(s?.table_trail_fade_sec ?? 8))
        .catch(() => {});
    };
    load();
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<string>('settings:changed', (e) => {
          if (e.payload === 'table_trail_fade_sec') load();
        });
      } catch {
        // Not running under Tauri
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);
  return sec;
}

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
