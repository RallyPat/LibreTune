/**
 * Hotkey Service
 * 
 * Provides a singleton HotkeyManager instance for use throughout the application.
 * Handles loading and managing custom keyboard bindings from backend storage.
 */

import { HotkeyManager } from '../components/HotkeyManager';

let hotkeyManagerInstance: HotkeyManager | null = null;

/**
 * Get the global HotkeyManager singleton instance.
 * @returns HotkeyManager instance
 */
export function getHotkeyManager(): HotkeyManager {
  if (!hotkeyManagerInstance) {
    hotkeyManagerInstance = new HotkeyManager();
  }
  return hotkeyManagerInstance;
}

/**
 * Initialize the global HotkeyManager by loading custom bindings from storage.
 * Should be called once during app initialization.
 */
export async function initializeHotkeyManager(): Promise<void> {
  const manager = getHotkeyManager();
  await manager.loadCustomBindings();
}

/**
 * Reset the HotkeyManager instance (useful for testing).
 */
export function resetHotkeyManager(): void {
  hotkeyManagerInstance = null;
}
