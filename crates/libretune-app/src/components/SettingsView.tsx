import { useState, useEffect } from "react";
import { Gamepad2, AlertTriangle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useTheme, ThemeName, THEME_INFO } from "../themes";
import { useToast } from "./ToastContext";

// Settings view
export function SettingsView() {
  const { theme, setTheme } = useTheme();
  const { showToast } = useToast();
  const [demoMode, setDemoMode] = useState(false);
  const [demoLoading, setDemoLoading] = useState(false);
  const [showAllHelpIcons, setShowAllHelpIcons] = useState(true);

  // Check demo mode status and load settings on mount
  useEffect(() => {
    invoke<boolean>("get_demo_mode").then(setDemoMode).catch(console.error);
    invoke<{ show_all_help_icons?: boolean }>("get_settings")
      .then((settings) => {
        if (settings.show_all_help_icons !== undefined) {
          setShowAllHelpIcons(settings.show_all_help_icons);
        }
      })
      .catch(console.error);
  }, []);

  const handleDemoToggle = async () => {
    setDemoLoading(true);
    try {
      const newValue = !demoMode;
      await invoke("set_demo_mode", { enabled: newValue });
      setDemoMode(newValue);
      
      if (newValue) {
        // Start realtime streaming when demo mode is enabled
        await invoke("start_realtime_stream", { intervalMs: 50 });
      } else {
        // Stop streaming when demo mode is disabled
        await invoke("stop_realtime_stream");
      }
    } catch (err) {
      console.error("Failed to toggle demo mode:", err);
      showToast(`Failed to toggle demo mode: ${err}`, "error");
    } finally {
      setDemoLoading(false);
    }
  };

  return (
    <div style={{ padding: 20 }}>
      <h2 style={{ marginBottom: 20 }}>Settings</h2>
      
      {/* Demo Mode Section */}
      <div style={{ 
        marginBottom: 24, 
        padding: 16, 
        background: demoMode ? 'rgba(255, 152, 0, 0.1)' : 'var(--bg-surface)', 
        border: `1px solid ${demoMode ? '#ff9800' : 'var(--border)'}`,
        borderRadius: 8 
      }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
          <label style={{ fontWeight: 600, fontSize: 14, display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <Gamepad2 size={16} aria-hidden /> Demo Mode (Simulated ECU)
          </label>
          <button
            onClick={handleDemoToggle}
            disabled={demoLoading}
            style={{
              padding: '6px 16px',
              background: demoMode ? '#ff9800' : 'var(--bg-elevated)',
              color: demoMode ? 'white' : 'var(--text-primary)',
              border: `1px solid ${demoMode ? '#e65100' : 'var(--border)'}`,
              borderRadius: 4,
              cursor: demoLoading ? 'wait' : 'pointer',
              fontWeight: 500,
            }}
          >
            {demoLoading ? 'Loading...' : demoMode ? 'Disable' : 'Enable'}
          </button>
        </div>
        <p style={{ 
          color: demoMode ? '#ffb74d' : 'var(--text-muted)', 
          fontSize: 12, 
          margin: 0,
          lineHeight: 1.5,
          display: 'flex',
          alignItems: 'flex-start',
          gap: 6,
        }}>
          <AlertTriangle size={14} aria-hidden style={{ flexShrink: 0, marginTop: 2 }} />
          <span>This generates <strong>fake sensor data</strong> for UI testing. 
          You are <strong>NOT connected to a real ECU</strong>. 
          The simulated engine idles at ~850 RPM with occasional throttle blips.</span>
        </p>
      </div>

      {/* Theme Section */}
      <div style={{ marginBottom: 16 }}>
        <label style={{ display: "block", marginBottom: 8 }}>Theme</label>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          {Object.entries(THEME_INFO).map(([key, info]) => (
            <label 
              key={key}
              style={{ 
                display: 'flex', 
                alignItems: 'center', 
                gap: 10, 
                padding: '6px 8px',
                background: theme === key ? 'var(--bg-selected)' : 'transparent',
                borderRadius: 4,
                cursor: 'pointer',
              }}
            >
              <input
                type="radio"
                name="theme"
                value={key}
                checked={theme === key}
                onChange={() => setTheme(key as ThemeName)}
                style={{ margin: 0 }}
              />
              {/* Color swatch preview */}
              <div style={{ 
                display: 'flex', 
                gap: 2, 
                borderRadius: 3, 
                overflow: 'hidden',
                border: '1px solid var(--border-default)',
              }}>
                <div style={{ width: 16, height: 16, background: info.bg }} />
                <div style={{ width: 16, height: 16, background: info.primary }} />
                <div style={{ width: 16, height: 16, background: info.accent }} />
              </div>
              <span style={{ flex: 1 }}>{info.label}</span>
            </label>
          ))}
        </div>
      </div>

      {/* Help Icons Section */}
      <div style={{ 
        marginBottom: 16, 
        padding: 16, 
        background: 'var(--bg-surface)', 
        border: '1px solid var(--border)',
        borderRadius: 8 
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 8 }}>
          <input
            type="checkbox"
            id="showAllHelpIcons"
            checked={showAllHelpIcons}
            onChange={async (e) => {
              const newValue = e.target.checked;
              setShowAllHelpIcons(newValue);
              try {
                await invoke("update_setting", { key: "show_all_help_icons", value: newValue.toString() });
              } catch (err) {
                console.error("Failed to save help icons setting:", err);
                showToast(`Failed to save setting: ${err}`, "error");
              }
            }}
            style={{ width: 18, height: 18 }}
          />
          <label htmlFor="showAllHelpIcons" style={{ fontWeight: 600, fontSize: 14, cursor: 'pointer' }}>
            Show help icons on all fields
          </label>
        </div>
        <p style={{ 
          color: 'var(--text-muted)', 
          fontSize: 12, 
          margin: 0,
          marginLeft: 30,
          lineHeight: 1.5
        }}>
          When disabled, help icons only appear for fields that have descriptions defined in the INI file.
        </p>
      </div>
    </div>
  );
}

