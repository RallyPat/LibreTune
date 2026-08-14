/**
 * Zustand store for dashboard state — the single owner of the loaded
 * dashboard file, selection, file-level operations (save/new/rename/…),
 * designer state, and the gauge context menu.
 *
 * Mirrors the realtimeStore pattern (subscribeWithSelector) so components
 * subscribe to exactly the slices they need. Realtime channel data lives
 * in realtimeStore; sweep/demo overrides live in the non-reactive
 * gaugeOverride module — neither ever flows through this store.
 *
 * Pop-out windows each get their own JS context (and therefore their own
 * store instance). Selection changes broadcast a `dashboard:changed`
 * Tauri event so every window loads the same dashboard; each instance
 * ignores events it emitted itself (session-id guard).
 */

import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import type { DashFile, DashFileInfo } from '../components/dashboards/dashTypes';
import {
  dashBaseName,
  dashFileName,
  isLegacyDashPath,
} from '../components/dashboards/shared/dashFilename';

export interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  targetGaugeId: string | null; // null = clicked on background
}

interface DashboardState {
  // Loaded dashboard
  dashFile: DashFile | null;
  selectedPath: string;
  availableDashes: DashFileInfo[];
  loading: boolean;
  error: string | null;
  /** Unsaved edits exist (designer/property edits set this; save clears it). */
  dirty: boolean;

  // Designer / editing UI
  designerMode: boolean;
  selectedGaugeId: string | null;
  gridSnap: number;
  showGrid: boolean;
  demoActive: boolean;
  contextMenu: ContextMenuState;
  /** Manual override of legacy-mode detection (null = derive from path). */
  legacyModeOverride: boolean | null;

  // Lifecycle
  /** Load the dashboard list and resolve the initial selection. Idempotent. */
  init: () => Promise<void>;
  refreshList: () => Promise<DashFileInfo[]>;
  selectDashboard: (path: string, opts?: { fromSync?: boolean }) => void;
  /** Re-read the currently selected dashboard from disk, discarding edits. */
  reloadCurrent: () => Promise<void>;
  save: () => Promise<void>;

  // File operations
  createDashboard: (name: string, template?: string) => Promise<string | null>;
  renameDashboard: (newName: string) => Promise<void>;
  deleteDashboard: () => Promise<void>;
  duplicateDashboard: () => Promise<void>;
  exportDashboard: () => Promise<void>;
  importCompleted: (imported: DashFileInfo[]) => Promise<void>;
  /** Delete every dashboard (including user files) and recreate the built-in defaults. */
  resetToDefaults: () => Promise<void>;

  // In-file edits
  /** Replace the dash file (designer/history writes). Marks dirty. */
  setDashFile: (file: DashFile) => void;

  // Designer / UI setters
  setDesignerMode: (on: boolean) => void;
  setSelectedGaugeId: (id: string | null) => void;
  setGridSnap: (snap: number) => void;
  setShowGrid: (on: boolean) => void;
  setDemoActive: (on: boolean) => void;
  setLegacyModeOverride: (value: boolean | null) => void;
  openContextMenu: (x: number, y: number, gaugeId: string | null) => void;
  closeContextMenu: () => void;
}

/** Identifies this window's store instance in `dashboard:changed` events. */
const SESSION_ID = `dash-${Math.random().toString(36).slice(2, 10)}`;

let syncListenerStarted = false;
function startSyncListener() {
  if (syncListenerStarted) return;
  syncListenerStarted = true;
  try {
    // Defensive like useGaugeRangeSync: a broken/unavailable event API
    // (e.g. reset test mocks) must never break dashboard loading — the
    // dashboards just stay window-local.
    Promise.resolve(
      listen<{ path: string; from: string }>('dashboard:changed', (event) => {
        if (event.payload.from === SESSION_ID) return;
        // Follow selection changes made in other windows (main ↔ pop-out).
        useDashboardStore.getState().selectDashboard(event.payload.path, { fromSync: true });
      }),
    ).catch(() => {});
  } catch {
    // ignore
  }
}

