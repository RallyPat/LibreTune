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
          <div className="demo-badge" title="演示模式 - 用于测试的模拟数据">
            🎮 演示
          </div>
        )}
        <div className="ecu-badge">{status.signature || '无 ECU'}</div>
        {status.ini_name && <div className="ini-badge">{status.ini_name}</div>}
        <div className="connection-status">
          <div className={`status-indicator ${status.demo_mode ? 'demo' : status.state === 'Connected' ? 'connected' : ''}`} />
          {status.demo_mode ? '演示模式' : status.state === 'Connected' ? '已连接' : status.state === 'Disconnected' ? '已断开' : status.state === 'Connecting' ? '连接中' : status.state}
        </div>
      </div>

      <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
        <button
          className="secondary-btn"
          style={{ padding: '0.4rem 0.8rem', fontSize: '0.8rem' }}
          onClick={onSave}
        >
          <Save size={16} style={{ marginRight: '0.3rem' }} /> 保存
        </button>
        <button
          className="secondary-btn"
          style={{ padding: '0.4rem 0.8rem', fontSize: '0.8rem' }}
          onClick={onLoad}
        >
          <Database size={16} style={{ marginRight: '0.3rem' }} /> 加载
        </button>
        <button
          className="primary-btn"
          style={{ padding: '0.4rem 0.8rem', fontSize: '0.8rem' }}
          onClick={onBurn}
        >
          <Zap size={16} style={{ marginRight: '0.3rem' }} /> 烧录
        </button>
        <div style={{ width: '1px', height: '20px', background: 'var(--border)', margin: '0 0.5rem' }} />
        <button className="icon-btn" title="新建项目" onClick={onNewProject}>
          <Plus size={18} />
        </button>
        <button className="icon-btn" title="浏览项目" onClick={onBrowseProjects}>
          <Database size={18} />
        </button>
        <button className="icon-btn" title="刷新数据" onClick={onRefresh}>
          <Activity size={18} />
        </button>
        <button className="icon-btn" title="设置" onClick={onSettings}>
          <Settings size={18} />
        </button>
      </div>
    </header>
  );
}

export type { ConnectionStatus };
