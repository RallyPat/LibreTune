/**
 * Event bridge for bidirectional Swing ↔ React communication.
 */

import { PluginEvent } from './types';
import { invoke } from '@tauri-apps/api/core';

export type EventCallback = (event: PluginEvent) => void;

/**
 * Manages event communication between React and the JVM plugin host.
 */
export class EventBridge {
  private listeners: Map<string, Set<EventCallback>> = new Map();

  /**
   * Send an event to a plugin.
   */
  async sendEvent(pluginId: string, event: PluginEvent): Promise<void> {
    try {
      await invoke('send_plugin_event', { pluginId, event });
    } catch (error) {
      console.error('Failed to send plugin event:', error);
      throw error;
    }
  }

  /**
   * Send a button click/action event.
   */
  async sendAction(pluginId: string, componentId: string): Promise<void> {
    await this.sendEvent(pluginId, {
      type: 'action',
      componentId,
    });
  }

  /**
   * Send a text change event.
   */
  async sendTextChange(pluginId: string, componentId: string, text: string): Promise<void> {
    await this.sendEvent(pluginId, {
      type: 'textChange',
      componentId,
      text,
    });
  }

  /**
   * Send a checkbox/radio state change event.
   */
  async sendStateChange(pluginId: string, componentId: string, selected: boolean): Promise<void> {
    await this.sendEvent(pluginId, {
      type: 'stateChange',
      componentId,
      selected,
    });
  }

  /**
   * Send a combobox selection event.
   */
  async sendItemSelect(
    pluginId: string,
    componentId: string,
    selectedIndex: number,
    selectedItem: string
  ): Promise<void> {
    await this.sendEvent(pluginId, {
      type: 'itemSelect',
      componentId,
      selectedIndex,
      selectedItem,
    });
  }

  /**
   * Send a slider value change event.
   */
  async sendSliderChange(pluginId: string, componentId: string, value: number): Promise<void> {
    await this.sendEvent(pluginId, {
      type: 'sliderChange',
      componentId,
      value,
    });
  }

  /**
   * Send a table cell edit event.
   */
  async sendTableEdit(
    pluginId: string,
    componentId: string,
    row: number,
    column: number,
    value: string
  ): Promise<void> {
    await this.sendEvent(pluginId, {
      type: 'tableEdit',
      componentId,
      row,
      column,
      value,
    });
  }

  /**
   * Send a table selection event.
   */
  async sendTableSelect(
    pluginId: string,
    componentId: string,
    selectedRows: number[]
  ): Promise<void> {
    await this.sendEvent(pluginId, {
      type: 'tableSelect',
      componentId,
      selectedRows,
    });
  }

  /**
   * Subscribe to events from a plugin.
   */
  subscribe(pluginId: string, callback: EventCallback): () => void {
    if (!this.listeners.has(pluginId)) {
      this.listeners.set(pluginId, new Set());
    }
    this.listeners.get(pluginId)!.add(callback);

    return () => {
      this.listeners.get(pluginId)?.delete(callback);
    };
  }

  /**
   * Handle an incoming event from the plugin host.
   */
  handleIncomingEvent(pluginId: string, event: PluginEvent): void {
    const callbacks = this.listeners.get(pluginId);
    if (callbacks) {
      for (const callback of callbacks) {
        try {
          callback(event);
        } catch (error) {
          console.error('Error in event callback:', error);
        }
      }
    }
  }
}

// Singleton instance
export const eventBridge = new EventBridge();