/** Broadcast a selection change to other windows; never throws. */
function broadcastSelectionChange(path: string) {
  try {
    Promise.resolve(emit('dashboard:changed', { path, from: SESSION_ID })).catch(() => {});
  } catch {
    // ignore
  }
}

export const useDashboardStore = create<DashboardState>()(
  subscribeWithSelector((set, get) => ({
    dashFile: null,
    selectedPath: '',
    availableDashes: [],
    loading: true,
    error: null,
    dirty: false,

    designerMode: false,
    selectedGaugeId: null,
    gridSnap: 5,
    showGrid: true,
    demoActive: false,
    contextMenu: { visible: false, x: 0, y: 0, targetGaugeId: null },
    legacyModeOverride: null,

    init: async () => {
      startSyncListener();
      const dashes = await get().refreshList();
      if (get().selectedPath) return;

      // 1. Prefer the last-selected dashboard (persisted in settings).
      try {
        const settings = await invoke<{ selected_dashboard?: string }>('get_settings');
        if (settings.selected_dashboard) {
          const saved = dashes.find((d) => d.name === settings.selected_dashboard);
          if (saved) {
            get().selectDashboard(saved.path);
            return;
          }
        }
      } catch { /* ignore — fall through to defaults */ }

      // 2. Prefer Telemetry Live, then Basic, then any LibreTune default.
      const telemetryDash = dashes.find((d) => d.name === 'Telemetry Live.ltdash.xml');
      if (telemetryDash) {
        get().selectDashboard(telemetryDash.path);
        return;
      }
      const basicDash = dashes.find((d) => d.name === 'Basic.ltdash.xml');
      if (basicDash) {
        get().selectDashboard(basicDash.path);
        return;
      }
      const libreTuneDash = dashes.find((d) => d.category === 'LibreTune');
      get().selectDashboard(libreTuneDash?.path || dashes[0]?.path || '');
    },

    refreshList: async () => {
      try {
        const dashes = await invoke<DashFileInfo[]>('list_available_dashes');
        set({ availableDashes: dashes ?? [] });
        return dashes ?? [];
      } catch (e) {
        console.warn('[dashboardStore] list_available_dashes failed:', e);
        set({ availableDashes: [] });
        return [];
      }
    },

    selectDashboard: (path, opts) => {
      if (get().selectedPath === path) return;
      set({ selectedPath: path, legacyModeOverride: null, error: null });
      void invoke('update_setting', {
        key: 'selected_dashboard',
        value: dashFileName(path),
      }).catch(() => {});
      if (!opts?.fromSync && path) {
        broadcastSelectionChange(path);
      }
      // Load the file.
      const load = async () => {
        if (!path) {
          set({ dashFile: null, loading: false });
          return;
        }
        set({ loading: true });
        try {
          const file = await invoke<DashFile>('get_dash_file', { path });
          set({ dashFile: file, loading: false, dirty: false, error: null });
        } catch (e) {
          set({ error: String(e), loading: false });
        }
      };
      void load();
    },

    reloadCurrent: async () => {
      const path = get().selectedPath;
      if (!path) return;
      try {
        const file = await invoke<DashFile>('get_dash_file', { path });
        set({ dashFile: file, dirty: false });
      } catch (e) {
        console.error('Failed to reload dashboard:', e);
      }
    },

    save: async () => {
      const { dashFile, selectedPath } = get();
      if (!dashFile || !selectedPath) return;
      try {
        await invoke('save_dash_file', { path: selectedPath, dashFile });
        set({ dirty: false });
      } catch (e) {
        console.error('Failed to save dashboard:', e);
      }
    },

    createDashboard: async (name, template = 'basic') => {
      const trimmed = name.trim();
      if (!trimmed) return null;
      try {
        const newPath = await invoke<string>('create_new_dashboard', {
          name: trimmed,
          template,
        });
        await get().refreshList();
        get().selectDashboard(newPath);
        return newPath;
      } catch (e) {
        console.error('Failed to create dashboard:', e);
        return null;
      }
    },

    renameDashboard: async (newName) => {
      const trimmed = newName.trim();
      const { selectedPath } = get();
      if (!trimmed || !selectedPath) return;
      try {
        const newPath = await invoke<string>('rename_dashboard', {
          path: selectedPath,
          newName: trimmed,
        });
        await get().refreshList();
        get().selectDashboard(newPath);
      } catch (e) {
        console.error('Failed to rename dashboard:', e);
      }
    },

    deleteDashboard: async () => {
      const { selectedPath } = get();
      if (!selectedPath) return;
      try {
        await invoke('delete_dashboard', { path: selectedPath });
        const dashes = await get().refreshList();
        if (dashes.length > 0) {
          get().selectDashboard(dashes[0].path);
        } else {
          set({ selectedPath: '', dashFile: null });
        }
      } catch (e) {
        console.error('Failed to delete dashboard:', e);
      }
    },

    duplicateDashboard: async () => {
      const { selectedPath } = get();
      if (!selectedPath) return;
      try {
        const copyName = `${dashBaseName(selectedPath)} (Copy)`;
        const newPath = await invoke<string>('duplicate_dashboard', {
          path: selectedPath,
          newName: copyName,
        });
        await get().refreshList();
        get().selectDashboard(newPath);
      } catch (e) {
        console.error('Failed to duplicate dashboard:', e);
      }
    },

    exportDashboard: async () => {
      const { dashFile, selectedPath } = get();
      if (!dashFile) return;
      try {
        const baseName = dashBaseName(selectedPath) || 'Dashboard';
        const filePath = await saveDialog({
          title: 'Export Dashboard',
          filters: [{ name: 'Dashboard Files', extensions: ['ltdash.xml', 'dash', 'gauge'] }],
          defaultPath: `${baseName}.ltdash.xml`,
        });
        if (filePath) {
          await invoke('export_dashboard', { dashFile, path: filePath });
        }
      } catch (e) {
        console.error('Failed to export dashboard:', e);
      }
    },

    importCompleted: async (imported) => {
      await get().refreshList();
      if (imported.length > 0) {
        get().selectDashboard(imported[0].path);
      }
    },

    resetToDefaults: async () => {
      try {
        await invoke('reset_dashboards_to_defaults');
        const dashes = await get().refreshList();
        const first =
          dashes.find((d) => d.name === 'Telemetry Live.ltdash.xml') ?? dashes[0];
        if (first) {
          get().selectDashboard(first.path);
        } else {
          set({ selectedPath: '', dashFile: null });
        }
      } catch (e) {
        console.error('Failed to reset dashboards to defaults:', e);
      }
    },

    setDashFile: (file) => set({ dashFile: file, dirty: true }),

    setDesignerMode: (on) => set({ designerMode: on, selectedGaugeId: on ? get().selectedGaugeId : null }),
    setSelectedGaugeId: (id) => set({ selectedGaugeId: id }),
    setGridSnap: (snap) => set({ gridSnap: snap }),
    setShowGrid: (on) => set({ showGrid: on }),
    setDemoActive: (on) => set({ demoActive: on }),
    setLegacyModeOverride: (value) => set({ legacyModeOverride: value }),
    openContextMenu: (x, y, gaugeId) =>
      set({ contextMenu: { visible: true, x, y, targetGaugeId: gaugeId } }),
    closeContextMenu: () =>
      set((state) => ({ contextMenu: { ...state.contextMenu, visible: false } })),
  })),
);

/** Derived: legacy mode = manual override, else inferred from the file path. */
export function selectLegacyMode(state: DashboardState): boolean {
  if (state.legacyModeOverride !== null) return state.legacyModeOverride;
  return isLegacyDashPath(state.selectedPath);
}
