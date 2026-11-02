/**
 * Measure the on-screen geometry of a plain HTML grid table for the trail
 * overlay (issue #132).
 *
 * Extracted from the tab-view TableEditor, which has always measured inline.
 * Expects the markup the editors and the AutoTune grid produce: column
 * headers in `thead th` / `tfoot th` (the corner cell first) and one `th` at
 * the start of each `tbody` row. Results are in the scroll host's content
 * space, so they stay correct while the user scrolls.
 */
import { useLayoutEffect, useState, type RefObject } from 'react';

/** Measured on-screen geometry of the grid, for the trail overlay. */
export interface GridGeometry {
  /** Table origin within the scroll container's content space. */
  left: number;
  top: number;
  width: number;
  height: number;
  /** Column header rects, table-relative. */
  cols: Array<{ left: number; width: number }>;
  /** Row header rects, table-relative, in DOM (display) order. */
  rows: Array<{ top: number; height: number }>;
}

export interface UseTableGridGeometryDeps {
  /** Number of columns — a bin-count change needs a fresh measurement. */
  colCount: number;
  /** Number of rows. */
  rowCount: number;
  /** The orientation flip re-orders the DOM rows. */
  yAxisBottom: boolean;
  /** Anything that remounts or resizes the table (2D/3D switch, table switch). */
  viewKey?: string | number | boolean;
}

export function useTableGridGeometry(
  tableRef: RefObject<HTMLTableElement | null>,
  scrollHostRef: RefObject<HTMLDivElement | null>,
  deps: UseTableGridGeometryDeps,
): GridGeometry | null {
  const { colCount, rowCount, yAxisBottom, viewKey } = deps;
  const [geometry, setGeometry] = useState<GridGeometry | null>(null);

  useLayoutEffect(() => {
    const table = tableRef.current;
    const host = scrollHostRef.current;
    if (!table || !host) return;

    const measure = () => {
      const tRect = table.getBoundingClientRect();
      const hRect = host.getBoundingClientRect();
      // Content-space origin: rects are viewport-based, so undo the current
      // scroll to get coordinates that stay correct as the user scrolls.
      const left = tRect.left - hRect.left + host.scrollLeft;
      const top = tRect.top - hRect.top + host.scrollTop;
      const cols = Array.from(
        table.querySelectorAll<HTMLTableCellElement>('thead th, tfoot th'),
      )
        .slice(1) // the corner cell is not a column
        .map((th) => {
          const r = th.getBoundingClientRect();
          return { left: r.left - tRect.left, width: r.width };
        });
      const rows = Array.from(
        table.querySelectorAll<HTMLTableCellElement>('tbody tr > th:first-child'),
      ).map((th) => {
        const r = th.getBoundingClientRect();
        return { top: r.top - tRect.top, height: r.height };
      });
      setGeometry((prev) => {
        const close = (a: number, b: number) => Math.abs(a - b) < 0.5;
        const same =
          prev !== null &&
          close(prev.left, left) &&
          close(prev.top, top) &&
          close(prev.width, tRect.width) &&
          close(prev.height, tRect.height) &&
          prev.cols.length === cols.length &&
          prev.rows.length === rows.length &&
          prev.cols.every(
            (c, i) => close(c.left, cols[i].left) && close(c.width, cols[i].width),
          ) &&
          prev.rows.every(
            (r, i) => close(r.top, rows[i].top) && close(r.height, rows[i].height),
          );
        return same
          ? prev
          : { left, top, width: tRect.width, height: tRect.height, cols, rows };
      });
    };

    measure();
    if (typeof ResizeObserver === 'undefined') return; // jsdom
    const ro = new ResizeObserver(() => measure());
    ro.observe(table);
    ro.observe(host);
    return () => ro.disconnect();
    // Bin-count changes, the orientation flip, and remounts all require a
    // fresh measurement.
  }, [tableRef, scrollHostRef, colCount, rowCount, yAxisBottom, viewKey]);

  return geometry;
}
