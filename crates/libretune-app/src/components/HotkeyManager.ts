//! Hot Key Manager
//!
//! Manages global keyboard shortcuts for table editing, dialogs, and navigation.
//! Based on standard ECU tuning software keyboard shortcuts.

import { invoke } from "@tauri-apps/api/core";

export interface Hotkey {
  keys: string;
  description: string;
}

export class HotkeyManager {
  private shortcuts: Record<string, Hotkey>;
  private customBindings: Record<string, string> = {};
  
  constructor() {
    this.shortcuts = {};
    this.initializeShortcuts();
  }
  
  /**
   * Load custom hotkey bindings from storage.
   * Call this after constructing the manager.
   */
  async loadCustomBindings(): Promise<void> {
    try {
      this.customBindings = await invoke<Record<string, string>>('get_hotkey_bindings');
    } catch (error) {
      console.warn('Failed to load custom hotkey bindings:', error);
      this.customBindings = {};
    }
  }
  
  private initializeShortcuts(): void {
    // Table navigation (2D Table help.html)
    this.shortcuts["ArrowUp"] = {
      keys: "Up, Down, Left, Right",
      description: "Navigate cells in table",
    };
    this.shortcuts["ArrowDown"] = {
      keys: "Down",
      description: "Navigate down in table",
    };
    this.shortcuts["ArrowLeft"] = {
      keys: "Left",
      description: "Navigate left in table",
    };
    this.shortcuts["ArrowRight"] = {
      keys: "Right",
      description: "Navigate right in table",
    };
    
    // Table editing shortcuts
    this.shortcuts["="] = {
      keys: "=",
      description: "Set selected cells to user parameter value",
    };
    this.shortcuts[">"] = {
      keys: ">, +, =, ., q",
      description: "Increase selected cells by increment amount",
    };
    this.shortcuts["<"] = {
      keys: "<, -, ,, _",
      description: "Decrease selected cells by increment amount",
    };
    this.shortcuts["*"] = {
      keys: "*",
      description: "Scale selected cells by user set amount",
    };
    this.shortcuts["/"] = {
      keys: "/",
      description: "Interpolate selected cells between corners",
    };
    this.shortcuts["s"] = {
      keys: "s",
      description: "Smooth table by smoothing factor",
    };
    
    // Dialog shortcuts
    this.shortcuts["Ctrl+S"] = {
      keys: "Ctrl+S",
      description: "Save current dialog",
    };
    this.shortcuts["Ctrl+Z"] = {
      keys: "Ctrl+Z",
      description: "Undo last operation",
    };
    this.shortcuts["Ctrl+Y"] = {
      keys: "Ctrl+Y",
      description: "Redo last operation",
    };
    this.shortcuts["Escape"] = {
      keys: "Escape",
      description: "Cancel current operation or close dialog",
    };
    
    // Navigation shortcuts
    this.shortcuts["F"] = {
      keys: "F",
      description: "Jump to current position or enter fullscreen",
    };
    this.shortcuts["Tab"] = {
      keys: "Tab",
      description: "Switch to next view or table",
    };
    
    // Table-specific shortcuts
    this.shortcuts["G"] = {
      keys: "G",
      description: "Jump to active position",
    };
    this.shortcuts["F"] = {
      keys: "F",
      description: "Toggle Follow Mode on/off",
    };
    this.shortcuts["M"] = {
      keys: "M",
      description: "Increase Yaw angle by 10°",
    };
    this.shortcuts["K"] = {
      keys: "K",
      description: "Decrease Yaw angle by 10°",
    };
    this.shortcuts["N"] = {
      keys: "N",
      description: "Increase Roll angle by 10°",
    };
    this.shortcuts["J"] = {
      keys: "J",
      description: "Decrease Roll angle by 10°",
    };
    this.shortcuts["Z"] = {
      keys: "Z",
      description: "Show top-down view of table",
    };
    this.shortcuts["Ctrl+C"] = {
      keys: "Ctrl+C",
      description: "Copy selected cells",
    };
    this.shortcuts["Ctrl+V"] = {
      keys: "Ctrl+V",
      description: "Paste cells",
    };
    this.shortcuts["Ctrl+Shift+>"] = {
      keys: "Ctrl+Shift+>",
      description: "Multiple increment by user set amount",
    };
    this.shortcuts["Ctrl+Shift+<"] = {
      keys: "Ctrl+Shift+<",
      description: "Multiple decrement by user set amount",
    };
  }
  
  public getShortcut(keys: string): Hotkey | undefined {
    return this.shortcuts[keys];
  }
  
  /**
   * Get the effective key binding for an action.
   * Returns custom binding if set, otherwise returns the action ID as default.
   * 
   * @param actionId - The action identifier (e.g., 'table.navigateUp')
   * @returns The key binding (e.g., 'ArrowUp')
   */
  public getBindingForAction(actionId: string): string {
    return this.customBindings[actionId] || '';
  }
  
  /**
   * Check if a key combination matches an action's binding.
   * 
   * @param actionId - The action identifier
   * @param keyCombo - The key combination from keyboard event (e.g., 'ArrowUp')
   * @returns true if the key matches the bound shortcut for this action
   */
  public matchesBinding(actionId: string, keyCombo: string): boolean {
    const bound = this.getBindingForAction(actionId);
    return bound === keyCombo;
  }
  
  public getAllShortcuts(): Record<string, Hotkey> {
    return { ...this.shortcuts };
  }
  
  /**
   * Get all custom bindings.
   */
  public getCustomBindings(): Record<string, string> {
    return { ...this.customBindings };
  }
}
