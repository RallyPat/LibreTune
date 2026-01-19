import { ReactNode, useState, useCallback } from 'react';
import { MenuBar } from './MenuBar';
import { Toolbar } from './Toolbar';
import { TabBar, Tab } from './TabBar';
import { Sidebar } from './Sidebar';
import { StatusBar } from './StatusBar';
import './TunerLayout.css';

export interface TunerLayoutProps {
  // Menu configuration
  menuItems: MenuItem[];
  
  // Toolbar configuration
  toolbarItems: ToolbarItem[];
  
  // Tab configuration
  tabs: Tab[];
  activeTabId: string | null;
  onTabSelect: (tabId: string) => void;
  onTabClose: (tabId: string) => void;
  onTabReorder?: (tabs: Tab[]) => void;
  onTabPopout?: (tabId: string) => void;
  
  // Sidebar configuration
  sidebarItems: SidebarNode[];
  sidebarVisible: boolean;
  onSidebarToggle: () => void;
  /** Callback when an item is selected. highlightTerm is the search query if user clicked from search results. */
  onSidebarItemSelect: (item: SidebarNode, highlightTerm?: string) => void;
  /** Index of searchable content for deep search (target -> terms) */
  searchIndex?: Record<string, string[]>;
  
  // Status bar
  statusItems: StatusItem[];

  // Connection status
  connected: boolean;
  ecuName?: string;

  // Unit system
  unitsSystem?: 'metric' | 'imperial';

  // Content
  children: ReactNode;
}

export interface MenuItem {
  id: string;
  label: string;
  accelerator?: string; // e.g., "&File" means Alt+F
  items?: MenuItem[];
  separator?: boolean;
  disabled?: boolean;
  checked?: boolean;
  onClick?: () => void;
}

export interface ToolbarItem {
  id: string;
  icon: string;
  tooltip: string;
  disabled?: boolean;
  active?: boolean;
  separator?: boolean;
  onClick?: () => void;
  /** Optional custom content to render instead of a standard icon button */
  content?: React.ReactNode;
}

export interface SidebarNode {
  id: string;
  label: string;
  icon?: string;
  children?: SidebarNode[];
  expanded?: boolean;
  type?: 'folder' | 'table' | 'dialog' | 'dashboard' | 'log' | 'help';
  data?: unknown;
  /** Whether item is disabled (visibility condition evaluated to false) */
  disabled?: boolean;
  /** Tooltip explaining why item is disabled */
  disabledReason?: string;
}

export interface StatusItem {
  id: string;
  content: ReactNode;
  align?: 'left' | 'center' | 'right';
  width?: number | string;
  onClick?: () => void;
}

export function TunerLayout({
  menuItems,
  toolbarItems,
  tabs,
  activeTabId,
  onTabSelect,
  onTabClose,
  onTabReorder,
  onTabPopout,
  sidebarItems,
  sidebarVisible,
  onSidebarToggle: _onSidebarToggle,
  onSidebarItemSelect,
  searchIndex,
  statusItems,
  connected,
  ecuName,
  unitsSystem,
  children,
}: TunerLayoutProps) {
  const [sidebarWidth, setSidebarWidth] = useState(240);

  const handleSidebarResize = useCallback((newWidth: number) => {
    setSidebarWidth(Math.max(150, Math.min(400, newWidth)));
  }, []);

  return (
    <div className="tuner-layout">
      {/* Menu Bar */}
      <MenuBar items={menuItems} />
      
      {/* Toolbar */}
      <Toolbar items={toolbarItems} />
      
      {/* Main content area */}
      <div className="tuner-layout-main">
        {/* Sidebar */}
        {sidebarVisible && (
          <Sidebar
            items={sidebarItems}
            width={sidebarWidth}
            onResize={handleSidebarResize}
            onItemSelect={onSidebarItemSelect}
            searchIndex={searchIndex}
          />
        )}
        
        {/* Document area */}
        <div className="tuner-layout-documents">
          {/* Tab bar */}
          <TabBar
            tabs={tabs}
            activeTabId={activeTabId}
            onTabSelect={onTabSelect}
            onTabClose={onTabClose}
            onTabReorder={onTabReorder}
            onTabPopout={onTabPopout}
          />
          
          {/* Tab content */}
          <div className="tuner-layout-content">
            {children}
          </div>
        </div>
      </div>
      
      {/* Status Bar */}
      <StatusBar
        items={statusItems}
        connected={connected}
        ecuName={ecuName}
        unitsSystem={unitsSystem}
      />
    </div>
  );
}
