/**
 * Heatmap Color System
 *
 * Centralized, theme-aware heatmap coloring with multiple presets
 * and accessibility options.
 */

// ============================================================================
// Types
// ============================================================================

/** Available heatmap color scheme presets */
export type HeatmapScheme =
  | 'tunerstudio'  // Classic: Blue → Cyan → Green → Yellow → Orange → Red
  | 'thermal'      // Black → Purple → Red → Orange → Yellow → White
  | 'viridis'      // Colorblind-safe: Purple → Blue → Teal → Green → Yellow
  | 'plasma'       // Colorblind-safe: Purple → Pink → Orange → Yellow
  | 'grayscale'    // Universal: Black → White
  | 'custom';      // User-defined colors

/** Context for heatmap coloring - different contexts can use different schemes */
export type HeatmapContext =
  | 'value'     // VE tables, timing tables, general value display
  | 'change'    // AFR correction magnitude, value deltas
  | 'coverage'; // Hit weighting, data coverage visualization

/** RGB color representation */
export interface RGBColor {
  r: number;
  g: number;
  b: number;
}

/** Heatmap scheme definition with color stops */
export interface HeatmapSchemeDefinition {
  name: string;
  description: string;
  colorblindSafe: boolean;
  stops: string[]; // Hex colors from low to high
}

// ============================================================================
// Preset Schemes
// ============================================================================

export const HEATMAP_SCHEMES: Record<Exclude<HeatmapScheme, 'custom'>, HeatmapSchemeDefinition> = {
  tunerstudio: {
    name: 'TunerStudio Classic',
    description: 'Traditional ECU tuning gradient',
    colorblindSafe: false,
    stops: [
      '#0000FF', // Blue (cold/low)
      '#00FFFF', // Cyan
      '#00FF00', // Green
      '#FFFF00', // Yellow
      '#FF8000', // Orange
      '#FF0000', // Red (hot/high)
    ],
  },
  thermal: {
    name: 'Thermal',
    description: 'Infrared camera style gradient',
    colorblindSafe: false,
    stops: [
      '#000000', // Black (cold)
      '#4B0082', // Indigo
      '#FF0000', // Red
      '#FF8000', // Orange
      '#FFFF00', // Yellow
      '#FFFFFF', // White (hot)
    ],
  },
  viridis: {
    name: 'Viridis',
    description: 'Perceptually uniform, colorblind-safe',
    colorblindSafe: true,
    stops: [
      '#440154', // Dark purple
      '#414487', // Purple-blue
      '#2A788E', // Teal
      '#22A884', // Green
      '#7AD151', // Light green
      '#FDE725', // Yellow
    ],
  },
  plasma: {
    name: 'Plasma',
    description: 'High contrast, colorblind-safe',
    colorblindSafe: true,
    stops: [
      '#0D0887', // Deep blue-purple
      '#7E03A8', // Purple
      '#CC4778', // Pink
      '#F89540', // Orange
      '#F0F921', // Bright yellow
      '#F0F921', // (repeat for 6 stops)
    ],
  },
  grayscale: {
    name: 'Grayscale',
    description: 'Universal accessibility, print-friendly',
    colorblindSafe: true,
    stops: [
      '#000000', // Black
      '#333333', // Dark gray
      '#666666', // Gray
      '#999999', // Light gray
      '#CCCCCC', // Lighter gray
      '#FFFFFF', // White
    ],
  },
};

// ============================================================================
// Color Utilities
// ============================================================================

/**
 * Parse a hex color string to RGB components
 */
export function hexToRgb(hex: string): RGBColor {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  if (!result) {
    return { r: 128, g: 128, b: 128 }; // Default gray
  }
  return {
    r: parseInt(result[1], 16),
    g: parseInt(result[2], 16),
    b: parseInt(result[3], 16),
  };
}

/**
 * Convert RGB to hex string
 */
export function rgbToHex(rgb: RGBColor): string {
  const toHex = (n: number) => Math.round(Math.max(0, Math.min(255, n))).toString(16).padStart(2, '0');
  return `#${toHex(rgb.r)}${toHex(rgb.g)}${toHex(rgb.b)}`;
}

/**
 * Convert RGB to CSS rgb() string
 */
export function rgbToCss(rgb: RGBColor): string {
  return `rgb(${Math.round(rgb.r)}, ${Math.round(rgb.g)}, ${Math.round(rgb.b)})`;
}

/**
 * Interpolate between two colors
 * @param color1 - Start color (hex)
 * @param color2 - End color (hex)
 * @param ratio - Interpolation ratio (0 = color1, 1 = color2)
 */
export function interpolateColor(color1: string, color2: string, ratio: number): RGBColor {
  const rgb1 = hexToRgb(color1);
  const rgb2 = hexToRgb(color2);
  const clampedRatio = Math.max(0, Math.min(1, ratio));

  return {
    r: rgb1.r + (rgb2.r - rgb1.r) * clampedRatio,
    g: rgb1.g + (rgb2.g - rgb1.g) * clampedRatio,
    b: rgb1.b + (rgb2.b - rgb1.b) * clampedRatio,
  };
}

