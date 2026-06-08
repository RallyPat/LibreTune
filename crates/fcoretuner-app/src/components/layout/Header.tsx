import { Activity, Database, Plus, Save, Settings, Zap } from 'lucide-react';

interface ConnectionStatus {
  state: 'Disconnected' | 'Connecting' | 'Connected' | string;
  signature: string | null;
  has_definition: boolean;
  ini_name?: string | null;
  demo_mode?: boolean;
}

export interface HeaderProps {
  status: ConnectionStatus;
  onSave: () => void;
  onLoad: () => void;
  onBurn: () => void;
  onNewProject: () => void;
  onBrowseProjects: () => void;
  onRefresh: () => void;
  onSettings: () => void;
}

export default function Header({
  status,
  onSave,
  onLoad,
  onBurn,
  onNewProject,
  onBrowseProjects,
  onRefresh,
  onSettings,
}: HeaderProps) {
  return (
    <header className="topbar">
      <div className="topbar-left">
        {status.demo_mode && (
          <div className="demo-badge" title="Demo Mode - Simulated data for testing">
            🎮 DEMO
          </div>
        )}
        <div className="ecu-badge">{status.signature || 'NO ECU'}</div>
        {status.ini_name && <div className="ini-badge">{status.ini_name}</div>}
        <div className="connection-status">
          <div className={`status-indicator ${status.demo_mode ? 'demo' : status.state === 'Connected' ? 'connected' : ''}`} />
          {status.demo_mode ? 'Demo Mode' : status.state}
        </div>
      </div>

      <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
        <button
          className="secondary-btn"
          style={{ padding: '0.4rem 0.8rem', fontSize: '0.8rem' }}
          onClick={onSave}
        >
          <Save size={16} style={{ marginRight: '0.3rem' }} /> Save
        </button>
        <button
          className="secondary-btn"
          style={{ padding: '0.4rem 0.8rem', fontSize: '0.8rem' }}
          onClick={onLoad}
        >
          <Database size={16} style={{ marginRight: '0.3rem' }} /> Load
        </button>
        <button
          className="primary-btn"
          style={{ padding: '0.4rem 0.8rem', fontSize: '0.8rem' }}
          onClick={onBurn}
        >
          <Zap size={16} style={{ marginRight: '0.3rem' }} /> Burn
        </button>
        <div style={{ width: '1px', height: '20px', background: 'var(--border)', margin: '0 0.5rem' }} />
        <button className="icon-btn" title="New Project" onClick={onNewProject}>
          <Plus size={18} />
        </button>
        <button className="icon-btn" title="Browse Projects" onClick={onBrowseProjects}>
          <Database size={18} />
        </button>
        <button className="icon-btn" title="Refresh Data" onClick={onRefresh}>
          <Activity size={18} />
        </button>
        <button className="icon-btn" title="Settings" onClick={onSettings}>
          <Settings size={18} />
        </button>
      </div>
    </header>
  );
}

export type { ConnectionStatus };
