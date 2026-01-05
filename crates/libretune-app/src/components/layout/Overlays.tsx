import { FolderOpen, Play, X } from 'lucide-react';
import AutoTuneLive from '../realtime/AutoTuneLive';

export interface OverlayState {
  showVeAnalyze: boolean;
  showPerformanceDialog: boolean;
  showActionsPanel: boolean;
  showSaveDialog: boolean;
  showLoadDialog: boolean;
  showBurnDialog: boolean;
  showNewProjectDialog: boolean;
  showBrowseProjectsDialog: boolean;
  showRebinDialog: boolean;
  showCellEditDialog: boolean;
  cellEditValue: number;
}

export interface OverlaysProps {
  state: OverlayState;
  onClose: (key: keyof OverlayState) => void;
  onCellEditValueChange: (value: number) => void;
  availableInis?: string[];
  onBrowseIni?: () => void;
  onLoadIni?: (path: string) => void;
}

export default function Overlays({ state, onClose, onCellEditValueChange, availableInis = [], onBrowseIni, onLoadIni }: OverlaysProps) {
  return (
    <>
      {state.showVeAnalyze && (
        <div className="modal-overlay">
          <div className="modal-content large-modal">
            <div className="modal-header">
              <h2>AutoTune Live</h2>
              <button className="icon-btn" onClick={() => onClose('showVeAnalyze')}>
                <X size={20} />
              </button>
            </div>
            <AutoTuneLive onClose={() => onClose('showVeAnalyze')} />
          </div>
        </div>
      )}

      {state.showPerformanceDialog && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>Performance & Economy Fields</h2>
              <button className="icon-btn" onClick={() => onClose('showPerformanceDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <p style={{ color: 'var(--text-muted)', marginBottom: '1rem' }}>
                Enter vehicle specifications to calculate performance metrics.
              </p>
              <div className="settings-field">
                <label>Injector Size (cc)</label>
                <input type="number" defaultValue={0} placeholder="e.g., 550" />
              </div>
              <div className="settings-field">
                <label>Vehicle Weight (lbs)</label>
                <input type="number" defaultValue={0} placeholder="e.g., 2500" />
              </div>
              <div className="settings-field">
                <label>Frontal Area (sq ft)</label>
                <input type="number" defaultValue={0} step="0.1" placeholder="e.g., 20" />
              </div>
              <div className="settings-field">
                <label>Drag Coefficient</label>
                <input type="number" defaultValue={0} step="0.01" placeholder="e.g., 0.35" />
              </div>
              <button
                className="primary-btn"
                style={{ marginTop: '1rem' }}
                onClick={() => onClose('showPerformanceDialog')}
              >
                Save Settings
              </button>
            </div>
          </div>
        </div>
      )}

      {state.showActionsPanel && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>Action Management</h2>
              <button className="icon-btn" onClick={() => onClose('showActionsPanel')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <p style={{ color: 'var(--text-muted)' }}>Record and playback tuning actions.</p>
              <div style={{ display: 'flex', gap: '1rem', marginTop: '1rem' }}>
                <button className="primary-btn">
                  <Play size={16} style={{ marginRight: '0.5rem' }} />
                  Record
                </button>
                <button className="secondary-btn">Load Action List</button>
              </div>
            </div>
          </div>
        </div>
      )}

      {state.showSaveDialog && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>Save Tune</h2>
              <button className="icon-btn" onClick={() => onClose('showSaveDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <div className="settings-field">
                <label>Filename</label>
                <input type="text" defaultValue="tune" placeholder="tune" />
              </div>
              <div className="settings-field">
                <label>
                  <input type="checkbox" defaultChecked />
                  Auto-burn on close
                </label>
              </div>
              <button
                className="primary-btn"
                style={{ marginTop: '1rem' }}
                onClick={() => onClose('showSaveDialog')}
              >
                Save
              </button>
            </div>
          </div>
        </div>
      )}

      {state.showLoadDialog && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>Load Tune</h2>
              <button className="icon-btn" onClick={() => onClose('showLoadDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <p style={{ color: 'var(--text-muted)' }}>Select a tune file to load.</p>
              <button className="primary-btn" onClick={() => onClose('showLoadDialog')}>
                Browse Files...
              </button>
            </div>
          </div>
        </div>
      )}

      {state.showBurnDialog && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>Burn to ECU</h2>
              <button className="icon-btn" onClick={() => onClose('showBurnDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <p style={{ marginBottom: '1rem' }}>
                Are you sure you want to burn the current tune to the ECU flash memory?
              </p>
              <div style={{ display: 'flex', gap: '1rem' }}>
                <button className="primary-btn" onClick={() => onClose('showBurnDialog')}>
                  Burn Now
                </button>
                <button className="secondary-btn" onClick={() => onClose('showBurnDialog')}>
                  Cancel
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {state.showNewProjectDialog && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>New Project</h2>
              <button className="icon-btn" onClick={() => onClose('showNewProjectDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <div className="settings-field">
                <label>Project Name</label>
                <input type="text" placeholder="My Project" />
              </div>
              <div className="settings-field">
                <label>ECU Definition File</label>
                <p style={{ fontSize: '13px', color: 'var(--text-muted)', margin: '8px 0' }}>
                  Select an ECU definition (.ini) file to configure the project.
                  The INI file determines available features, tables, and dialogs.
                </p>
                {availableInis.length > 0 && (
                  <select 
                    defaultValue=""
                    onChange={(e) => e.target.value && onLoadIni?.(e.target.value)}
                    style={{ width: '100%', marginBottom: '8px' }}
                  >
                    <option value="">Select from available definitions...</option>
                    {availableInis.map((ini) => (
                      <option key={ini} value={ini}>
                        {ini.split('/').pop()}
                      </option>
                    ))}
                  </select>
                )}
                <button
                  className="secondary-btn"
                  style={{ display: 'flex', alignItems: 'center', gap: '8px' }}
                  onClick={() => onBrowseIni?.()}
                >
                  <FolderOpen size={16} />
                  Browse for Definition...
                </button>
              </div>
              <button
                className="primary-btn"
                style={{ marginTop: '1rem' }}
                onClick={() => onClose('showNewProjectDialog')}
              >
                Create Project
              </button>
            </div>
          </div>
        </div>
      )}

      {state.showBrowseProjectsDialog && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>Browse Projects</h2>
              <button className="icon-btn" onClick={() => onClose('showBrowseProjectsDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <p style={{ color: 'var(--text-muted)' }}>Select a project to open.</p>
            </div>
          </div>
        </div>
      )}

      {state.showRebinDialog && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h2>Re-bin Table</h2>
              <button className="icon-btn" onClick={() => onClose('showRebinDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <p style={{ color: 'var(--text-muted)', marginBottom: '1rem' }}>
                Enter new bin values. The Z values will be interpolated automatically.
              </p>
              <button className="primary-btn" onClick={() => onClose('showRebinDialog')}>
                Apply Re-binning
              </button>
            </div>
          </div>
        </div>
      )}

      {state.showCellEditDialog && (
        <div className="modal-overlay">
          <div className="modal-content small-modal">
            <div className="modal-header">
              <h2>Edit Cell Value</h2>
              <button className="icon-btn" onClick={() => onClose('showCellEditDialog')}>
                <X size={20} />
              </button>
            </div>
            <div className="glass-card" style={{ padding: '2rem' }}>
              <div className="settings-field">
                <label>Value</label>
                <input
                  type="number"
                  value={state.cellEditValue}
                  onChange={(e) => onCellEditValueChange(parseFloat(e.target.value))}
                />
              </div>
              <button
                className="primary-btn"
                style={{ marginTop: '1rem' }}
                onClick={() => onClose('showCellEditDialog')}
              >
                Apply
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
