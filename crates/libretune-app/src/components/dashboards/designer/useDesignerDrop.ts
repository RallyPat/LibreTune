import { useCallback } from 'react';
import { DashFile, TsGaugeConfig } from '../dashTypes';

interface ChannelInfo {
  units?: string | null;
  label?: string | null;
  scale: number;
  translate: number;
}

/**
 * Build a default `TsGaugeConfig` populated from a channel drop event,
 * using INI-derived metadata when available.
 */
export function createDefaultGaugeFromChannel(
  channelId: string,
  channelLabel: string,
  info: ChannelInfo | undefined,
  relX: number,
  relY: number,
): TsGaugeConfig {
  const units = info?.units || '';
  const label = info?.label || channelLabel;
  const minVal = info ? Math.min(0, info.translate) : 0;
  const maxVal = info ? Math.max(100, info.translate + (100 * info.scale)) : 100;

  return {
    id: `gauge_${Date.now()}`,
    gauge_painter: 'BasicReadout',
    gauge_style: '',
    output_channel: channelId,
    title: label || channelLabel,
    units,
    value: 0,
    min: minVal,
    max: maxVal,
    min_vp: null,
    max_vp: null,
    default_min: null,
    default_max: null,
    peg_limits: false,
    low_warning: null,
    high_warning: null,
    low_critical: null,
    high_critical: null,
    low_warning_vp: null,
    high_warning_vp: null,
    low_critical_vp: null,
    high_critical_vp: null,
    back_color: { alpha: 0, red: 40, green: 40, blue: 40 },
    font_color: { alpha: 0, red: 255, green: 255, blue: 255 },
    trim_color: { alpha: 0, red: 100, green: 100, blue: 100 },
    warn_color: { alpha: 0, red: 255, green: 165, blue: 0 },
    critical_color: { alpha: 0, red: 255, green: 0, blue: 0 },
    needle_color: { alpha: 0, red: 200, green: 200, blue: 200 },
    value_digits: 2,
    label_digits: 0,
    font_family: 'Arial',
    font_size_adjustment: 0,
    italic_font: false,
    sweep_angle: 270,
    start_angle: 225,
    face_angle: 0,
    sweep_begin_degree: 0,
    counter_clockwise: false,
    major_ticks: 5,
    minor_ticks: 0,
    relative_x: relX,
    relative_y: relY,
    relative_width: 0.2,
    relative_height: 0.2,
    border_width: 1,
    shortest_size: 0,
    shape_locked_to_aspect: false,
    antialiasing_on: true,
    background_image_file_name: null,
    needle_image_file_name: null,
    show_history: false,
    history_value: 0,
    history_delay: 0,
    needle_smoothing: 0,
    short_click_action: null,
    long_click_action: null,
    display_value_at_180: false,
  };
}

interface UseDesignerDropArgs {
  dashFile: DashFile;
  gridSnap: number;
  snapToGrid: (v: number) => number;
  channelInfoMap: Record<string, ChannelInfo>;
  pushHistory: (file: DashFile, action: string) => void;
  onDashFileChange: (file: DashFile) => void;
}

/**
 * Drag/drop handlers for accepting channel drops from the channel panel
 * onto the designer canvas. Adds a new gauge at the drop location.
 */
export function useDesignerDrop({
  dashFile,
  gridSnap,
  snapToGrid,
  channelInfoMap,
  pushHistory,
  onDashFileChange,
}: UseDesignerDropArgs) {
  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'copy';
    e.currentTarget.classList.add('drag-over-dropzone');
  }, []);

  const onDragLeave = useCallback((e: React.DragEvent) => {
    e.currentTarget.classList.remove('drag-over-dropzone');
  }, []);

  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.classList.remove('drag-over-dropzone');

    try {
      const data = e.dataTransfer.getData('application/json');
      if (!data) return;

      const payload = JSON.parse(data);
      if (!dashFile) return;

      const rect = e.currentTarget.getBoundingClientRect();
      let relX = (e.clientX - rect.left) / rect.width;
      let relY = (e.clientY - rect.top) / rect.height;

      // Offset to roughly center on cursor
      relX = Math.max(0, Math.min(0.9, relX - 0.1));
      relY = Math.max(0, Math.min(0.9, relY - 0.1));

      if (gridSnap > 0) {
        relX = snapToGrid(relX);
        relY = snapToGrid(relY);
      }

      let gauge: TsGaugeConfig;
      let actionLabel: string;

      if (payload.type === 'painter') {
        // Plan v2 / D-7c: dropping a painter palette tile creates a new
        // placeholder gauge of that type bound to no channel yet.
        gauge = createDefaultGaugeFromChannel('', payload.label || payload.painter, undefined, relX, relY);
        gauge.gauge_painter = payload.painter;
        gauge.title = payload.label || payload.painter;
        actionLabel = `Add ${payload.painter}`;
      } else if (payload.type === 'channel') {
        const info = channelInfoMap[payload.id];
        gauge = createDefaultGaugeFromChannel(
          payload.id,
          payload.label,
          info,
          relX,
          relY,
        );
        actionLabel = `Add gauge from ${payload.label}`;
      } else {
        return;
      }

      const updatedComponents = [
        ...dashFile.gauge_cluster.components,
        { Gauge: gauge },
      ];
      const updatedFile: DashFile = {
        ...dashFile,
        gauge_cluster: { ...dashFile.gauge_cluster, components: updatedComponents },
      };

      pushHistory(updatedFile, actionLabel);
      onDashFileChange(updatedFile);
    } catch (err) {
      console.error('Drop handler error:', err);
    }
  }, [dashFile, gridSnap, snapToGrid, channelInfoMap, pushHistory, onDashFileChange]);

  return { onDragOver, onDragLeave, onDrop };
}
