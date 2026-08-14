import React from 'react';
import { DashFile, GaugeCluster, isGauge, isIndicator } from '../dashTypes';
import DashComponentView from './DashComponentView';
import { useEnabledCondition } from '../hooks/useEnabledCondition';

interface Props {
  dashFile: DashFile;
  embeddedImages: Map<string, string>;
  legacyMode: boolean;
  scale: number;
  scrollable?: boolean;
  aspectRatio: number;
  bgColor: string;
  backgroundImageLayers: string;
  backgroundSizeLayers: string | undefined;
  backgroundRepeatLayers: string | undefined;
  isConnected: boolean;
  wrapperRef: React.RefObject<HTMLDivElement>;
  onContextMenu: (e: React.MouseEvent, gaugeId: string | null) => void;
}

/**
 * Dashboard rendering surface: scaling wrapper, cluster background, and the
 * component map (via the shared DashComponentView).
 */
export default function DashboardCanvas({
  dashFile,
  embeddedImages,
  legacyMode,
  scale,
  scrollable = false,
  aspectRatio,
  bgColor,
  backgroundImageLayers,
  backgroundSizeLayers,
  backgroundRepeatLayers,
  isConnected,
  wrapperRef,
  onContextMenu,
}: Props) {
  const cluster = dashFile.gauge_cluster;

  return (
    <div
      ref={wrapperRef}
      className={`ts-dashboard-wrapper${scrollable ? ' ts-dashboard-wrapper--scroll' : ''}`}
    >
      <div
        className="ts-dashboard"
        style={{
          backgroundColor: bgColor,
          backgroundImage: backgroundImageLayers || undefined,
          backgroundSize: backgroundSizeLayers,
          backgroundRepeat: backgroundRepeatLayers,
          backgroundPosition: 'center',
          aspectRatio: `${aspectRatio}`,
          width: scrollable ? '100%' : undefined,
          transform: !scrollable && scale < 1 ? `scale(${scale})` : undefined,
          transformOrigin: 'top center',
        }}
        onContextMenu={(e) => onContextMenu(e, null)}
      >
        {cluster.components.map((component, index) => (
          <DashComponentView
            key={(isGauge(component) ? component.Gauge.id : isIndicator(component) ? component.Indicator.id : null) || `gauge-${index}`}
            component={component}
            embeddedImages={embeddedImages}
            legacyMode={legacyMode}
            isConnected={isConnected}
            onContextMenu={onContextMenu}
          />
        ))}
        {/* Additional clusters layered on top of the primary cluster (Plan D-6).
            They share the wrapper's coordinate space; per-cluster
            enabled_condition gates their visibility. */}
        {(dashFile.additional_clusters ?? []).map((extra, idx) => (
          <ExtraClusterLayer
            key={`extra-cluster-${idx}`}
            cluster={extra}
            embeddedImages={embeddedImages}
            legacyMode={legacyMode}
            isConnected={isConnected}
            onContextMenu={onContextMenu}
          />
        ))}
      </div>
    </div>
  );
}

/** Renders an additional cluster atop the primary one (Plan D-6, multi-cluster). */
function ExtraClusterLayer({
  cluster,
  embeddedImages,
  legacyMode,
  isConnected,
  onContextMenu,
}: {
  cluster: GaugeCluster;
  embeddedImages: Map<string, string>;
  legacyMode: boolean;
  isConnected: boolean;
  onContextMenu: (e: React.MouseEvent, gaugeId: string | null) => void;
}) {
  const visible = useEnabledCondition(cluster.enabled_condition ?? null);
  if (!visible) return null;
  return (
    <>
      {cluster.components.map((component, index) => (
        <DashComponentView
          key={(isGauge(component) ? component.Gauge.id : isIndicator(component) ? component.Indicator.id : null) || `xg-${index}`}
          component={component}
          embeddedImages={embeddedImages}
          legacyMode={legacyMode}
          isConnected={isConnected}
          onContextMenu={onContextMenu}
        />
      ))}
    </>
  );
}
