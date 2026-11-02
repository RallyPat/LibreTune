import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import TableTrailOverlay from '../TableTrailOverlay';
import type { GridGeometry } from '../../../hooks/useTableGridGeometry';

// A 2x2 grid measured inside a scroll host: origin (10, 20), uniform cells.
const GEOMETRY: GridGeometry = {
  left: 10,
  top: 20,
  width: 100,
  height: 60,
  cols: [
    { left: 20, width: 40 },
    { left: 60, width: 40 },
  ],
  rows: [
    { top: 20, height: 20 },
    { top: 40, height: 20 },
  ],
};

describe('TableTrailOverlay', () => {
  it('renders nothing without geometry or points', () => {
    const { container } = render(
      <TableTrailOverlay
        geometry={null}
        trail={[]}
        rowCount={2}
        yAxisBottom={false}
        fadeMs={8000}
      />,
    );
    expect(container.querySelector('svg')).toBeNull();
  });

  it('draws one segment and dots per trail point plus the current-cell halo', () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);
    const trail = [
      { row: 0, col: 0, time: 0 },
      { row: 1, col: 1, time: 500 },
    ];
    const { container } = render(
      <TableTrailOverlay
        geometry={GEOMETRY}
        trail={trail}
        rowCount={2}
        yAxisBottom={false}
        fadeMs={8000}
        currentCell={{ row: 1, col: 1 }}
      />,
    );

    const svg = container.querySelector('svg.table-trail-overlay');
    expect(svg).not.toBeNull();
    // 2 points → 1 segment, 2 dots, halo + head dot (4 circles).
    expect(svg!.querySelectorAll('line')).toHaveLength(1);
    expect(svg!.querySelectorAll('circle')).toHaveLength(4);

    // The head sits on the current cell's centre — col 1 (left 60, width 40),
    // row 1 (top 40, height 20). Geometry rects are table-relative.
    const circles = svg!.querySelectorAll('circle');
    const head = circles[circles.length - 1];
    expect(head.getAttribute('cx')).toBe(String(60 + 40 / 2));
    expect(head.getAttribute('cy')).toBe(String(40 + 20 / 2));
    vi.useRealTimers();
  });

  it('maps data rows through the yAxisBottom flip', () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const { container } = render(
      <TableTrailOverlay
        geometry={GEOMETRY}
        trail={[{ row: 0, col: 0, time: 0 }]}
        rowCount={2}
        yAxisBottom={true}
        fadeMs={Infinity}
      />,
    );

    // Data row 0 with the flip renders at the DOM's last row (top 40).
    const dot = container.querySelector('circle');
    expect(dot).not.toBeNull();
    expect(dot!.getAttribute('cy')).toBe(String(40 + 20 / 2));
    vi.useRealTimers();
  });
});
