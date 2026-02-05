import React, { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./PluginPanel.css";

interface Plugin {
  name: string;
  version: string;
  description: string;
  author: string;
  state: string;
  permissions: string[];
  exec_count: number;
}

interface PluginPanelProps {
  isConnected: boolean;
}

export const PluginPanel: React.FC<PluginPanelProps> = ({ isConnected }) => {
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedPlugin, setSelectedPlugin] = useState<string | null>(null);
  const [showPermissions, setShowPermissions] = useState(false);

  // Load list of plugins
  const loadPlugins = useCallback(async () => {
    try {
      setLoading(true);
      const list: Plugin[] = await invoke("list_wasm_plugins");
      setPlugins(list);
    } catch (error) {
      console.error("Failed to load plugins:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  // Load on mount
  useEffect(() => {
    loadPlugins();
  }, [loadPlugins]);

  // Load plugin from file
  const handleLoadPlugin = useCallback(async () => {
    try {
      const files = await open({
        filters: [
          { name: "WASM Plugins", extensions: ["wasm"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });

      if (files && !Array.isArray(files)) {
        // Create a default manifest for the plugin based on filename
        const filename = (files as string).split("/").pop()?.split("\\").pop()?.replace(".wasm", "") || "unknown";
        const manifest = JSON.stringify({
          name: filename,
          version: "1.0.0",
          description: `Plugin loaded from ${filename}.wasm`,
          author: "Unknown",
          permissions: ["ReadTables"],
        });
        await invoke("load_wasm_plugin", { path: files, manifestJson: manifest });
        await loadPlugins();
      }
    } catch (error) {
      console.error("Failed to load plugin:", error);
    }
  }, [loadPlugins]);

  // Unload plugin
  const handleUnloadPlugin = useCallback(
    async (name: string) => {
      try {
        await invoke("unload_wasm_plugin", { name });
        await loadPlugins();
        setSelectedPlugin(null);
      } catch (error) {
        console.error("Failed to unload plugin:", error);
      }
    },
    [loadPlugins]
  );

  // Execute plugin
  const handleExecutePlugin = useCallback(async (name: string) => {
    try {
      await invoke("execute_wasm_plugin", { name });
      await loadPlugins();
    } catch (error) {
      console.error("Failed to execute plugin:", error);
    }
  }, []);

  // Get permission display
  const getPermissionDisplay = (perm: string) => {
    const permMap: Record<string, string> = {
      ReadTables: "📖 Read Tables",
      WriteConstants: "✏️ Write Constants",
      SubscribeChannels: "📡 Subscribe Channels",
      ExecuteActions: "⚡ Execute Actions",
    };
    return permMap[perm] || perm;
  };

  // Get state color
  const getStateColor = (state: string) => {
    const s = state.toLowerCase();
    switch (s) {
      case "ready":
        return "#4ade80"; // Green
      case "running":
        return "#60a5fa"; // Blue
      case "loaded":
        return "#fbbf24"; // Amber
      case "disabled":
      case "unloading":
        return "#ef4444"; // Red
      default:
        return "#6b7280"; // Gray
    }
  };

  const selected = plugins.find((p) => p.name === selectedPlugin);

  return (
    <div className="plugin-panel">
      <div className="plugin-header">
        <h2>Plugin Manager</h2>
        <button
          className="plugin-button plugin-button-primary"
          onClick={handleLoadPlugin}
          disabled={!isConnected}
          title={isConnected ? "Load plugin" : "Connect to ECU first"}
        >
          + Load Plugin
        </button>
        <button
          className="plugin-button plugin-button-secondary"
          onClick={loadPlugins}
          disabled={loading}
        >
          {loading ? "Loading..." : "Refresh"}
        </button>
      </div>

      <div className="plugin-content">
        <div className="plugin-list">
          <div className="plugin-list-header">
            <h3>Loaded Plugins ({plugins.length})</h3>
          </div>

          {plugins.length === 0 ? (
            <div className="plugin-empty">
              <p>No plugins loaded</p>
              <small>Click "Load Plugin" to add WASM plugins</small>
            </div>
          ) : (
            <div className="plugin-grid">
              {plugins.map((plugin) => (
                <div
                  key={plugin.name}
                  className={`plugin-card ${selectedPlugin === plugin.name ? "selected" : ""}`}
                  onClick={() => setSelectedPlugin(plugin.name)}
                >
                  <div className="plugin-card-header">
                    <div>
                      <h4>{plugin.name}</h4>
                      <small>v{plugin.version}</small>
                    </div>
                    <span
                      className="plugin-state-dot"
                      style={{
                        backgroundColor: getStateColor(plugin.state),
                      }}
                      title={`State: ${plugin.state}`}
                    />
                  </div>
                  <p className="plugin-description">{plugin.description}</p>
                  <div className="plugin-card-footer">
                    <span className="plugin-executions">
                      {plugin.exec_count} executions
                    </span>
                    <span className="plugin-perms">
                      {plugin.permissions.length} permissions
                    </span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {selected && (
          <div className="plugin-details">
            <h3>{selected.name}</h3>

            <div className="plugin-info-section">
              <label>Version</label>
              <span className="plugin-value">{selected.version}</span>
            </div>

            <div className="plugin-info-section">
              <label>State</label>
              <div className="plugin-state-badge">
                <span
                  style={{
                    backgroundColor: getStateColor(selected.state),
                  }}
                />
                <span>{selected.state.toUpperCase()}</span>
              </div>
            </div>

            <div className="plugin-info-section">
              <label>Description</label>
              <p>{selected.description}</p>
            </div>

            <div className="plugin-info-section">
              <label>
                Permissions ({selected.permissions.length}){" "}
                <button
                  className="plugin-expand-btn"
                  onClick={() => setShowPermissions(!showPermissions)}
                >
                  {showPermissions ? "▼" : "▶"}
                </button>
              </label>
              {showPermissions && (
                <div className="plugin-permissions-list">
                  {selected.permissions.length > 0 ? (
                    selected.permissions.map((perm) => (
                      <div key={perm} className="plugin-permission-item">
                        {getPermissionDisplay(perm)}
                      </div>
                    ))
                  ) : (
                    <p className="plugin-no-perms">No permissions required</p>
                  )}
                </div>
              )}
            </div>

            <div className="plugin-info-section">
              <label>Executions</label>
              <span className="plugin-value">{selected.exec_count}</span>
            </div>

            <div className="plugin-actions">
              <button
                className="plugin-button plugin-button-action"
                onClick={() => handleExecutePlugin(selected.name)}
                disabled={selected.state === "disabled"}
              >
                Execute
              </button>
              <button
                className="plugin-button plugin-button-danger"
                onClick={() => handleUnloadPlugin(selected.name)}
              >
                Unload
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default PluginPanel;
