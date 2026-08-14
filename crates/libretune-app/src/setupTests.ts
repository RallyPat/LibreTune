import '@testing-library/jest-dom';

// Provide a minimal localStorage stub for jsdom environments where it is missing.
if (typeof globalThis.localStorage === 'undefined') {
  const store: Record<string, string> = {};
  globalThis.localStorage = {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => { store[key] = String(value); },
    removeItem: (key: string) => { delete store[key]; },
    clear: () => { Object.keys(store).forEach((k) => { delete store[k]; }); },
    key: (index: number) => Object.keys(store)[index] ?? null,
    length: 0,
  } as Storage;
  Object.defineProperty(globalThis.localStorage, 'length', {
    get: () => Object.keys(store).length,
  });
}

// Initialize i18next (side-effect: configures the global i18n instance) so that
// any component rendered in tests via useTranslation() resolves real translated
// strings instead of falling back to the raw key path. Mirrors main.tsx, which
// imports './i18n' before rendering. Defaults to English (the fallback locale).
import './i18n';

// Provide a minimal ResizeObserver stub for jsdom (not available in JSDOM)
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof globalThis.ResizeObserver;
}

// Provide a minimal CanvasRenderingContext2D stub to silence jsdom warnings
// Install a robust stub unconditionally so gauge components don't trigger 'Not implemented'
// errors in CI/jsdom environments.
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore - test environment stub
HTMLCanvasElement.prototype.getContext = function () {
  return {
    setTransform: () => {},
    scale: () => {},
    clearRect: () => {},
    fillRect: () => {},
    beginPath: () => {},
    arc: () => {},
    stroke: () => {},
    fill: () => {},
    fillText: () => {},
    strokeText: () => {},
    measureText: (text: string) => ({ width: String(text).length * 6 }),
    createLinearGradient: () => ({ addColorStop: () => {} }),
    getImageData: () => ({ data: new Uint8ClampedArray(0) }),
    putImageData: () => {},
    setLineDash: () => {},
    // Path drawing helpers used by gauges
    moveTo: () => {},
    lineTo: () => {},
    quadraticCurveTo: () => {},
    closePath: () => {},
  } as unknown as CanvasRenderingContext2D;
};


// Silence a verbose Three.js warning that appears during tests
const _origConsoleWarn = console.warn.bind(console);
console.warn = (...args: any[]) => {
  try {
    if (typeof args[0] === 'string' && args[0].includes('THREE.WARNING: Multiple instances of Three.js being imported')) {
      return;
    }
  } catch (_) {
    // fallthrough
  }
  return _origConsoleWarn(...args);
};

// Provide a default mock for the Tauri invoke API so tests can override it.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Stub heavy 3D libs to avoid React version mismatches and jsdom issues during tests
vi.mock('@react-three/fiber', () => ({
  Canvas: (props: any) => (props.children ? props.children : null),
  useFrame: () => {},
  useThree: () => ({}),
}));
vi.mock('@react-three/drei', () => ({
  OrbitControls: () => null,
}));

// Provide a default mock for event.listen / event.emit
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (_event: string, _handler: any) => {
    // Return a no-op unlisten function
    return () => {};
  }),
  emit: vi.fn(async (_event: string, _payload?: unknown) => {}),
}));
