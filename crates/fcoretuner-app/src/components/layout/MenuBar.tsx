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
    name: '文件',
    children: [
      { name: '新建项目...', target: 'newProject', hotkey: 'Ctrl+N' },
      { name: '打开项目...', target: 'browseProjects', hotkey: 'Ctrl+O' },
      { separator: true, name: 'sep1' },
      { name: '保存调教', target: 'save', hotkey: 'Ctrl+S' },
      { name: '加载调教...', target: 'load' },
      { name: '烧录到 ECU', target: 'burn', hotkey: 'Ctrl+B' },
      { separator: true, name: 'sep2' },
      { name: '设置', target: 'settings' },
    ]
  },
  {
    name: '编辑',
    children: [
      { name: '撤销', target: 'undo', hotkey: 'Ctrl+Z' },
      { name: '重做', target: 'redo', hotkey: 'Ctrl+Y' },
      { separator: true, name: 'sep1' },
      { name: '剪切', target: 'cut', hotkey: 'Ctrl+X' },
      { name: '复制', target: 'copy', hotkey: 'Ctrl+C' },
      { name: '粘贴', target: 'paste', hotkey: 'Ctrl+V' },
    ]
  },
  {
    name: '视图',
    children: [
      { name: '仪表盘', target: 'std_realtime' },
      { name: '数据记录器', target: 'dataLogger' },
      { separator: true, name: 'sep1' },
      { name: '全屏', target: 'fullScreen', hotkey: 'F11' },
    ]
  },
  {
    name: '调校',
    children: [
      { name: '实时自动调校', target: 'autoTune', hotkey: 'Ctrl+A' },
      { name: '性能计算器', target: 'performance' },
      { separator: true, name: 'sep1' },
      { name: '齿记录器', target: 'toothLogger' },
      { name: '复合记录器', target: 'compositeLogger' },
    ]
  },
  {
    name: '工具',
    children: [
      { name: '动作管理器', target: 'actions' },
      { name: '表格比较', target: 'tableCompare' },
      { separator: true, name: 'sep1' },
      { name: '重置为默认值', target: 'resetDefaults' },
    ]
  },
  {
    name: '帮助',
    children: [
      { name: '文档', target: 'docs' },
      { name: '键盘快捷键', target: 'shortcuts' },
      { separator: true, name: 'sep1' },
      { name: '关于 FCoreTuner', target: 'about' },
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
    const tuningMenu = allMenus.find(m => m.name === '调校');
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
