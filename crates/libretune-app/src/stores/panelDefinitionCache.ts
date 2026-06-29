import { invoke } from '@tauri-apps/api/core';
import type { DialogDefinition } from '../components/dialogs/types';

const cache = new Map<string, DialogDefinition>();
const inflight = new Map<string, Promise<DialogDefinition | null>>();

/** Incremented on clear — stale in-flight writes are ignored. */
let cacheGeneration = 0;

const PANEL_IPC_TIMEOUT_MS = 15_000;
const MAX_LOW_PRIORITY_FETCHES = 2;
let lowPriorityActive = 0;
const lowPriorityWaiters: Array<() => void> = [];

type FetchPriority = 'high' | 'low';

function withTimeout<T>(promise: Promise<T>, label: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`${label} timed out`)),
      PANEL_IPC_TIMEOUT_MS,
    );
    promise
      .then((v) => { clearTimeout(timer); resolve(v); })
      .catch((e) => { clearTimeout(timer); reject(e); });
  });
}

async function acquireLowPrioritySlot(): Promise<void> {
  if (lowPriorityActive < MAX_LOW_PRIORITY_FETCHES) {
    lowPriorityActive++;
    return;
  }
  await new Promise<void>((resolve) => {
    lowPriorityWaiters.push(resolve);
  });
  lowPriorityActive++;
}

function releaseLowPrioritySlot(): void {
  lowPriorityActive = Math.max(0, lowPriorityActive - 1);
  const next = lowPriorityWaiters.shift();
  if (next) next();
}

async function invokeDialogDefinition(
  name: string,
  priority: FetchPriority,
): Promise<DialogDefinition> {
  if (priority === 'low') {
    await acquireLowPrioritySlot();
  }
  try {
    return await withTimeout(
      invoke<DialogDefinition>('get_dialog_definition', { name }),
      `Panel ${name}`,
    );
  } finally {
    if (priority === 'low') {
      releaseLowPrioritySlot();
    }
  }
}

export function getCachedPanelDefinition(name: string): DialogDefinition | undefined {
  return cache.get(name);
}

export function cachePanelDefinition(name: string, def: DialogDefinition): void {
  cache.set(name, def);
}

/** Background prefetch — throttled and idle-deferred so navigation stays responsive. */
export function deferPrefetchNestedPanels(parent: DialogDefinition): void {
  const panelNames = (parent.components ?? [])
    .filter((c) => c.type === 'Panel' && c.name)
    .map((c) => c.name!);
  if (panelNames.length === 0) return;

  const run = () => {
    for (const panelName of panelNames) {
      void fetchPanelDefinition(panelName);
    }
  };

  if (typeof requestIdleCallback !== 'undefined') {
    requestIdleCallback(run, { timeout: 3000 });
  } else {
    setTimeout(run, 500);
  }
}

function startFetch(name: string, priority: FetchPriority): Promise<DialogDefinition | null> {
  const gen = cacheGeneration;
  const task = invokeDialogDefinition(name, priority)
    .then(async (def) => {
      if (gen !== cacheGeneration) return null;
      cache.set(name, def);
      const { prefetchFieldsForDefinition } = await import('./constantsMetadataCache');
      void prefetchFieldsForDefinition(def);
      return def;
    })
    .catch((err) => {
      // Suppress expected "not found" errors — panel names can be tables/curves, not dialogs.
      const msg = String(err);
      if (!msg.includes('not found') && !msg.includes('Not found')) {
        console.warn(`[panelDefinitionCache] Failed to load '${name}':`, err);
      }
      return null;
    })
    .finally(() => {
      inflight.delete(name);
    });

  inflight.set(name, task);
  return task;
}

/** Low-priority fetch for background prefetch / expanded panels. */
export function fetchPanelDefinition(name: string): Promise<DialogDefinition | null> {
  const hit = cache.get(name);
  if (hit) return Promise.resolve(hit);

  const pending = inflight.get(name);
  if (pending) return pending;

  return startFetch(name, 'low');
}

/** High-priority fetch for active tab / user navigation. */
export function fetchPanelDefinitionPriority(name: string): Promise<DialogDefinition | null> {
  const hit = cache.get(name);
  if (hit) return Promise.resolve(hit);

  const pending = inflight.get(name);
  if (pending) return pending;

  return startFetch(name, 'high');
}

export function clearPanelDefinitionCache(): void {
  cacheGeneration++;
  cache.clear();
  inflight.clear();
}
