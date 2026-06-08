/**
 * TS Dashboard Types
 * 
 * These types mirror the Rust dash::types structures exactly for full compatibility
 * with TS .dash and .gauge file formats.
 */

/** TS color format (ARGB) */
export interface TsColor {
  alpha: number;
  red: number;
  green: number;
  blue: number;
}

/** Convert TsColor to CSS rgba string */
export function tsColorToRgba(color: TsColor | null | undefined): string {
  if (!color) {
    return 'rgba(128, 128, 128, 1)'; // Default gray if color is null/undefined
  }
  // TunerStudio treats alpha=0 as fully opaque (their default), not transparent
  // So we map 0 -> 1.0 (fully opaque) and 1-255 -> normal alpha range
  const alpha = (color.alpha ?? 255) === 0 ? 1 : (color.alpha ?? 255) / 255;
  return `rgba(${color.red ?? 128}, ${color.green ?? 128}, ${color.blue ?? 128}, ${alpha})`;
}

/** Convert TsColor to CSS hex string */
export function tsColorToHex(color: TsColor | null | undefined): string {
  if (!color) {
    return '#808080'; // Default gray if color is null/undefined
  }
  const r = (color.red ?? 128).toString(16).padStart(2, '0');
  const g = (color.green ?? 128).toString(16).padStart(2, '0');
  const b = (color.blue ?? 128).toString(16).padStart(2, '0');
  return `#${r}${g}${b}`;
}

/** Bibliography metadata */
export interface Bibliography {
  author: string;
  company: string;
  write_date: string;
}

/** Version information */
export interface VersionInfo {
  file_format: string;
  firmware_signature: string | null;
}

/** Resource type for embedded images/fonts */
export type ResourceType = 'Png' | 'Gif' | 'Ttf';

/** Embedded image/font resource */
export interface EmbeddedImage {
  file_name: string;
  image_id: string;
  resource_type: ResourceType;
  data: string; // Base64 encoded
}

/** Background image display style */
export type BackgroundStyle = 'Tile' | 'Stretch' | 'Center' | 'Fit';

/** Gauge painter type - determines rendering style */
export type GaugePainter =
  | 'AnalogGauge'
  | 'BasicAnalogGauge'
  | 'CircleAnalogGauge'
  | 'AsymmetricSweepGauge'
  | 'BasicReadout'
  | 'HorizontalBarGauge'
  | 'VerticalBarGauge'
  | 'HorizontalLineGauge'
  | 'VerticalDashedBar'
  | 'AnalogBarGauge'
  | 'AnalogMovingBarGauge'
  | 'Histogram'
  | 'LineGraph'
  | 'RoundGauge'
  | 'RoundDashedGauge'
  | 'FuelMeter'
  | 'Tachometer';

/** Indicator painter type */
export type IndicatorPainter = 'BasicRectangleIndicator';

/** Gauge configuration from TS .dash file */
export interface TsGaugeConfig {
  // Identification
  id: string;
  gauge_painter: GaugePainter;
  gauge_style: string;

  // Data binding
  output_channel: string;
  title: string;
  units: string;

  // Value
  value: number;

  // Range
  min: number;
  max: number;
  min_vp: string | null;
  max_vp: string | null;
  default_min: number | null;
  default_max: number | null;
  peg_limits: boolean;

  // Thresholds
  low_warning: number | null;
  high_warning: number | null;
  low_critical: number | null;
  high_critical: number | null;
  low_warning_vp: string | null;
  high_warning_vp: string | null;
  low_critical_vp: string | null;
  high_critical_vp: string | null;

  // Colors
  back_color: TsColor;
  font_color: TsColor;
  trim_color: TsColor;
  warn_color: TsColor;
  critical_color: TsColor;
  needle_color: TsColor;

  // Display
  value_digits: number;
  label_digits: number;
  font_family: string;
  font_size_adjustment: number;
  italic_font: boolean;

  // Geometry (analog/sweep gauges)
  sweep_angle: number;
  start_angle: number;
  face_angle: number;
  sweep_begin_degree: number;
  counter_clockwise: boolean;

  // Tick marks
  major_ticks: number;
  minor_ticks: number;

  // Layout (relative 0.0-1.0)
  relative_x: number;
  relative_y: number;
  relative_width: number;
  relative_height: number;

  // Appearance
  border_width: number;
  shortest_size: number;
  shape_locked_to_aspect: boolean;
  antialiasing_on: boolean;

  // Custom images
  background_image_file_name: string | null;
  needle_image_file_name: string | null;

  // History/tracking
  show_history: boolean;
  history_value: number;
  history_delay: number;
  needle_smoothing: number;

  // Interaction
  short_click_action: string | null;
  long_click_action: string | null;

  // Display options
  display_value_at_180: boolean;
}

/** Indicator configuration */
export interface TsIndicatorConfig {
  id: string;
  indicator_painter: IndicatorPainter;
  output_channel: string;
  value: number;

  on_text: string;
  off_text: string;
  on_text_color: TsColor;
  off_text_color: TsColor;
  on_background_color: TsColor;
  off_background_color: TsColor;
  on_image_file_name: string | null;
  off_image_file_name: string | null;

  relative_x: number;
  relative_y: number;
  relative_width: number;
  relative_height: number;

  font_family: string;
  italic_font: boolean;
  antialiasing_on: boolean;

  short_click_action: string | null;
  long_click_action: string | null;
  ecu_configuration_name: string | null;
}

/** Dashboard component - gauge or indicator */
export type DashComponent =
  | { Gauge: TsGaugeConfig }
  | { Indicator: TsIndicatorConfig };

/** Helper to check if component is a gauge */
export function isGauge(comp: DashComponent): comp is { Gauge: TsGaugeConfig } {
  return 'Gauge' in comp;
}

/** Helper to check if component is an indicator */
export function isIndicator(comp: DashComponent): comp is { Indicator: TsIndicatorConfig } {
  return 'Indicator' in comp;
}

/** Gauge cluster - container for dashboard components */
export interface GaugeCluster {
  anti_aliasing: boolean;
  background_dither_color: TsColor | null;
  cluster_background_color: TsColor;
  cluster_background_image_file_name: string | null;
  cluster_background_image_style: BackgroundStyle;
  embedded_images: EmbeddedImage[];
  components: DashComponent[];
}

/** Top-level dashboard file structure */
export interface DashFile {
  bibliography: Bibliography;
  version_info: VersionInfo;
  gauge_cluster: GaugeCluster;
}

/** Dashboard file info for listing */
export interface DashFileInfo {
  name: string;
  path: string;
  category: string; // "User", "Reference", "Bundled", etc.
}

/** Build embedded image map for quick lookup */
export function buildEmbeddedImageMap(images: EmbeddedImage[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const img of images) {
    const mimeType = img.resource_type === 'Png' ? 'image/png' 
                   : img.resource_type === 'Gif' ? 'image/gif'
                   : 'font/ttf';
    const dataUrl = `data:${mimeType};base64,${img.data}`;
    map.set(img.image_id, dataUrl);
    // Also map by filename for convenience
    if (img.file_name) {
      map.set(img.file_name, dataUrl);
    }
  }
  return map;
}
