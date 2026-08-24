/**
 * Scrolling and zooming the traces.
 *
 * Before this, the only zoom was the Q/A keys and two buttons, and there was no
 * pan at all — the view moved only as a side effect of arrow-keying the data
 * cursor off the edge of the window. Reading a ten-minute log meant holding an
 * arrow key.
 *
 * The canvas draws nothing under jsdom, so these assert the two things the
 * toolbar makes visible: the window-length label (zoom) and the Latest button,
 * which only appears once the view has left the live edge (pan).
 */
import { render, screen, fireEvent } from '@testing-library/react';
import { beforeEach, vi } from 'vitest';
import GraphLog, { type GraphSample } from '../GraphLog';
import { useGraphLogStore } from '../../../stores/graphLogStore';

/** Two minutes at 10 Hz, so there is room to scroll inside a 30 s window. */
const SAMPLES: GraphSample[] = Array.from({ length: 1200 }, (_, i) => ({
  t: i * 100,
  values: { rpm: 1000 + (i % 500), afr: 14 + (i % 10) / 10 },
}));

const CHANNELS = ['rpm', 'afr'];

function panes() {
  return document.querySelector('.graphlog-panes') as HTMLElement;
}

const windowLabel = () =>
  document.querySelector('.graphlog-window-label')?.textContent?.trim() ?? '';

const latestButton = () => screen.queryByRole('button', { name: 'Latest' });

beforeEach(() => {
  // The store persists, so a window length left over from another test would
  // make the zoom assertions depend on execution order.
  useGraphLogStore.setState({ timeWindowSec: 30 });
  vi.restoreAllMocks();
  // jsdom gives every element a zero-size rect; the drag maths divides by the
  // plot width, so give it a real one.
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0, y: 0, top: 0, left: 0, right: 1000, bottom: 400, width: 1000, height: 400,
    toJSON: () => ({}),
  } as DOMRect);
});

test('wheel up zooms in, wheel down zooms out', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  expect(windowLabel()).toBe('30 s');

  fireEvent.wheel(panes(), { deltaY: -120 });
  expect(windowLabel()).toBe('23 s'); // 30 * 0.75

  fireEvent.wheel(panes(), { deltaY: 120 });
  expect(windowLabel()).toBe('30 s');
});

test('zoom stops rather than running away in either direction', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  for (let i = 0; i < 40; i++) fireEvent.wheel(panes(), { deltaY: -120 });
  expect(windowLabel()).toBe('2.0 s');
  for (let i = 0; i < 80; i++) fireEvent.wheel(panes(), { deltaY: 120 });
  expect(windowLabel()).toBe('10 min');
});

test('shift+wheel scrolls back through the log and Latest returns', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  expect(latestButton()).toBeNull(); // following the live edge

  // Shift+wheel up scrolls back in time, the way a browser scrolls left.
  fireEvent.wheel(panes(), { deltaY: -120, shiftKey: true });
  expect(latestButton()).not.toBeNull();

  fireEvent.click(latestButton()!);
  expect(latestButton()).toBeNull();

  // Scrolling forward at the live edge has nowhere to go and must stay there,
  // rather than offering a Latest button that does nothing.
  fireEvent.wheel(panes(), { deltaY: 120, shiftKey: true });
  expect(latestButton()).toBeNull();
});

test('dragging the graphs pans them', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  fireEvent.mouseDown(panes(), { clientX: 600, button: 0 });
  fireEvent.mouseMove(panes(), { clientX: 750 });
  fireEvent.mouseUp(panes());
  expect(latestButton()).not.toBeNull();
});

/**
 * A drag ends in a click, and click places the data cursor. Without
 * suppression, every pan dropped a cursor wherever the drag finished.
 */
test('a drag does not also drop the data cursor, but a click does', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  const cursorTime = () => document.querySelector('.graphlog-cursor-time');

  fireEvent.mouseDown(panes(), { clientX: 600, button: 0 });
  fireEvent.mouseMove(panes(), { clientX: 750 });
  fireEvent.mouseUp(panes());
  fireEvent.click(panes(), { clientX: 750 });
  expect(cursorTime()).toBeNull();

  // A click that never moved is still a click.
  fireEvent.mouseDown(panes(), { clientX: 500, button: 0 });
  fireEvent.mouseUp(panes());
  fireEvent.click(panes(), { clientX: 500 });
  expect(cursorTime()).not.toBeNull();
});

/** A tiny wobble during a click must not be read as a pan. */
test('a two-pixel wobble still counts as a click', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  fireEvent.mouseDown(panes(), { clientX: 500, button: 0 });
  fireEvent.mouseMove(panes(), { clientX: 502 });
  fireEvent.mouseUp(panes());
  expect(latestButton()).toBeNull();
});

/** Nothing to scroll through, so nothing should suggest there is. */
test('an empty log ignores wheel and drag', () => {
  render(<GraphLog samples={[]} availableChannels={CHANNELS} />);
  fireEvent.wheel(panes(), { deltaY: -120 });
  fireEvent.mouseDown(panes(), { clientX: 600, button: 0 });
  fireEvent.mouseMove(panes(), { clientX: 750 });
  fireEvent.mouseUp(panes());
  expect(latestButton()).toBeNull();
  expect(windowLabel()).toBe('30 s');
});

const hscroll = () => document.querySelector('.graphlog-hscroll') as HTMLElement;
const strip = () => hscroll().firstElementChild as HTMLElement;

/**
 * The thumb has to say how much of the log is on screen. A 120 s log in a 30 s
 * window is four windows long, so the strip is 400% and the thumb a quarter of
 * the track.
 */
test('the time scrollbar sizes its thumb to the visible fraction', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  // 1200 samples 100 ms apart span 119.9 s, so just under four 30 s windows.
  expect(parseFloat(strip().style.width)).toBeCloseTo(399.7, 0);

  fireEvent.wheel(panes(), { deltaY: -120 }); // 30 s -> 22.5 s
  expect(parseFloat(strip().style.width)).toBeCloseTo(532.9, 0);
});

/** A log shorter than the window has nowhere to go; the bar must sit inert. */
test('a log that fits leaves nothing to scroll', () => {
  render(<GraphLog samples={SAMPLES.slice(0, 50)} availableChannels={CHANNELS} />);
  expect(strip().style.width).toBe('100%');
});

/** Dragging the scrollbar moves the view, same as dragging the graphs. */
test('scrolling the bar pans the view', () => {
  render(<GraphLog samples={SAMPLES} availableChannels={CHANNELS} />);
  const bar = hscroll();
  // jsdom does no layout, so give the track a scrollable extent by hand.
  Object.defineProperty(bar, 'scrollWidth', { value: 4000, configurable: true });
  Object.defineProperty(bar, 'clientWidth', { value: 1000, configurable: true });
  bar.scrollLeft = 0;
  fireEvent.scroll(bar);
  expect(latestButton()).not.toBeNull();
});
