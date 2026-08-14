/**
 * Shared INI channel metadata type + loader.
 *
 * `get_available_channels` returns one entry per output channel in the
 * loaded INI. Several components need it keyed by name (gauge creation,
 * designer drops, range sync) — previously each file declared its own
 * slightly-different copy of the interface.
 */

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface ChannelInfo {
  name: string;
  label?: string | null;
  units: string;
  scale: number;
  translate: number;
}

export type ChannelInfoMap = Record<string, ChannelInfo>;

/** Fetch the channel list once on mount; empty map on failure. */
export function useChannelInfoMap(): ChannelInfoMap {
  const [map, setMap] = useState<ChannelInfoMap>({});

  useEffect(() => {
    let cancelled = false;
    invoke<ChannelInfo[]>('get_available_channels')
      .then((channels) => {
        if (cancelled) return;
        const next: ChannelInfoMap = {};
        for (const ch of channels ?? []) {
          next[ch.name] = ch;
        }
        setMap(next);
      })
      .catch((e) => {
        console.warn('[useChannelInfoMap] get_available_channels failed:', e);
        if (!cancelled) setMap({});
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return map;
}
