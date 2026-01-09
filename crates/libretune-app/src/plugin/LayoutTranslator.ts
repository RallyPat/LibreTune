/**
 * Translates Swing layout managers to CSS styles.
 */

import {
  LayoutInfo,
  GridBagConstraint,
  BorderConstraint,
  GRIDBAG_ANCHOR,
  GRIDBAG_FILL,
  FLOW_ALIGN,
  BOX_AXIS,
} from './types';
import React from 'react';

/**
 * Get CSS styles for a container based on its Swing layout manager.
 */
export function getLayoutContainerStyle(layout: LayoutInfo | undefined): React.CSSProperties {
  if (!layout) {
    return { position: 'relative' };
  }

  switch (layout.type) {
    case 'BorderLayout':
      return {
        display: 'grid',
        gridTemplateAreas: `
          "north north north"
          "west center east"
          "south south south"
        `,
        gridTemplateRows: 'auto 1fr auto',
        gridTemplateColumns: 'auto 1fr auto',
        gap: `${layout.vgap}px ${layout.hgap}px`,
        width: '100%',
        height: '100%',
      };

    case 'GridBagLayout':
      return {
        display: 'grid',
        width: '100%',
        height: '100%',
      };

    case 'FlowLayout':
      return {
        display: 'flex',
        flexWrap: 'wrap',
        gap: `${layout.vgap}px ${layout.hgap}px`,
        justifyContent: flowAlignToJustify(layout.alignment),
        alignItems: 'flex-start',
        width: '100%',
      };

    case 'BoxLayout':
      return {
        display: 'flex',
        flexDirection: layout.axis === BOX_AXIS.X_AXIS || layout.axis === BOX_AXIS.LINE_AXIS 
          ? 'row' 
          : 'column',
        width: '100%',
        height: '100%',
      };

    case 'GridLayout':
      return {
        display: 'grid',
        gridTemplateColumns: layout.cols > 0 
          ? `repeat(${layout.cols}, 1fr)` 
          : `repeat(auto-fill, minmax(100px, 1fr))`,
        gridTemplateRows: layout.rows > 0 
          ? `repeat(${layout.rows}, 1fr)` 
          : 'auto',
        gap: `${layout.vgap}px ${layout.hgap}px`,
        width: '100%',
        height: '100%',
      };

    case 'CardLayout':
      return {
        position: 'relative',
        width: '100%',
        height: '100%',
        padding: `${layout.vgap}px ${layout.hgap}px`,
      };

    default:
      return { position: 'relative' };
  }
}

/**
 * Get CSS styles for a child component based on layout constraints.
 */
export function getLayoutChildStyle(
  layout: LayoutInfo | undefined,
  constraint: unknown
): React.CSSProperties {
  if (!layout) {
    return {};
  }

  switch (layout.type) {
    case 'BorderLayout':
      return getBorderLayoutChildStyle(constraint as BorderConstraint | undefined);

    case 'GridBagLayout':
      return getGridBagChildStyle(constraint as GridBagConstraint | undefined);

    case 'FlowLayout':
      return { flexShrink: 0 };

    case 'BoxLayout':
      return { flexShrink: 0 };

    case 'GridLayout':
      return {};

    case 'CardLayout':
      return {
        position: 'absolute',
        inset: 0,
      };

    default:
      return {};
  }
}

function getBorderLayoutChildStyle(constraint: BorderConstraint | undefined): React.CSSProperties {
  const area = constraint?.toUpperCase() || 'CENTER';
  
  switch (area) {
    case 'NORTH':
      return { gridArea: 'north' };
    case 'SOUTH':
      return { gridArea: 'south' };
    case 'EAST':
      return { gridArea: 'east' };
    case 'WEST':
      return { gridArea: 'west' };
    case 'CENTER':
    default:
      return { gridArea: 'center' };
  }
}

function getGridBagChildStyle(constraint: GridBagConstraint | undefined): React.CSSProperties {
  if (!constraint) {
    return {};
  }

  const style: React.CSSProperties = {};

  // Grid position (convert from 0-based to 1-based)
  const gridx = constraint.gridx >= 0 ? constraint.gridx + 1 : 1;
  const gridy = constraint.gridy >= 0 ? constraint.gridy + 1 : 1;
  
  style.gridColumn = `${gridx} / span ${Math.max(1, constraint.gridwidth)}`;
  style.gridRow = `${gridy} / span ${Math.max(1, constraint.gridheight)}`;

  // Alignment
  style.justifySelf = anchorToJustifySelf(constraint.anchor);
  style.alignSelf = anchorToAlignSelf(constraint.anchor);

  // Fill
  if (constraint.fill === GRIDBAG_FILL.HORIZONTAL || constraint.fill === GRIDBAG_FILL.BOTH) {
    style.width = '100%';
    style.justifySelf = 'stretch';
  }
  if (constraint.fill === GRIDBAG_FILL.VERTICAL || constraint.fill === GRIDBAG_FILL.BOTH) {
    style.height = '100%';
    style.alignSelf = 'stretch';
  }

  // Insets (padding)
  if (constraint.insetsTop || constraint.insetsRight || constraint.insetsBottom || constraint.insetsLeft) {
    style.margin = `${constraint.insetsTop}px ${constraint.insetsRight}px ${constraint.insetsBottom}px ${constraint.insetsLeft}px`;
  }

  return style;
}

