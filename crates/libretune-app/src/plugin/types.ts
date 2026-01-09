/**
 * Swing component tree types for React rendering.
 * These match the Rust protocol.rs and Java SwingIntrospector types.
 */

export interface SwingComponent {
  id: string;
  componentType: string;
  bounds: ComponentBounds;
  layout?: LayoutInfo;
  layoutConstraint?: unknown;
  properties: Record<string, unknown>;
  children?: SwingComponent[];
}

export interface ComponentBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type LayoutInfo =
  | { type: 'BorderLayout'; hgap: number; vgap: number }
  | { type: 'GridBagLayout' }
  | { type: 'FlowLayout'; alignment: number; hgap: number; vgap: number }
  | { type: 'BoxLayout'; axis: number }
  | { type: 'GridLayout'; rows: number; cols: number; hgap: number; vgap: number }
  | { type: 'CardLayout'; hgap: number; vgap: number }
  | { type: 'null' };

export interface GridBagConstraint {
  gridx: number;
  gridy: number;
  gridwidth: number;
  gridheight: number;
  weightx: number;
  weighty: number;
  anchor: number;
  fill: number;
  insetsTop: number;
  insetsLeft: number;
  insetsBottom: number;
  insetsRight: number;
  ipadx?: number;
  ipady?: number;
}

export type BorderConstraint = 'NORTH' | 'SOUTH' | 'EAST' | 'WEST' | 'CENTER' | 'North' | 'South' | 'East' | 'West' | 'Center';

export interface PluginEvent {
  type: string;
  componentId: string;
  [key: string]: unknown;
}

export interface PluginInfo {
  id: string;
  displayName: string;
  description: string;
  version: string;
  pluginType: string;
  jarPath: string;
  helpUrl?: string;
}

// GridBag anchor constants
export const GRIDBAG_ANCHOR = {
  CENTER: 10,
  NORTH: 11,
  NORTHEAST: 12,
  EAST: 13,
  SOUTHEAST: 14,
  SOUTH: 15,
  SOUTHWEST: 16,
  WEST: 17,
  NORTHWEST: 18,
  PAGE_START: 19,
  PAGE_END: 20,
  LINE_START: 21,
  LINE_END: 22,
  FIRST_LINE_START: 23,
  FIRST_LINE_END: 24,
  LAST_LINE_START: 25,
  LAST_LINE_END: 26,
};

// GridBag fill constants
export const GRIDBAG_FILL = {
  NONE: 0,
  HORIZONTAL: 2,
  VERTICAL: 3,
  BOTH: 1,
};

// FlowLayout alignment constants
export const FLOW_ALIGN = {
  LEFT: 0,
  CENTER: 1,
  RIGHT: 2,
  LEADING: 3,
  TRAILING: 4,
};

// BoxLayout axis constants
export const BOX_AXIS = {
  X_AXIS: 0,
  Y_AXIS: 1,
  LINE_AXIS: 2,
  PAGE_AXIS: 3,
};
