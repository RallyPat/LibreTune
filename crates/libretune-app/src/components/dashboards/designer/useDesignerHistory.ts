import { useState, useCallback } from 'react';
import { DashFile, DashComponent, isGauge, isIndicator } from '../dashTypes';

interface HistoryEntry {
  dashFile: DashFile;
  description: string;
}

interface Options {
  dashFile: DashFile;
  selectedGaugeId: string | null;
  onDashFileChange: (file: DashFile) => void;
  onSelectGauge: (id: string | null) => void;
}

/**
 * Designer history (undo/redo) plus delete/copy/paste of the selected component.
 * Extracted from DashboardDesigner during Phase D.
 */
export function useDesignerHistory({
  dashFile,
  selectedGaugeId,
  onDashFileChange,
  onSelectGauge,
}: Options) {
  const [history, setHistory] = useState<HistoryEntry[]>([
    { dashFile, description: 'Initial' },
  ]);
  const [historyIndex, setHistoryIndex] = useState(0);
  const [clipboard, setClipboard] = useState<DashComponent | null>(null);

  const selectedComponent = selectedGaugeId
    ? dashFile.gauge_cluster.components.find((c) => {
        if (isGauge(c)) return c.Gauge.id === selectedGaugeId;
        if (isIndicator(c)) return c.Indicator.id === selectedGaugeId;
        return false;
      })
    : null;

  const pushHistory = useCallback(
    (newFile: DashFile, description: string) => {
      setHistory((prev) => {
        const newHistory = prev.slice(0, historyIndex + 1);
        newHistory.push({ dashFile: newFile, description });
        return newHistory;
      });
      setHistoryIndex((prev) => prev + 1);
    },
    [historyIndex],
  );

  const undo = useCallback(() => {
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      onDashFileChange(history[newIndex].dashFile);
    }
  }, [historyIndex, history, onDashFileChange]);

  const redo = useCallback(() => {
    if (historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      onDashFileChange(history[newIndex].dashFile);
    }
  }, [historyIndex, history, onDashFileChange]);

  const remove = useCallback(() => {
    if (!selectedGaugeId) return;
    const newComponents = dashFile.gauge_cluster.components.filter((c) => {
      if (isGauge(c)) return c.Gauge.id !== selectedGaugeId;
      if (isIndicator(c)) return c.Indicator.id !== selectedGaugeId;
      return true;
    });
    const newFile: DashFile = {
      ...dashFile,
      gauge_cluster: { ...dashFile.gauge_cluster, components: newComponents },
    };
    pushHistory(newFile, `Delete ${selectedGaugeId}`);
    onDashFileChange(newFile);
    onSelectGauge(null);
  }, [selectedGaugeId, dashFile, pushHistory, onDashFileChange, onSelectGauge]);

  const copy = useCallback(() => {
    if (!selectedComponent) return;
    setClipboard(JSON.parse(JSON.stringify(selectedComponent)));
  }, [selectedComponent]);

  const paste = useCallback(() => {
    if (!clipboard) return;

    let newComponent: DashComponent;
    if (isGauge(clipboard)) {
      const gauge = clipboard.Gauge;
      newComponent = {
        Gauge: {
          ...gauge,
          id: `gauge-${Date.now()}`,
          relative_x: (gauge.relative_x ?? 0) + 0.05,
          relative_y: (gauge.relative_y ?? 0) + 0.05,
        },
      };
    } else if (isIndicator(clipboard)) {
      const indicator = clipboard.Indicator;
      newComponent = {
        Indicator: {
          ...indicator,
          id: `indicator-${Date.now()}`,
          relative_x: (indicator.relative_x ?? 0) + 0.05,
          relative_y: (indicator.relative_y ?? 0) + 0.05,
        },
      };
    } else {
      return;
    }

    const newFile: DashFile = {
      ...dashFile,
      gauge_cluster: {
        ...dashFile.gauge_cluster,
        components: [...dashFile.gauge_cluster.components, newComponent],
      },
    };

    pushHistory(newFile, 'Paste gauge');
    onDashFileChange(newFile);
  }, [clipboard, dashFile, pushHistory, onDashFileChange]);

  return {
    selectedComponent,
    pushHistory,
    undo,
    redo,
    remove,
    copy,
    paste,
    canUndo: historyIndex > 0,
    canRedo: historyIndex < history.length - 1,
    hasClipboard: clipboard !== null,
  };
}