function flowAlignToJustify(alignment: number): string {
  switch (alignment) {
    case FLOW_ALIGN.LEFT:
    case FLOW_ALIGN.LEADING:
      return 'flex-start';
    case FLOW_ALIGN.RIGHT:
    case FLOW_ALIGN.TRAILING:
      return 'flex-end';
    case FLOW_ALIGN.CENTER:
    default:
      return 'center';
  }
}

function anchorToJustifySelf(anchor: number): string {
  switch (anchor) {
    case GRIDBAG_ANCHOR.WEST:
    case GRIDBAG_ANCHOR.NORTHWEST:
    case GRIDBAG_ANCHOR.SOUTHWEST:
    case GRIDBAG_ANCHOR.LINE_START:
    case GRIDBAG_ANCHOR.FIRST_LINE_START:
    case GRIDBAG_ANCHOR.LAST_LINE_START:
      return 'start';
    case GRIDBAG_ANCHOR.EAST:
    case GRIDBAG_ANCHOR.NORTHEAST:
    case GRIDBAG_ANCHOR.SOUTHEAST:
    case GRIDBAG_ANCHOR.LINE_END:
    case GRIDBAG_ANCHOR.FIRST_LINE_END:
    case GRIDBAG_ANCHOR.LAST_LINE_END:
      return 'end';
    default:
      return 'center';
  }
}

function anchorToAlignSelf(anchor: number): string {
  switch (anchor) {
    case GRIDBAG_ANCHOR.NORTH:
    case GRIDBAG_ANCHOR.NORTHEAST:
    case GRIDBAG_ANCHOR.NORTHWEST:
    case GRIDBAG_ANCHOR.PAGE_START:
    case GRIDBAG_ANCHOR.FIRST_LINE_START:
    case GRIDBAG_ANCHOR.FIRST_LINE_END:
      return 'start';
    case GRIDBAG_ANCHOR.SOUTH:
    case GRIDBAG_ANCHOR.SOUTHEAST:
    case GRIDBAG_ANCHOR.SOUTHWEST:
    case GRIDBAG_ANCHOR.PAGE_END:
    case GRIDBAG_ANCHOR.LAST_LINE_START:
    case GRIDBAG_ANCHOR.LAST_LINE_END:
      return 'end';
    default:
      return 'center';
  }
}

/**
 * Calculate grid template from GridBag constraints.
 * Analyzes all children to determine optimal column/row templates.
 */
export function calculateGridBagTemplate(
  children: Array<{ constraint?: GridBagConstraint; weight?: { x: number; y: number } }>
): { columns: string; rows: string } {
  // Find max grid dimensions
  let maxCol = 0;
  let maxRow = 0;
  const colWeights: number[] = [];
  const rowWeights: number[] = [];

  for (const child of children) {
    if (child.constraint) {
      const endCol = child.constraint.gridx + child.constraint.gridwidth;
      const endRow = child.constraint.gridy + child.constraint.gridheight;
      maxCol = Math.max(maxCol, endCol);
      maxRow = Math.max(maxRow, endRow);

      // Track weights
      if (child.constraint.weightx > 0) {
        colWeights[child.constraint.gridx] = Math.max(
          colWeights[child.constraint.gridx] || 0,
          child.constraint.weightx
        );
      }
      if (child.constraint.weighty > 0) {
        rowWeights[child.constraint.gridy] = Math.max(
          rowWeights[child.constraint.gridy] || 0,
          child.constraint.weighty
        );
      }
    }
  }

  // Generate templates
  const columns = Array.from({ length: maxCol || 1 }, (_, i) => 
    colWeights[i] ? `${colWeights[i]}fr` : 'auto'
  ).join(' ');

  const rows = Array.from({ length: maxRow || 1 }, (_, i) => 
    rowWeights[i] ? `${rowWeights[i]}fr` : 'auto'
  ).join(' ');

  return { columns: columns || 'auto', rows: rows || 'auto' };
}
