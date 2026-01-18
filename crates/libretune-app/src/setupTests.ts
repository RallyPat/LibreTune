import '@testing-library/jest-dom';

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

// Provide a default mock for event.listen
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (_event: string, handler: any) => {
    // Return a no-op unlisten function
    return () => {};
  }),
}));
