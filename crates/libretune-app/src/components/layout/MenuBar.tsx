import { useState, useRef, useEffect } from 'react';
import './MenuBar.css';

interface MenuItem {
  name: string;
  target?: string;
  children?: MenuItem[];
  separator?: boolean;
  hotkey?: string;
}

interface MenuBarProps {
  menuTree: MenuItem[];
  onMenuSelect: (target: string) => void;
  onSave?: () => void;
  onLoad?: () => void;
  onBurn?: () => void;
  onNewProject?: () => void;
  onBrowseProjects?: () => void;
  onSettings?: () => void;
  onAutoTune?: () => void;
  onPerformance?: () => void;
  onActions?: () => void;
}

// Standard menu items (File, Edit, View, etc.)
const standardMenus: MenuItem[] = [
  {
    name: 'File',
    children: [
      { name: 'New Project...', target: 'newProject', hotkey: 'Ctrl+N' },
      { name: 'Open Project...', target: 'browseProjects', hotkey: 'Ctrl+O' },
      { separator: true, name: 'sep1' },
      { name: 'Save Tune', target: 'save', hotkey: 'Ctrl+S' },
      { name: 'Load Tune...', target: 'load' },
      { name: 'Burn to ECU', target: 'burn', hotkey: 'Ctrl+B' },
      { separator: true, name: 'sep2' },
      { name: 'Settings', target: 'settings' },
    ]
  },
  {
    name: 'Edit',
    children: [
      { name: 'Undo', target: 'undo', hotkey: 'Ctrl+Z' },
      { name: 'Redo', target: 'redo', hotkey: 'Ctrl+Y' },
      { separator: true, name: 'sep1' },
      { name: 'Cut', target: 'cut', hotkey: 'Ctrl+X' },
      { name: 'Copy', target: 'copy', hotkey: 'Ctrl+C' },
      { name: 'Paste', target: 'paste', hotkey: 'Ctrl+V' },
    ]
  },
  {
    name: 'View',
    children: [
      { name: 'Dashboard', target: 'std_realtime' },
      { name: 'Data Logger', target: 'dataLogger' },
      { separator: true, name: 'sep1' },
      { name: 'Full Screen', target: 'fullScreen', hotkey: 'F11' },
    ]
  },
  {
    name: 'Tuning',
    children: [
      { name: 'AutoTune', target: 'autoTune', hotkey: 'Ctrl+A' },
      { name: 'Performance Calculator', target: 'performance' },
      { separator: true, name: 'sep1' },
      { name: 'Tooth Logger', target: 'toothLogger' },
      { name: 'Composite Logger', target: 'compositeLogger' },
    ]
  },
  {
    name: 'Tools',
    children: [
      { name: 'Action Manager', target: 'actions' },
      { name: 'Table Comparison', target: 'tableCompare' },
      { separator: true, name: 'sep1' },
      { name: 'Reset to Defaults', target: 'resetDefaults' },
    ]
  },
  {
    name: 'Help',
    children: [
      { name: 'Documentation', target: 'docs' },
      { name: 'Keyboard Shortcuts', target: 'shortcuts' },
      { separator: true, name: 'sep1' },
      { name: 'About LibreTune', target: 'about' },
    ]
  },
];

export function MenuBar({
  menuTree,
  onMenuSelect,
  onSave,
  onLoad,
  onBurn,
  onNewProject,
  onBrowseProjects,
  onSettings,
  onAutoTune,
  onPerformance,
  onActions,
}: MenuBarProps) {
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const [openSubmenu, setOpenSubmenu] = useState<string | null>(null);
  const menuBarRef = useRef<HTMLDivElement>(null);

  // Close menus when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuBarRef.current && !menuBarRef.current.contains(e.target as Node)) {
        setOpenMenu(null);
        setOpenSubmenu(null);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleMenuClick = (menuName: string) => {
    setOpenMenu(openMenu === menuName ? null : menuName);
    setOpenSubmenu(null);
  };

  const handleMenuHover = (menuName: string) => {
    if (openMenu !== null) {
      setOpenMenu(menuName);
      setOpenSubmenu(null);
    }
  };

  const handleItemClick = (item: MenuItem) => {
    if (item.separator) return;
    
    // Handle built-in actions
    switch (item.target) {
      case 'save': onSave?.(); break;
      case 'load': onLoad?.(); break;
      case 'burn': onBurn?.(); break;
      case 'newProject': onNewProject?.(); break;
      case 'browseProjects': onBrowseProjects?.(); break;
      case 'settings': onSettings?.(); break;
      case 'autoTune': onAutoTune?.(); break;
      case 'performance': onPerformance?.(); break;
      case 'actions': onActions?.(); break;
      default:
        if (item.target) {
          onMenuSelect(item.target);
        }
    }
    
    setOpenMenu(null);
    setOpenSubmenu(null);
  };

  // Merge standard menus with INI-driven menus
  const allMenus = [...standardMenus];
  
  // Add INI menus under "Tuning" submenu
  if (menuTree.length > 0) {
    const tuningMenu = allMenus.find(m => m.name === 'Tuning');
    if (tuningMenu && tuningMenu.children) {
      tuningMenu.children.push({ separator: true, name: 'sep-ini' });
      menuTree.forEach(iniMenu => {
        tuningMenu.children!.push({
          name: iniMenu.name,
          target: iniMenu.target,
          children: iniMenu.children,
        });
      });
    }
  }

  const renderMenuItem = (item: MenuItem, parentPath: string = '', index: number = 0) => {
    const itemKey = item.separator ? `${parentPath}/sep-${index}` : `${parentPath}/${item.name}`;
    
    if (item.separator) {
      return <div key={itemKey} className="menu-separator" />;
    }

    const hasChildren = item.children && item.children.length > 0;
    const isSubmenuOpen = openSubmenu === itemKey;

    return (
      <div
        key={itemKey}
        className={`menu-item ${hasChildren ? 'has-submenu' : ''} ${isSubmenuOpen ? 'submenu-open' : ''}`}
        onClick={() => !hasChildren && handleItemClick(item)}
        onMouseEnter={() => hasChildren && setOpenSubmenu(itemKey)}
        onMouseLeave={() => hasChildren && setOpenSubmenu(null)}
      >
        <span className="menu-item-label">{item.name}</span>
        {item.hotkey && <span className="menu-item-hotkey">{item.hotkey}</span>}
        {hasChildren && <span className="menu-item-arrow">▶</span>}
        
        {hasChildren && isSubmenuOpen && (
          <div className="submenu-dropdown">
            {item.children!.map((child, idx) => renderMenuItem(child, itemKey, idx))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="menubar" ref={menuBarRef}>
      {allMenus.map(menu => (
        <div
          key={menu.name}
          className={`menubar-item ${openMenu === menu.name ? 'active' : ''}`}
          onClick={() => handleMenuClick(menu.name)}
          onMouseEnter={() => handleMenuHover(menu.name)}
        >
          {menu.name}
          
          {openMenu === menu.name && menu.children && (
            <div className="menu-dropdown">
              {menu.children.map((item, idx) => renderMenuItem(item, menu.name, idx))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

export default MenuBar;
