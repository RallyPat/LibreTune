import { vi } from 'vitest';
import * as core from '@tauri-apps/api/core';
import * as eventModule from '@tauri-apps/api/event';

/**
 * Test helper: setupTauriMocks(defaultResponses?)
 *
 * Lightweight Tauri mock harness intended for frontend unit/integration tests.
 * Features and guarantees:
 * - Emits events to registered listeners (listeners receive an object shaped like { payload })
 * - Queues events that arrive before a listener is registered and drains them when listeners register
 * - Maintains a global `__TAURI_LISTENERS__` registry so multiple test modules can observe the same events
 * - Provides a predictable `core.invoke` mock (overrideable with `setInvokeResponse`) so tests can simulate Tauri command responses (e.g., connection/connect tests)
 *
 * Returns a handle with these helpers usable from tests:
 * - emit(eventName, payload): emits (or queues) an event
 * - listen(eventName, handler): registers a handler and immediately drains queued events; returns an unlisten function
 * - getQueued(eventName): returns queued payloads for inspection
 * - drainQueued(eventName): delivers queued payloads to registered listeners and clears the queue
 * - setInvokeResponse(cmd, resp): set/override the response for `core.invoke` calls (exact command match)
 *
 * Example usage:
 * const h = setupTauriMocks({ get_settings: { runtime_packet_mode: 'Auto' } });
 * const unlisten = await h.listen('connection:metrics', ev => console.log(ev.payload));
 * h.emit('connection:metrics', { tx_bps: 1024, rx_bps: 2048 });
 * if (unlisten) unlisten();
 */
export type TauriMocksHandle = {
  emit: (eventName: string, payload: any) => void;
  listen?: (eventName: string, handler: (e: any) => void) => Promise<() => void>;
  getQueued?: (eventName: string) => any[];
  drainQueued?: (eventName: string) => void;
  setInvokeResponse: (cmd: string, resp: any) => void;
};


