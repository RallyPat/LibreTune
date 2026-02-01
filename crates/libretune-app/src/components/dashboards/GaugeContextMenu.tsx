/**
 * Gauge Context Menu
 * 
 * Right-click menu for dashboard gauges matching TS functionality:
 * - LibreTune Gauges submenu (grouped by category from INI)
 * - Background settings
 * - Designer Mode toggle
 * - Gauge Demo mode
 */

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TsColor, tsColorToHex } from './dashTypes';
import './GaugeContextMenu.css';

/** Gauge category from INI [GaugeConfigurations] */
interface GaugeCategory {
  name: string;
  gauges: GaugeInfo[];
}

/** Individual gauge info */
interface GaugeInfo {
  name: string;
  channel: string;
  title: string;
  units: string;
  min: number;
  max: number;
}

export interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  targetGaugeId: string | null; // null = clicked on background
}

export interface GaugeContextMenuProps {
  state: ContextMenuState;
  onClose: () => void;
  
  // Dashboard settings
  designerMode: boolean;
  onDesignerModeChange: (enabled: boolean) => void;
  antialiasingEnabled: boolean;
  onAntialiasingChange: (enabled: boolean) => void;
  gaugeDemoActive: boolean;
  onGaugeDemoToggle: () => void;
  
  // Background settings
  backgroundColor: TsColor;
  onBackgroundColorChange: (color: TsColor) => void;
  backgroundDitherColor: TsColor | null;
  onBackgroundDitherColorChange: (color: TsColor | null) => void;
  
  // Gauge operations
  onReloadDefaultGauges: () => void;
  onResetValue: () => void;
  onReplaceGauge: (channel: string, gaugeInfo: GaugeInfo) => void;
}

