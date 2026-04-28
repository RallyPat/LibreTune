import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DashFile, TsGaugeConfig, isGauge, isIndicator } from '../dashTypes';
import TsGauge from '../../gauges/TsGauge';
import LiveTsIndicator from './LiveTsIndicator';
import { buildDefaultGauge } from '../utils/defaultGauge';

interface ChannelInfo {
  name: string;
  label?: string | null;
  units: string;
  scale: number;
  translate: number;
}

interface Props {
  dashFile: DashFile;
  selectedPath: string;
  setDashFile: (file: DashFile) => void;
  channelInfoMap: Record<string, ChannelInfo>;
  embeddedImages: Map<string, string>;
  designerMode: boolean;
  legacyMode: boolean;
  scale: number;
  aspectRatio: number;
  bgColor: string;
  backgroundImageLayers: string;
  backgroundSizeLayers: string | undefined;
  backgroundRepeatLayers: string | undefined;
  sweepActive: boolean;
  sweepValues: Record<string, number>;
  gaugeDemoActive: boolean;
  demoValues: Record<string, number>;
  wrapperRef: React.RefObject<HTMLDivElement>;
  onContextMenu: (e: React.MouseEvent, gaugeId: string | null) => void;
}

/**
 * Dashboard rendering surface (scaling wrapper, drop handler, gauge/indicator map).
 * Extracted from TsDashboard during Phase C4.
 */
export default function DashboardCanvas({
  dashFile,
  selectedPath,
  setDashFile,
  channelInfoMap,
  embeddedImages,
  designerMode,
  legacyMode,
  scale,
  aspectRatio,
  bgColor,
  backgroundImageLayers,
  backgroundSizeLayers,
  backgroundRepeatLayers,
  sweepActive,
  sweepValues,
  gaugeDemoActive,
  demoValues,
  wrapperRef,
  onContextMenu,
}: Props) {
  const cluster = dashFile.gauge_cluster;
  const toPercent = (v: number | undefined | null) => (v ?? 0) * 100;

  const handleDrop = (e: React.DragEvent<HTMLDivElement>) => {
    if (!designerMode) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.style.opacity = '1';

    try {
      const data = e.dataTransfer.getData('application/json');
      if (!data) return;

      const channel = JSON.parse(data);
      if (channel.type !== 'channel' || !dashFile) return;

      const rect = e.currentTarget.getBoundingClientRect();
      const relX = (e.clientX - rect.left) / rect.width;
      const relY = (e.clientY - rect.top) / rect.height;

      const info = channelInfoMap[channel.id];
      const units = info?.units || '';
      const label = info?.label || channel.label;

      const defaultGauge: TsGaugeConfig = buildDefaultGauge({
        id: `gauge_${Date.now()}`,
        channel: channel.id,
        title: label || channel.label,
        units,
        relativeX: relX - 0.1,
        relativeY: relY - 0.1,
      });

      const updatedComponents = [...dashFile.gauge_cluster.components, { Gauge: defaultGauge }];
      const updatedFile: DashFile = {
        ...dashFile,
        gauge_cluster: {
          ...dashFile.gauge_cluster,
          components: updatedComponents,
        },
      };
      setDashFile(updatedFile);

      try {
        invoke('save_dash_file', {
          path: selectedPath,
          dashFile: updatedFile,
        }).catch((err) => console.error('Failed to auto-save dashboard:', err));
      } catch (err) {
        console.error('Failed to save dashboard:', err);
      }
    } catch (err) {
      console.error('Failed to process dropped channel:', err);
    }
  };

  return (
    <div ref={wrapperRef} className="ts-dashboard-wrapper">
      <div
        className={`ts-dashboard ${designerMode ? 'designer-mode' : ''}`}
        style={{
          backgroundColor: bgColor,
          backgroundImage: backgroundImageLayers || undefined,
          backgroundSize: backgroundSizeLayers,
          backgroundRepeat: backgroundRepeatLayers,
          backgroundPosition: 'center',
          aspectRatio: `${aspectRatio}`,
          transform: scale < 1 ? `scale(${scale})` : undefined,
          transformOrigin: 'top center',
        }}
        onContextMenu={(e) => onContextMenu(e, null)}
        onDragOver={(e) => {
          if (!designerMode) return;
          e.preventDefault();
          e.stopPropagation();
          e.dataTransfer.dropEffect = 'copy';
          e.currentTarget.style.opacity = '0.8';
        }}
        onDragLeave={(e) => {
          if (!designerMode) return;
          e.currentTarget.style.opacity = '1';
        }}
        onDrop={handleDrop}
      >
        {cluster.components.map((component, index) => {
          if (isGauge(component)) {
            const gauge = component.Gauge;
            const value = sweepActive
              ? (sweepValues[gauge.output_channel] ?? gauge.min)
              : gaugeDemoActive
                ? (demoValues[gauge.output_channel] ?? gauge.value)
                : gauge.value;

            const gaugeStyle: React.CSSProperties = {
              left: `${toPercent(gauge.relative_x)}%`,
              top: `${toPercent(gauge.relative_y)}%`,
              width: `${toPercent(gauge.relative_width)}%`,
              height: `${toPercent(gauge.relative_height)}%`,
              minWidth: !legacyMode && gauge.shortest_size > 0 ? `${gauge.shortest_size}px` : undefined,
              minHeight: !legacyMode && gauge.shortest_size > 0 ? `${gauge.shortest_size}px` : undefined,
              aspectRatio: gauge.shape_locked_to_aspect ? '1 / 1' : undefined,
            };

            return (
              <div
                key={gauge.id || `gauge-${index}`}
                className={`ts-component ts-gauge ${designerMode ? 'editable' : ''}`}
                style={gaugeStyle}
                onContextMenu={(e) => onContextMenu(e, gauge.id)}
              >
                <TsGauge
                  config={gauge}
                  value={value}
                  embeddedImages={embeddedImages}
                  legacyMode={legacyMode}
                  overrideStore={sweepActive || gaugeDemoActive}
                />
              </div>
            );
          }

          if (isIndicator(component)) {
            const indicator = component.Indicator;
            return (
              <div
                key={indicator.id || `indicator-${index}`}
                className={`ts-component ts-indicator ${designerMode ? 'editable' : ''}`}
                style={{
                  left: `${toPercent(indicator.relative_x)}%`,
                  top: `${toPercent(indicator.relative_y)}%`,
                  width: `${toPercent(indicator.relative_width)}%`,
                  height: `${toPercent(indicator.relative_height)}%`,
                }}
                onContextMenu={(e) => onContextMenu(e, indicator.id)}
              >
                <LiveTsIndicator config={indicator} embeddedImages={embeddedImages} />
              </div>
            );
          }

          return null;
        })}
      </div>
    </div>
  );
}
