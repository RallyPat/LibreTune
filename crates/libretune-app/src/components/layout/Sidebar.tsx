import { Activity, ChevronRight, Database, Grid3X3, MoreHorizontal, Play, Search, Settings, Terminal } from 'lucide-react';

interface MenuItem {
  type: 'Dialog' | 'Table' | 'SubMenu' | 'Separator';
  label: string;
  target?: string;
  items?: MenuItem[];
  condition?: string;
}

interface Menu {
  name: string;
  title: string;
  items: MenuItem[];
}

export interface SidebarProps {
  activeTab: string;
  menuTree: Menu[];
  filteredMenuTree: Menu[];
  expandedMenus: Record<string, boolean>;
  menuSearch: string;
  onTabChange: (tab: string) => void;
  onMenuSearch: (search: string) => void;
  onMenuExpand: (menuName: string) => void;
  onMenuTargetOpen: (target: string) => void;
  onAutoTune: () => void;
  onPerformance: () => void;
  onActions: () => void;
}

export default function Sidebar({
  activeTab,
  menuTree,
  filteredMenuTree,
  expandedMenus,
  menuSearch,
  onTabChange,
  onMenuSearch,
  onMenuExpand,
  onMenuTargetOpen,
  onAutoTune,
  onPerformance,
  onActions,
}: SidebarProps) {
  return (
    <aside className="sidebar">
      <div
        className="logo-section"
        onClick={() => {
          onTabChange('dashboard');
        }}
      >
        <Activity className="logo-icon" />
        <span className="app-name">LibreTune</span>
      </div>

      <nav className="nav-links">
        <div
          className={`nav-item ${activeTab === 'dashboard' ? 'active' : ''}`}
          onClick={() => onTabChange('dashboard')}
        >
          <Grid3X3 className="nav-icon" />
          <span>Dashboard</span>
        </div>
        <div className="nav-item" onClick={onAutoTune}>
          <Play className="nav-icon" />
          <span>AutoTune</span>
        </div>
        <div className="nav-item" onClick={onPerformance}>
          <Activity className="nav-icon" />
          <span>Performance</span>
        </div>
        <div className="nav-item" onClick={onActions}>
          <MoreHorizontal className="nav-icon" />
          <span>Actions</span>
        </div>

        <div
          className={`nav-item ${activeTab === 'project' ? 'active' : ''}`}
          onClick={() => onTabChange('project')}
        >
          <Database className="nav-icon" />
          <span>Project</span>
        </div>

        {menuTree.length > 0 && (
          <>
            <div className="sidebar-divider">TUNING</div>
            <div className="sidebar-search-container">
              <Search size={14} className="search-icon" />
              <input
                type="text"
                placeholder="Quick search..."
                value={menuSearch}
                onChange={(e) => onMenuSearch(e.target.value)}
              />
            </div>
          </>
        )}

        {filteredMenuTree.map((menu) => {
          const isExpanded = expandedMenus[menu.name] || menuSearch.length > 0;
          return (
            <div key={menu.name} className="nav-group">
              <div
                className={`nav-item group-header ${isExpanded ? 'active expanded' : ''}`}
                onClick={() => onMenuExpand(menu.name)}
              >
                <Settings className="nav-icon" />
                <span>{menu.title.replace('&', '')}</span>
                <ChevronRight size={14} className={`group-chevron ${isExpanded ? 'rotated' : ''}`} />
              </div>
              {isExpanded && (
                <div className="group-children">
                  {menu.items.map((item, idx) => {
                    if (item.type === 'Separator') return <div key={idx} className="menu-separator" />;
                    return (
                      <div
                        key={idx}
                        className="sub-nav-item"
                        onClick={() => {
                          if (item.target) {
                            onMenuTargetOpen(item.target);
                          }
                        }}
                      >
                        {item.label.replace('&', '')}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}

        <div className="sidebar-divider">SYSTEM</div>

        <div
          className={`nav-item ${activeTab === 'settings' ? 'active' : ''}`}
          onClick={() => onTabChange('settings')}
        >
          <Terminal className="nav-icon" />
          <span>Settings</span>
        </div>
      </nav>

      <div style={{ padding: '0.5rem', opacity: 0.4, fontSize: '0.7rem', textAlign: 'center' }}>
        ALPHA v0.1.0
      </div>
    </aside>
  );
}

export type { Menu, MenuItem };
