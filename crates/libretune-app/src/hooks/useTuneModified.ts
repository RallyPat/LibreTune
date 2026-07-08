import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface TuneInfo {
  modified: boolean;
  has_tune: boolean;
}

/** Poll backend tune modification state for toolbar burn indicator. */
export function useTuneModified(projectOpen: boolean) {
  const [tuneModified, setTuneModified] = useState(false);

  const refreshTuneModified = useCallback(async () => {
    if (!projectOpen) {
      setTuneModified(false);
      return;
    }
    try {
      const info = await invoke<TuneInfo>('get_tune_info');
      setTuneModified(info.has_tune && info.modified);
    } catch {
      setTuneModified(false);
    }
  }, [projectOpen]);

  useEffect(() => {
    void refreshTuneModified();
    if (!projectOpen) return undefined;

    const interval = window.setInterval(() => {
      void refreshTuneModified();
    }, 1500);

    return () => window.clearInterval(interval);
  }, [projectOpen, refreshTuneModified]);

  return { tuneModified, refreshTuneModified };
}
