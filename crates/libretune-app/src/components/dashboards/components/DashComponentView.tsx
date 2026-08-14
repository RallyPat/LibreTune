/**
 * DashComponentView — renders one positioned gauge or indicator.
 *
 * The single render path shared by the live dashboard canvas and the
 * (WYSIWYG) designer canvas: position/size style, enabled-condition
 * gating, and context-menu wiring live here so the two views can never
 * drift apart.
 */

import React from 'react';
import { DashComponent, isGauge, isIndicator } from '../dashTypes';
import TsGauge from '../../gauges/TsGauge';
import LiveTsIndicator from './LiveTsIndicator';
import { useEnabledCondition } from '../hooks/useEnabledCondition';
import { isDesignerHidden } from '../shared/designerHidden';

const toPercent = (v: number | undefined | null) => (v ?? 0) * 100;

export interface DashComponentViewProps {
  component: DashComponent;
  embeddedImages: Map<string, string>;
  legacyMode: boolean;
  isConnected: boolean;
  onContextMenu: (e: React.MouseEvent, gaugeId: string | null) => void;
  /** Extra classes for the positioned wrapper (designer selection chrome). */
  className?: string;
  /** Overrides for position/size (designer drag preview / fill-parent). */
  styleOverride?: React.CSSProperties;
  children?: React.ReactNode;
  onMouseDown?: (e: React.MouseEvent, id: string) => void;
  onClick?: (e: React.MouseEvent, id: string) => void;
  /** Designer mode: render designer-hidden components (dimmed by caller). */
  showHidden?: boolean;
  /** Designer mode: skip enabled_condition gating so everything is editable. */
  ignoreConditions?: boolean;
}

export default function DashComponentView({
  component,
  embeddedImages,
  legacyMode,
  isConnected,
  onContextMenu,
  className = '',
  styleOverride,
  children,
  onMouseDown,
  onClick,
  showHidden = false,
  ignoreConditions = false,
}: DashComponentViewProps) {
  if (!showHidden && isDesignerHidden(component)) return null;

  /** Gates children on the enabled_condition unless the designer asked to skip it. */
  const wrapConditionally = (condition: string | null, element: React.ReactElement) =>
    ignoreConditions ? element : <ConditionalWrapper condition={condition}>{element}</ConditionalWrapper>;

  if (isGauge(component)) {
    const gauge = component.Gauge;
    const style: React.CSSProperties = {
      left: `${toPercent(gauge.relative_x)}%`,
      top: `${toPercent(gauge.relative_y)}%`,
      width: `${toPercent(gauge.relative_width)}%`,
      height: `${toPercent(gauge.relative_height)}%`,
      minWidth: !legacyMode && gauge.shortest_size > 0 ? `${gauge.shortest_size}px` : undefined,
      minHeight: !legacyMode && gauge.shortest_size > 0 ? `${gauge.shortest_size}px` : undefined,
      aspectRatio: gauge.shape_locked_to_aspect ? '1 / 1' : undefined,
      ...styleOverride,
    };
    return wrapConditionally(
      gauge.enabled_condition ?? null,
      <div
        className={`ts-component ts-gauge ${className}`}
        style={style}
        onContextMenu={(e) => onContextMenu(e, gauge.id)}
        onMouseDown={onMouseDown ? (e) => onMouseDown(e, gauge.id) : undefined}
        onClick={onClick ? (e) => onClick(e, gauge.id) : undefined}
      >
        <TsGauge
          config={gauge}
          embeddedImages={embeddedImages}
          legacyMode={legacyMode}
          isConnected={isConnected}
        />
        {children}
      </div>,
    );
  }

  if (isIndicator(component)) {
    const indicator = component.Indicator;
    return wrapConditionally(
      indicator.enabled_condition ?? null,
      <div
        className={`ts-component ts-indicator ${className}`}
        style={{
          left: `${toPercent(indicator.relative_x)}%`,
          top: `${toPercent(indicator.relative_y)}%`,
          width: `${toPercent(indicator.relative_width)}%`,
          height: `${toPercent(indicator.relative_height)}%`,
          ...styleOverride,
        }}
        onContextMenu={(e) => onContextMenu(e, indicator.id)}
        onMouseDown={onMouseDown ? (e) => onMouseDown(e, indicator.id) : undefined}
        onClick={onClick ? (e) => onClick(e, indicator.id) : undefined}
      >
        <LiveTsIndicator config={indicator} embeddedImages={embeddedImages} />
        {children}
      </div>,
    );
  }

  return null;
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