export function setupTauriMocks(defaultResponses: Record<string, any> = {}): TauriMocksHandle {
  // Mark environment as Tauri for code paths that check for it
  (window as any).__TAURI_INTERNALS__ = {};

  // Create a local listeners registry so tests can emit events
  const listeners: Record<string, Array<(e: any) => void>> = {};
  // Expose a global registry so multiple test modules can add listeners and
  // events emitted from any module will be observable across the test harness.
  (window as any).__TAURI_LISTENERS__ = Object.assign((window as any).__TAURI_LISTENERS__ || {}, listeners);

  // Ensure core.invoke is a predictable mock (prefer modifying existing mock implementation)
  try {
    const isMock = core && core.invoke && typeof (core.invoke as any).mockImplementation === 'function';
    const impl = (cmd: string) => {
      if (defaultResponses && Object.prototype.hasOwnProperty.call(defaultResponses, cmd)) {
        return Promise.resolve(defaultResponses[cmd]);
      }
      return Promise.resolve();
    };

    if (isMock) {
      (core.invoke as any).mockReset();
      (core.invoke as any).mockImplementation(impl);
    } else {
      (core as any).invoke = vi.fn(impl);
    }
  } catch (e) {
    // If mocking fails, silently ignore - tests will still be able to override with setInvokeResponse
  }

  // Queued events (emit before listen registered)
  const queued: Record<string, any[]> = {};

  // Ensure event.listen is a predictable mock and route handlers into our registry
  try {
    const event = eventModule;

    // Prefer spyOn so we patch the very function reference that modules imported earlier
    // (this mirrors the unit test approach vi.spyOn(...)). This is more robust than
    // replacing the module property in some bundling scenarios.
    try {
      if (event && typeof event.listen === 'function') {
        try {
          vi.spyOn(event, 'listen').mockImplementation(async (eventName: string, handler: any) => {
            // Delegate to listenImpl (defined below)
            return listenImpl(eventName, handler);
          });
          console.debug('[tauriMocks] spyOn listen');
        } catch (_err) {
          // Some environments may not allow spyOn; fall through to patching
        }
      }
    } catch (_e) {
      // ignore
    }

    const isListenMock = event && event.listen && typeof (event.listen as any).mockImplementation === 'function';
    const listenImpl = async (eventName: string, handler: any) => {
      // Register handler locally
      console.debug('[tauriMocks] listen registered', eventName);
      listeners[eventName] = listeners[eventName] || [];
      listeners[eventName].push(handler);

      // Drain any queued events for this eventName so late listeners still get recent events
      // This mirrors real-world behavior where listeners registering late still see recent updates
      (queued[eventName] || []).forEach((payload) => {
        console.debug('[tauriMocks] delivering queued event', eventName, payload);
        try { handler({ payload }); } catch (_err) { /* ignore */ }
      });
      queued[eventName] = [];

      // Mirror into global registry so other test modules that read __TAURI_LISTENERS__ see the handler
      (window as any).__TAURI_LISTENERS__ = (window as any).__TAURI_LISTENERS__ || {};
      (window as any).__TAURI_LISTENERS__[eventName] = (window as any).__TAURI_LISTENERS__[eventName] || [];
      (window as any).__TAURI_LISTENERS__[eventName].push(handler);

      // Return an unlisten function that removes handler both locally and from the global registry
      return () => {
        listeners[eventName] = listeners[eventName].filter((h) => h !== handler);
        (window as any).__TAURI_LISTENERS__[eventName] = (window as any).__TAURI_LISTENERS__[eventName].filter((h: any) => h !== handler);
      };
    };

    if (isListenMock) {
      (event.listen as any).mockReset();
      (event.listen as any).mockImplementation(listenImpl);
      console.debug('[tauriMocks] patched existing listen mock');
    } else {
      event.listen = vi.fn(listenImpl);
      console.debug('[tauriMocks] replaced listen with mock impl');
    }
  } catch (e) {
    // ignore
  }

  return {
    emit: (eventName: string, payload: any) => {
      const local = listeners[eventName] || [];
      const global = (window as any).__TAURI_LISTENERS__?.[eventName] || [];

      if (local.length || global.length) {
        [...local, ...global].forEach((h: any) => {
          try { h({ payload }); } catch (_err) { /* ignore handler errors */ }
        });
      } else {
        // Queue events that arrive before listeners are registered
        queued[eventName] = queued[eventName] || [];
        queued[eventName].push(payload);
      }
    },
    // Test helpers
    listen: async (eventName: string, handler: (e: any) => void) => {
      listeners[eventName] = listeners[eventName] || [];
      listeners[eventName].push(handler);

      (window as any).__TAURI_LISTENERS__ = (window as any).__TAURI_LISTENERS__ || {};
      (window as any).__TAURI_LISTENERS__[eventName] = (window as any).__TAURI_LISTENERS__[eventName] || [];
      (window as any).__TAURI_LISTENERS__[eventName].push(handler);

      // Drain queued events to this new listener immediately
      (queued[eventName] || []).forEach((payload) => {
        try { handler({ payload }); } catch (_err) { /* ignore */ }
      });
      queued[eventName] = [];

      return () => {
        listeners[eventName] = listeners[eventName].filter((h) => h !== handler);
        (window as any).__TAURI_LISTENERS__[eventName] = (window as any).__TAURI_LISTENERS__[eventName].filter((h: any) => h !== handler);
      };
    },
    getQueued: (eventName: string) => {
      return (queued[eventName] || []).slice();
    },
    drainQueued: (eventName: string) => {
      const q = queued[eventName] || [];
      const local = listeners[eventName] || [];
      const global = (window as any).__TAURI_LISTENERS__?.[eventName] || [];
      q.forEach((payload) => {
        [...local, ...global].forEach((h: any) => {
          try { h({ payload }); } catch (_err) { /* ignore */ }
        });
      });
      queued[eventName] = [];
    },
    setInvokeResponse: (cmd: string, resp: any) => {
      // Replace the `core.invoke` mock such that calls with the exact command string
      // return the provided response. This is useful to simulate backend Tauri
      // command responses (e.g., `connect_to_ecu`, `get_settings`).
      try {
        (core as any).invoke = vi.fn((c: string) => (c === cmd ? Promise.resolve(resp) : Promise.resolve()));
      } catch (_e) { /* ignore */ }
    },
  };
}

export function tearDownTauriMocks() {
  try {
    delete (window as any).__TAURI_INTERNALS__;
    const coreModule = core;
    const event = eventModule;

    // Reset to default mock functions if available
    if (coreModule && typeof (coreModule as any).invoke === 'function' && ((coreModule as any).invoke as any).mockReset) ((coreModule as any).invoke as any).mockReset();
    if (event && typeof event.listen === 'function' && (event.listen as any).mockReset) (event.listen as any).mockReset();
  } catch (e) {
    // ignore
  }
}
