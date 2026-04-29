/**
 * useDashboardCRUD — manages the available-dashboards list and the
 * file-level operations (save, new, rename, delete, duplicate, export,
 * reload, import). Returns state and handlers; the caller is responsible
 * for hiding/showing dialogs and updating local form state.
 */

import { useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import type { DashFile, DashFileInfo } from '../dashTypes';

interface UseDashboardCRUDOptions {
  dashFile: DashFile | null;
  selectedPath: string;
  setSelectedPath: (path: string) => void;
  setDashFile: (file: DashFile | null) => void;
}

export function useDashboardCRUD({
  dashFile,
  selectedPath,
  setSelectedPath,
  setDashFile,
}: UseDashboardCRUDOptions) {
  const [availableDashes, setAvailableDashes] = useState<DashFileInfo[]>([]);

  /**
   * Load/refresh available dashboards list. The Rust backend seeds the
   * app data dir with default dashboards (Basic/Tuning/Racing) on first
   * run, so this should always return at least these. If it does come
   * back empty (e.g. seeding failed), we surface an empty list and let
   * the empty-state UI / Reset to Defaults action recover.
   */
  const refreshDashboardList = useCallback(async () => {
    try {
      const dashes = await invoke<DashFileInfo[]>('list_available_dashes');
      setAvailableDashes(dashes ?? []);
      return dashes ?? [];
    } catch (e) {
      console.warn('[useDashboardCRUD] list_available_dashes failed:', e);
      setAvailableDashes([]);
      return [];
    }
  }, []);

  const reloadCurrentDashboard = useCallback(async () => {
    if (!selectedPath) return;
    try {
      const file = await invoke<DashFile>('get_dash_file', { path: selectedPath });
      setDashFile(file);
    } catch (e) {
      console.error('Failed to reload dashboard:', e);
    }
  }, [selectedPath, setDashFile]);

  const saveDashboard = useCallback(async () => {
    if (!dashFile || !selectedPath) return;
    try {
      await invoke('save_dash_file', { path: selectedPath, dashFile });
    } catch (e) {
      console.error('Failed to save dashboard:', e);
    }
  }, [dashFile, selectedPath]);

  const createDashboard = useCallback(async (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    try {
      const newPath = await invoke<string>('create_new_dashboard', {
        name: trimmed,
        template: 'basic',
      });
      await refreshDashboardList();
      setSelectedPath(newPath);
    } catch (e) {
      console.error('Failed to create dashboard:', e);
    }
  }, [refreshDashboardList, setSelectedPath]);

  const renameDashboard = useCallback(async (newName: string) => {
    const trimmed = newName.trim();
    if (!trimmed || !selectedPath) return;
    try {
      const newPath = await invoke<string>('rename_dashboard', {
        path: selectedPath,
        newName: trimmed,
      });
      await refreshDashboardList();
      setSelectedPath(newPath);
    } catch (e) {
      console.error('Failed to rename dashboard:', e);
    }
  }, [selectedPath, refreshDashboardList, setSelectedPath]);

  const deleteDashboard = useCallback(async () => {
    if (!selectedPath) return;
    try {
      await invoke('delete_dashboard', { path: selectedPath });
      const dashes = await refreshDashboardList();
      if (dashes.length > 0) {
        setSelectedPath(dashes[0].path);
      } else {
        setSelectedPath('');
        setDashFile(null);
      }
    } catch (e) {
      console.error('Failed to delete dashboard:', e);
    }
  }, [selectedPath, refreshDashboardList, setSelectedPath, setDashFile]);

  const duplicateDashboard = useCallback(async () => {
    if (!dashFile || !selectedPath) return;
    try {
      const currentName = selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || 'Dashboard';
      const copyName = `${currentName} (Copy)`;
      const newPath = await invoke<string>('duplicate_dashboard', {
        path: selectedPath,
        newName: copyName,
      });
      await refreshDashboardList();
      setSelectedPath(newPath);
    } catch (e) {
      console.error('Failed to duplicate dashboard:', e);
    }
  }, [dashFile, selectedPath, refreshDashboardList, setSelectedPath]);

  const exportDashboard = useCallback(async () => {
    if (!dashFile) return;
    try {
      const currentName = selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || 'Dashboard';
      const filePath = await save({
        title: 'Export Dashboard',
        filters: [{ name: 'Dashboard Files', extensions: ['ltdash.xml', 'dash', 'gauge'] }],
        defaultPath: `${currentName}.ltdash.xml`,
      });
      if (filePath) {
        await invoke('export_dashboard', { dashFile, path: filePath });
      }
    } catch (e) {
      console.error('Failed to export dashboard:', e);
    }
  }, [dashFile, selectedPath]);

  const onImportComplete = useCallback(async (imported: DashFileInfo[]) => {
    await refreshDashboardList();
    if (imported.length > 0) {
      setSelectedPath(imported[0].path);
    }
  }, [refreshDashboardList, setSelectedPath]);

  return {
    availableDashes,
    refreshDashboardList,
    reloadCurrentDashboard,
    saveDashboard,
    createDashboard,
    renameDashboard,
    deleteDashboard,
    duplicateDashboard,
    exportDashboard,
    onImportComplete,
  };
}
