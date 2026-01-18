# Tauri Test Helpers (tauriMocks)

This folder contains test utilities for stubbing and simulating Tauri behaviors in unit and integration tests.

## `setupTauriMocks(defaultResponses?)` 🔧
Creates a test harness for Tauri APIs and returns a handle with the following helpers:

- `emit(eventName: string, payload: any)`
  - Emit an event to all registered listeners. If no listeners are present, the payload is queued for delivery.

- `listen(eventName: string, handler: (event: { payload: any }) => void) -> Promise<unlistenFn>`
  - Registers a handler and immediately drains any queued payloads for that event to the new handler. Returns a function to unlisten.

- `getQueued(eventName: string) -> any[]` (optional)
  - Inspect the currently queued payloads for the given event.

- `drainQueued(eventName: string)` (optional)
  - Manually deliver queued payloads to currently registered listeners and clear the queue.

- `setInvokeResponse(cmd: string, resp: any)`
  - Override the response for `@tauri-apps/api/core` `invoke` calls during a test.

## Usage examples
```ts
const h = setupTauriMocks({ get_settings: { runtime_packet_mode: 'Auto' } });
const unlisten = await h.listen('connection:metrics', ev => console.log(ev.payload));
h.emit('connection:metrics', { tx_bps: 1024, rx_bps: 2048 });
if (unlisten) unlisten();
```

## Notes
- The helper patches `@tauri-apps/api/event.listen` so code that imports `listen` from that module will use the mocked implementation automatically (tests still may use `h.listen` directly).
- `emit` queues events when listeners are not yet registered; use `getQueued`/`drainQueued` to inspect/deliver queued payloads in complex test flows.

If you'd like, I can add example tests that use `setupTauriMocks` across more test files.