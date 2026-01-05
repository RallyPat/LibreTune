//! Menu Manager Component
//!
//! Handles dynamic menu system parsed from ECU definition INI files.
//! Supports hierarchical menus with conditions and submenus.

export interface MenuItem {
  id: string;
  label: string;
  shortcut?: string;
  target?: string;
  condition?: string;
  icon?: string;
  children?: MenuItem[];
  type?: "dialog" | "table" | "submenu" | "separator" | "action";
}

export interface MenuGroup {
  name: string;
  title: string;
  items: MenuItem[];
  defaultExpanded?: boolean;
}

export class MenuManager {
  private groups: MenuGroup[];
  private expandedGroups: string[];
  // @ts-expect-error - Reserved for future dialog state management
  private _currentDialog: string | null;
  
  constructor() {
    this.groups = [];
    this.expandedGroups = [];
    this._currentDialog = null;
  }
  
  public loadFromIni(groups: MenuGroup[]): void {
    this.groups = groups;
  }
  
  public findItem(id: string): MenuItem | undefined {
    for (const group of this.groups) {
      for (const item of group.items) {
        if (item.target === id) {
          return item;
        }
        if (item.children) {
          for (const child of item.children) {
            if (child.target === id) {
              return child;
            }
          }
        }
      }
    }
    return undefined;
  }
  
  public toggleGroup(name: string): void {
    const index = this.expandedGroups.indexOf(name);
    if (index !== -1) {
      this.expandedGroups.splice(index, 1);
    } else {
      this.expandedGroups.push(name);
    }
  }
}