/**
 * Get color from a multi-stop gradient at a given position
 * @param stops - Array of hex color stops
 * @param position - Position in gradient (0-1)
 */
export function getGradientColor(stops: string[], position: number): RGBColor {
  const clampedPos = Math.max(0, Math.min(1, position));

  if (stops.length === 0) {
    return { r: 128, g: 128, b: 128 };
  }
  if (stops.length === 1) {
    return hexToRgb(stops[0]);
  }

  // Map position to segment
  const segmentCount = stops.length - 1;
  const scaledPos = clampedPos * segmentCount;
  const segmentIndex = Math.min(Math.floor(scaledPos), segmentCount - 1);
  const segmentRatio = scaledPos - segmentIndex;

  return interpolateColor(stops[segmentIndex], stops[segmentIndex + 1], segmentRatio);
}

// ============================================================================
// Main Heatmap Functions
// ============================================================================

/**
 * Convert a value to a heatmap color
 *
 * @param value - The value to colorize
 * @param min - Minimum value in range
 * @param max - Maximum value in range
 * @param scheme - Color scheme to use (or custom stops array)
 * @returns CSS color string
 */
export function valueToHeatmapColor(
  value: number,
  min: number,
  max: number,
  scheme: HeatmapScheme | string[] = 'tunerstudio'
): string {
  // Handle edge cases
  const range = max - min;
  if (range === 0) {
    // All values are the same - return middle color
    const stops = getSchemeStops(scheme);
    return rgbToCss(hexToRgb(stops[Math.floor(stops.length / 2)]));
  }

  // Normalize value to 0-1 range
  const normalized = Math.max(0, Math.min(1, (value - min) / range));

  // Get color from gradient
  const stops = getSchemeStops(scheme);
  const rgb = getGradientColor(stops, normalized);

  return rgbToCss(rgb);
}

/**
 * Get CSS gradient string for a scheme (for legends, previews)
 */
export function getHeatmapGradientCSS(
  scheme: HeatmapScheme | string[] = 'tunerstudio',
  direction: 'to right' | 'to left' | 'to top' | 'to bottom' = 'to right'
): string {
  const stops = getSchemeStops(scheme);
  return `linear-gradient(${direction}, ${stops.join(', ')})`;
}

/**
 * Get the color stops for a scheme
 */
export function getSchemeStops(scheme: HeatmapScheme | string[]): string[] {
  if (Array.isArray(scheme)) {
    return scheme;
  }
  if (scheme === 'custom') {
    // Custom scheme - return tunerstudio as fallback
    return HEATMAP_SCHEMES.tunerstudio.stops;
  }
  return HEATMAP_SCHEMES[scheme]?.stops ?? HEATMAP_SCHEMES.tunerstudio.stops;
}

/**
 * Get scheme definition by name
 */
export function getSchemeDefinition(scheme: HeatmapScheme): HeatmapSchemeDefinition | null {
  if (scheme === 'custom') return null;
  return HEATMAP_SCHEMES[scheme] ?? null;
}

/**
 * Get all available scheme names
 */
export function getAvailableSchemes(): Array<{ id: HeatmapScheme; name: string; colorblindSafe: boolean }> {
  return [
    { id: 'tunerstudio', name: 'TunerStudio Classic', colorblindSafe: false },
    { id: 'thermal', name: 'Thermal', colorblindSafe: false },
    { id: 'viridis', name: 'Viridis', colorblindSafe: true },
    { id: 'plasma', name: 'Plasma', colorblindSafe: true },
    { id: 'grayscale', name: 'Grayscale', colorblindSafe: true },
    { id: 'custom', name: 'Custom', colorblindSafe: false },
  ];
}

// ============================================================================
// Context-Aware Heatmap Hook Support
// ============================================================================

/** Default schemes for each context */
export const DEFAULT_CONTEXT_SCHEMES: Record<HeatmapContext, HeatmapScheme> = {
  value: 'tunerstudio',
  change: 'tunerstudio',
  coverage: 'tunerstudio',
};

/** Settings structure for heatmap configuration */
export interface HeatmapSettings {
  valueScheme: HeatmapScheme;
  changeScheme: HeatmapScheme;
  coverageScheme: HeatmapScheme;
  customValueStops?: string[];
  customChangeStops?: string[];
  customCoverageStops?: string[];
}

/**
 * Create a color getter function for a specific context
 */
export function createContextColorGetter(
  settings: HeatmapSettings,
  context: HeatmapContext
): (value: number, min: number, max: number) => string {
  let scheme: HeatmapScheme | string[];

  switch (context) {
    case 'value':
      scheme = settings.valueScheme === 'custom' && settings.customValueStops
        ? settings.customValueStops
        : settings.valueScheme;
      break;
    case 'change':
      scheme = settings.changeScheme === 'custom' && settings.customChangeStops
        ? settings.customChangeStops
        : settings.changeScheme;
      break;
    case 'coverage':
      scheme = settings.coverageScheme === 'custom' && settings.customCoverageStops
        ? settings.customCoverageStops
        : settings.coverageScheme;
      break;
  }

  return (value: number, min: number, max: number) => valueToHeatmapColor(value, min, max, scheme);
}
