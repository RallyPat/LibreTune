import React from 'react';
import { DashFile, GaugeCluster, isGauge, isIndicator } from '../dashTypes';
import TsGauge from '../../gauges/TsGauge';
import LiveTsIndicator from './LiveTsIndicator';
import { useEnabledCondition } from '../hooks/useEnabledCondition';
import { isDesignerHidden } from '../shared/designerHidden';

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
 * Dashboard rendering surface (scaling wrapper, gauge/indicator map).
 * Extracted from TsDashboard during Phase C4.
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
  const toPercent = (v: number | undefined | null) => (v ?? 0) * 100;

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
        {cluster.components.map((component, index) => {
          if (isDesignerHidden(component)) return null;

          if (isGauge(component)) {
            const gauge = component.Gauge;

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
              <ConditionalWrapper key={gauge.id || `gauge-${index}`} condition={gauge.enabled_condition ?? null}>
                <div
                  className="ts-component ts-gauge"
                  style={gaugeStyle}
                  onContextMenu={(e) => onContextMenu(e, gauge.id)}
                >
                  <TsGauge
                    config={gauge}
                    embeddedImages={embeddedImages}
                    legacyMode={legacyMode}
                    isConnected={isConnected}
                  />
                </div>
              </ConditionalWrapper>
            );
          }

          if (isIndicator(component)) {
            const indicator = component.Indicator;
            return (
              <ConditionalWrapper key={indicator.id || `indicator-${index}`} condition={indicator.enabled_condition ?? null}>
                <div
                  className="ts-component ts-indicator"
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
              </ConditionalWrapper>
            );
          }

          return null;
        })}
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

/** Visibility wrapper that hides children when their `enabled_condition` evaluates false. */
function ConditionalWrapper({
  condition,
  children,
}: {
  condition: string | null;
  children: React.ReactNode;
}) {
  const visible = useEnabledCondition(condition);
  if (!visible) return null;
  return <>{children}</>;
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
  const toPercent = (v: number | undefined | null) => (v ?? 0) * 100;
  return (
    <>
      {cluster.components.map((component, index) => {
        if (isDesignerHidden(component)) return null;

        if (isGauge(component)) {
          const gauge = component.Gauge;
          const style: React.CSSProperties = {
            left: `${toPercent(gauge.relative_x)}%`,
            top: `${toPercent(gauge.relative_y)}%`,
            width: `${toPercent(gauge.relative_width)}%`,
            height: `${toPercent(gauge.relative_height)}%`,
            aspectRatio: gauge.shape_locked_to_aspect ? '1 / 1' : undefined,
          };
          return (
            <ConditionalWrapper key={gauge.id || `xg-${index}`} condition={gauge.enabled_condition ?? null}>
              <div
                className="ts-component ts-gauge"
                style={style}
                onContextMenu={(e) => onContextMenu(e, gauge.id)}
              >
                <TsGauge
                  config={gauge}
                  embeddedImages={embeddedImages}
                  legacyMode={legacyMode}
                  isConnected={isConnected}
                />
              </div>
            </ConditionalWrapper>
          );
        }
        if (isIndicator(component)) {
          const ind = component.Indicator;
          return (
            <ConditionalWrapper key={ind.id || `xi-${index}`} condition={ind.enabled_condition ?? null}>
              <div
                className="ts-component ts-indicator"
                style={{
                  left: `${toPercent(ind.relative_x)}%`,
                  top: `${toPercent(ind.relative_y)}%`,
                  width: `${toPercent(ind.relative_width)}%`,
                  height: `${toPercent(ind.relative_height)}%`,
                }}
                onContextMenu={(e) => onContextMenu(e, ind.id)}
              >
                <LiveTsIndicator config={ind} embeddedImages={embeddedImages} />
              </div>
            </ConditionalWrapper>
          );
        }
        return null;
      })}
    </>
  );
}
