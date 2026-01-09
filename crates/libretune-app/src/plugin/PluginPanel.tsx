/**
 * PluginPanel - UI for managing and displaying TS-compatible plugins.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { PluginInfo, SwingComponent } from './types';
import { SwingRenderer } from './SwingRenderer';
import './PluginPanel.css';

interface PluginPanelProps {
  onClose?: () => void;
}

export const PluginPanel: React.FC<PluginPanelProps> = ({ onClose }) => {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [selectedPlugin, setSelectedPlugin] = useState<string | null>(null);
  const [pluginUi, setPluginUi] = useState<SwingComponent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [jreStatus, setJreStatus] = useState<string | null>(null);

  // Check JRE on mount
  useEffect(() => {
    checkJre();
    loadPlugins();
  }, []);

  const checkJre = async () => {
    try {
      const version = await invoke<string>('check_jre');
      setJreStatus(version);
    } catch (err) {
      setJreStatus(`Not found: ${err}`);
    }
  };

  const loadPlugins = async () => {
    try {
      const list = await invoke<PluginInfo[]>('list_plugins');
      setPlugins(list);
    } catch (err) {
      console.error('Failed to load plugins:', err);
    }
  };

  const handleLoadPlugin = useCallback(async () => {
    try {
      setError(null);
      const selected = await open({
        multiple: false,
        filters: [{ name: 'JAR files', extensions: ['jar'] }],
        title: 'Select TunerStudio Plugin JAR',
      });

      if (!selected) return;

      setLoading(true);
      const info = await invoke<PluginInfo>('load_plugin', { jarPath: selected });
      setPlugins(prev => [...prev, info]);
      setSelectedPlugin(info.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleUnloadPlugin = useCallback(async (pluginId: string) => {
    try {
      setError(null);
      await invoke('unload_plugin', { pluginId });
      setPlugins(prev => prev.filter(p => p.id !== pluginId));
      if (selectedPlugin === pluginId) {
        setSelectedPlugin(null);
        setPluginUi(null);
      }
    } catch (err) {
      setError(String(err));
    }
  }, [selectedPlugin]);

  const handleSelectPlugin = useCallback(async (pluginId: string) => {
    setSelectedPlugin(pluginId);
    setError(null);
    setLoading(true);

    try {
      const ui = await invoke<SwingComponent | null>('get_plugin_ui', { pluginId });
      setPluginUi(ui);
    } catch (err) {
      setError(String(err));
      setPluginUi(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const selectedPluginInfo = plugins.find(p => p.id === selectedPlugin);

  return (
    <div className="plugin-panel">
      <div className="plugin-panel-header">
        <h2>TS Plugins</h2>
        {onClose && (
          <button className="close-button" onClick={onClose}>×</button>
        )}
      </div>

      <div className="plugin-panel-content">
        {/* Sidebar: Plugin list */}
        <div className="plugin-sidebar">
          <div className="plugin-toolbar">
            <button 
              className="plugin-load-btn"
              onClick={handleLoadPlugin}
              disabled={loading}
            >
              + Load Plugin
            </button>
          </div>

          <div className="plugin-list">
            {plugins.length === 0 ? (
              <div className="plugin-empty">
                No plugins loaded.<br />
                Click "Load Plugin" to add a TS-compatible JAR plugin.
              </div>
            ) : (
              plugins.map(plugin => (
                <div
                  key={plugin.id}
                  className={`plugin-item ${selectedPlugin === plugin.id ? 'selected' : ''}`}
                  onClick={() => handleSelectPlugin(plugin.id)}
                >
                  <div className="plugin-item-info">
                    <div className="plugin-name">{plugin.displayName}</div>
                    <div className="plugin-version">v{plugin.version}</div>
                    <div className="plugin-type">{plugin.pluginType}</div>
                  </div>
                  <button
                    className="plugin-unload-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleUnloadPlugin(plugin.id);
                    }}
                    title="Unload plugin"
                  >
                    ×
                  </button>
                </div>
              ))
            )}
          </div>

          <div className="plugin-jre-status">
            <span className="jre-label">Java:</span>
            <span className={`jre-value ${jreStatus?.includes('Not found') ? 'error' : ''}`}>
              {jreStatus || 'Checking...'}
            </span>
          </div>
        </div>

        {/* Main area: Plugin UI */}
        <div className="plugin-main">
          {error && (
            <div className="plugin-error">
              <strong>Error:</strong> {error}
            </div>
          )}

          {loading && (
            <div className="plugin-loading">
              Loading plugin...
            </div>
          )}

          {!loading && selectedPlugin && selectedPluginInfo && (
            <div className="plugin-ui-container">
              <div className="plugin-ui-header">
                <h3>{selectedPluginInfo.displayName}</h3>
                <p>{selectedPluginInfo.description}</p>
                {selectedPluginInfo.helpUrl && (
                  <a 
                    href={selectedPluginInfo.helpUrl} 
                    target="_blank" 
                    rel="noopener noreferrer"
                  >
                    Help
                  </a>
                )}
              </div>

              <div className="plugin-ui-content">
                {pluginUi ? (
                  <SwingRenderer 
                    pluginId={selectedPlugin} 
                    component={pluginUi} 
                  />
                ) : (
                  <div className="plugin-no-ui">
                    This plugin does not have a UI panel.
                  </div>
                )}
              </div>
            </div>
          )}

          {!loading && !selectedPlugin && (
            <div className="plugin-placeholder">
              Select a plugin from the list to view its interface.
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default PluginPanel;