export default function GaugeContextMenu({
  state,
  onClose,
  designerMode,
  onDesignerModeChange,
  antialiasingEnabled,
  onAntialiasingChange,
  gaugeDemoActive,
  onGaugeDemoToggle,
  backgroundColor,
  onBackgroundColorChange,
  backgroundDitherColor,
  onBackgroundDitherColorChange,
  onReloadDefaultGauges,
  onResetValue,
  onReplaceGauge,
}: GaugeContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [gaugeCategories, setGaugeCategories] = useState<GaugeCategory[]>([]);
  const [expandedSubmenu, setExpandedSubmenu] = useState<string | null>(null);
  const [expandedCategory, setExpandedCategory] = useState<string | null>(null);

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    if (state.visible) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [state.visible, onClose]);

  // Load gauge categories from INI
  useEffect(() => {
    const loadGaugeCategories = async () => {
      try {
        const categories = await invoke<GaugeCategory[]>('get_gauge_categories');
        setGaugeCategories(categories);
      } catch (e) {
        console.error('Failed to load gauge categories:', e);
        // Provide some default categories if backend doesn't have them yet
        setGaugeCategories([
          {
            name: 'Sensors - Basic',
            gauges: [
              { name: 'rpm', channel: 'rpm', title: 'RPM', units: '', min: 0, max: 8000 },
              { name: 'map', channel: 'map', title: 'MAP', units: 'kPa', min: 0, max: 250 },
              { name: 'tps', channel: 'tps', title: 'TPS', units: '%', min: 0, max: 100 },
              { name: 'coolant', channel: 'coolant', title: 'Coolant', units: '°C', min: -40, max: 120 },
              { name: 'iat', channel: 'iat', title: 'IAT', units: '°C', min: -40, max: 80 },
            ],
          },
          {
            name: 'Fueling',
            gauges: [
              { name: 'afr', channel: 'afr', title: 'AFR', units: '', min: 10, max: 20 },
              { name: 've', channel: 've', title: 'VE', units: '%', min: 0, max: 150 },
              { name: 'pulseWidth1', channel: 'pulseWidth1', title: 'PW', units: 'ms', min: 0, max: 20 },
            ],
          },
          {
            name: 'Ignition',
            gauges: [
              { name: 'advance', channel: 'advance', title: 'Advance', units: '°', min: -10, max: 50 },
              { name: 'dwell', channel: 'dwell', title: 'Dwell', units: 'ms', min: 0, max: 10 },
            ],
          },
        ]);
      }
    };

    if (state.visible) {
      loadGaugeCategories();
    }
  }, [state.visible]);

  // Close all submenus
  const closeSubmenus = () => {
    setExpandedSubmenu(null);
    setExpandedCategory(null);
  };

  if (!state.visible) {
    return null;
  }

  // Adjust position to keep menu on screen
  const adjustedX = Math.min(state.x, window.innerWidth - 250);
  const adjustedY = Math.min(state.y, window.innerHeight - 400);

  return (
    <div
      ref={menuRef}
      className="gauge-context-menu"
      style={{ left: adjustedX, top: adjustedY }}
      onMouseLeave={closeSubmenus}
    >
      {/* Reload Default Gauges */}
      <div className="menu-item" onClick={() => { onReloadDefaultGauges(); onClose(); }}>
        Reload Default Gauges
      </div>

      {/* TS/LibreTune Gauges Submenu (Designer Mode Only) */}
      {designerMode && (
        <>
          <div
            className="menu-item has-submenu"
            onMouseEnter={() => setExpandedSubmenu('gauges')}
          >
            TS/LibreTune Gauges
            <span className="submenu-arrow">▶</span>
            
            {expandedSubmenu === 'gauges' && (
              <div className="submenu">
                {gaugeCategories.map((category) => (
                  <div
                    key={category.name}
                    className="menu-item has-submenu"
                    onMouseEnter={() => setExpandedCategory(category.name)}
                  >
                    {category.name}
                    <span className="submenu-arrow">▶</span>
                    
                    {expandedCategory === category.name && (
                      <div className="submenu">
                        {category.gauges.map((gauge) => (
                          <div
                            key={gauge.channel}
                            className="menu-item"
                            onClick={() => {
                              onReplaceGauge(gauge.channel, gauge);
                              onClose();
                            }}
                          >
                            {gauge.title}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="menu-separator" />
        </>
      )}

      {/* Reset Value (only if a gauge is selected) */}
      {state.targetGaugeId && (
        <div className="menu-item" onClick={() => { onResetValue(); onClose(); }}>
          Reset Value
        </div>
      )}

      <div className="menu-separator" />

      {/* Background Submenu */}
      <div
        className="menu-item has-submenu"
        onMouseEnter={() => setExpandedSubmenu('background')}
      >
        Background
        <span className="submenu-arrow">▶</span>
        
        {expandedSubmenu === 'background' && (
          <div className="submenu">
            <div className="menu-item">
              <label>
                Set Background Color
                <input
                  type="color"
                  value={tsColorToHex(backgroundColor)}
                  onChange={(e) => {
                    const hex = e.target.value;
                    const r = parseInt(hex.slice(1, 3), 16);
                    const g = parseInt(hex.slice(3, 5), 16);
                    const b = parseInt(hex.slice(5, 7), 16);
                    onBackgroundColorChange({ alpha: 255, red: r, green: g, blue: b });
                  }}
                  className="color-picker"
                />
              </label>
            </div>
            <div className="menu-item">
              <label>
                Set Background Dither Color
                <input
                  type="color"
                  value={tsColorToHex(backgroundDitherColor)}
                  onChange={(e) => {
                    const hex = e.target.value;
                    const r = parseInt(hex.slice(1, 3), 16);
                    const g = parseInt(hex.slice(3, 5), 16);
                    const b = parseInt(hex.slice(5, 7), 16);
                    onBackgroundDitherColorChange({ alpha: 255, red: r, green: g, blue: b });
                  }}
                  className="color-picker"
                />
              </label>
            </div>
            <div className="menu-item">
              Set Background Image...
            </div>
            <div
              className="menu-item has-submenu"
              onMouseEnter={() => setExpandedCategory('imagePosition')}
            >
              Image Position
              <span className="submenu-arrow">▶</span>
              
              {expandedCategory === 'imagePosition' && (
                <div className="submenu">
                  <div className="menu-item">Stretch</div>
                  <div className="menu-item">Tile</div>
                  <div className="menu-item">Center</div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <div className="menu-separator" />

      {/* Antialiasing Toggle */}
      <div
        className="menu-item checkbox"
        onClick={() => onAntialiasingChange(!antialiasingEnabled)}
      >
        <span className={`checkbox-mark ${antialiasingEnabled ? 'checked' : ''}`}>
          {antialiasingEnabled ? '☑' : '☐'}
        </span>
        Antialiasing Enabled
      </div>

      {/* Designer Mode Toggle */}
      <div
        className="menu-item checkbox"
        onClick={() => onDesignerModeChange(!designerMode)}
      >
        <span className={`checkbox-mark ${designerMode ? 'checked' : ''}`}>
          {designerMode ? '☑' : '☐'}
        </span>
        Designer Mode
      </div>

      {/* Gauge Demo */}
      <div
        className="menu-item"
        onClick={() => { onGaugeDemoToggle(); onClose(); }}
      >
        {gaugeDemoActive ? 'Stop Gauge Demo' : 'Gauge Demo'}
      </div>
    </div>
  );
}
