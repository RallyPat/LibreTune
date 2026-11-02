/**
 * Shared primitives for the table trail line (issue #132).
 *
 * The trail follows the live operating point across a table grid: points are
 * recorded in DATA space (row indexes the table's Y bins as stored, col
 * indexes the X bins), independent of how any particular view displays the
 * grid. Views that flip the Y axis (`yAxisBottom`) map through
 * {@link trailDisplayRow} at draw time.
 */

export interface TrailPoint {
  /** Data-space row (index into the table's Y bins as stored). */
  row: number;
  /** Data-space column (index into the table's X bins). */
  col: number;
  /** Recording time (Date.now(), ms). */
  time: number;
}

/**
 * Display row for a data-space row. `yAxisBottom` renders rows reversed;
 * geometry arrays follow the DOM, so an overlay must flip back.
 */
export function trailDisplayRow(row: number, rowCount: number, yAxisBottom: boolean): number {
  return yAxisBottom ? rowCount - 1 - row : row;
}
