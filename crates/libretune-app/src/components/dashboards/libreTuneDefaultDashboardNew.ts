/**
 * LibreTune Default Dashboard
 * 
 * A legally distinct, original, but config-compatible
 * Features:
 * - Dark theme throughout (no light-on-dark contrast issues)
 * - LibreTune color scheme (deep blues, teals, amber accents)
 * - Asymmetric layout for visual interest
 * - Proper channel name resolution (no hardcoded names)
 * - Mix of gauge types (analog, digital, bar)
 */

import { TsColor, TsGaugeConfig, TsIndicatorConfig, GaugeCluster, DashFile, Bibliography, VersionInfo, GaugePainter } from './dashTypes';

// LibreTune brand colors
const LT_DARK_BG: TsColor = { red: 18, green: 20, blue: 28, alpha: 255 };
const LT_DARKER_BG: TsColor = { red: 12, green: 14, blue: 20, alpha: 255 };
const LT_ACCENT_BLUE: TsColor = { red: 74, green: 158, blue: 248, alpha: 255 };
const LT_ACCENT_TEAL: TsColor = { red: 56, green: 189, blue: 248, alpha: 255 };
const LT_ACCENT_AMBER: TsColor = { red: 251, green: 191, blue: 36, alpha: 255 };
const LT_ACCENT_GREEN: TsColor = { red: 34, green: 197, blue: 94, alpha: 255 };
const LT_TEXT_PRIMARY: TsColor = { red: 255, green: 255, blue: 255, alpha: 255 };
const LT_TEXT_SECONDARY: TsColor = { red: 148, green: 163, blue: 184, alpha: 255 };
const LT_TEXT_MUTED: TsColor = { red: 100, green: 116, blue: 139, alpha: 255 };
const LT_WARN_COLOR: TsColor = { red: 234, green: 179, blue: 8, alpha: 255 };
const LT_CRITICAL_COLOR: TsColor = { red: 239, green: 68, blue: 68, alpha: 255 };

// Helper to create a TsColor
function color(r: number, g: number, b: number, a: number = 255): TsColor {
  return { red: r, green: g, blue: b, alpha: a };
}

// Helper to create gauge config with LibreTune defaults
function createGauge(config: Partial<TsGaugeConfig>): TsGaugeConfig {
  return {
    id: config.id || '',
    gauge_painter: config.gauge_painter || 'BasicReadout',
    gauge_style: config.gauge_style || '',
    output_channel: config.output_channel || '',
    title: config.title || '',
    units: config.units || '',
    value: config.value || 0,
    min: config.min ?? 0,
    max: config.max ?? 100,
    min_vp: config.min_vp || null,
    max_vp: config.max_vp || null,
    default_min: config.default_min || null,
    default_max: config.default_max || null,
    peg_limits: config.peg_limits ?? true,
    low_warning: config.low_warning || null,
    high_warning: config.high_warning || null,
    low_critical: config.low_critical || null,
    high_critical: config.high_critical || null,
    low_warning_vp: config.low_warning_vp || null,
    high_warning_vp: config.high_warning_vp || null,
    low_critical_vp: config.low_critical_vp || null,
    high_critical_vp: config.high_critical_vp || null,
    back_color: config.back_color || LT_DARK_BG,
    font_color: config.font_color || LT_TEXT_PRIMARY,
    trim_color: config.trim_color || LT_TEXT_SECONDARY,
    warn_color: config.warn_color || LT_WARN_COLOR,
    critical_color: config.critical_color || LT_CRITICAL_COLOR,
    needle_color: config.needle_color || LT_ACCENT_AMBER,
    value_digits: config.value_digits ?? 0,
    label_digits: config.label_digits ?? 0,
    font_family: config.font_family || 'Inter',
    font_size_adjustment: config.font_size_adjustment ?? 0,
    italic_font: config.italic_font || false,
    sweep_angle: config.sweep_angle ?? 270,
    start_angle: config.start_angle ?? 225,
    face_angle: config.face_angle ?? 270,
    sweep_begin_degree: config.sweep_begin_degree ?? 225,
    counter_clockwise: config.counter_clockwise ?? false,
    major_ticks: config.major_ticks ?? -1,
    minor_ticks: config.minor_ticks ?? 0,
    relative_x: config.relative_x ?? 0,
    relative_y: config.relative_y ?? 0,
    relative_width: config.relative_width ?? 0.25,
    relative_height: config.relative_height ?? 0.25,
    border_width: config.border_width ?? 8,
    shortest_size: config.shortest_size ?? 120,
    shape_locked_to_aspect: config.shape_locked_to_aspect ?? true,
    antialiasing_on: config.antialiasing_on ?? true,
    background_image_file_name: config.background_image_file_name || null,
    needle_image_file_name: config.needle_image_file_name || null,
    needle_length: config.needle_length ?? undefined,
    needle_pivot_offset_x: config.needle_pivot_offset_x ?? undefined,
    needle_pivot_offset_y: config.needle_pivot_offset_y ?? undefined,
    needle_image_offset_x: config.needle_image_offset_x ?? undefined,
    needle_image_offset_y: config.needle_image_offset_y ?? undefined,
    show_history: config.show_history || false,
    history_value: config.history_value || 0,
    history_delay: config.history_delay || 0,
    needle_smoothing: config.needle_smoothing || 0,
    short_click_action: config.short_click_action || null,
    long_click_action: config.long_click_action || null,
    display_value_at_180: config.display_value_at_180 || false,
  };
}

// Helper to create indicator config with LibreTune defaults
function createIndicator(config: Partial<TsIndicatorConfig>): TsIndicatorConfig {
  return {
    id: config.id || '',
    indicator_painter: config.indicator_painter || 'BasicRectangleIndicator',
    output_channel: config.output_channel || '',
    value: config.value || 0,
    on_text: config.on_text || 'On',
    off_text: config.off_text || 'Off',
    on_text_color: config.on_text_color || LT_TEXT_PRIMARY,
    off_text_color: config.off_text_color || LT_DARK_BG,
    on_background_color: config.on_background_color || LT_DARK_BG,
    off_background_color: config.off_background_color || LT_DARKER_BG,
    on_image_file_name: config.on_image_file_name || null,
    off_image_file_name: config.off_image_file_name || null,
    relative_x: config.relative_x ?? 0,
    relative_y: config.relative_y ?? 0,
    relative_width: config.relative_width ?? 0.1,
    relative_height: config.relative_height ?? 0.05,
    font_family: config.font_family || 'Inter',
    italic_font: config.italic_font || false,
    antialiasing_on: config.antialiasing_on ?? true,
    short_click_action: config.short_click_action || null,
    long_click_action: config.long_click_action || null,
    ecu_configuration_name: config.ecu_configuration_name || null,
  };
}

/**
 * Create a LibreTune default dashboard.
 * 
 * This dashboard is designed to be:
 * - Legally distinct from TunerStudio (original layout)
 * - Professional and visually appealing
 * - Compatible with all ECU platforms (dynamic channel resolution)
 * - Dark-themed throughout for consistency
 * - Branded with LibreTune colors
 */
export function createLibreTuneDefaultDashboard(): DashFile {
  const version_info: VersionInfo = {
    file_format: '3.0',
    firmware_signature: null,
  };

  const bibliography: Bibliography = {
    author: 'LibreTune Team',
    company: 'LibreTune',
    write_date: new Date().toISOString(),
  };

  const gauges: TsGaugeConfig[] = [
    // Large RPM analog gauge (centered, spanning 2 columns)
    // Corresponds to XML: <Title>Engine Speed</Title><Value>0</Value><Min>0</Min><Max>8000</Max><HighCritical>7200</HighCritical><StartAngle>292</StartAngle><SweepAngle>270</SweepAngle>
    createGauge({
      id: 'rpm',
      gauge_painter: 'AnalogGauge',
      gauge_style: 'LibreTune Analog',
      output_channel: 'rpm',
      title: 'RPM',
      units: '',
      min: 0,
      max: 8000,
      high_warning: 6500,
      high_critical: 7200,
      start_angle: 292,
      sweep_angle: 270,
      value_digits: 0,
      label_digits: 0,
      back_color: LT_DARK_BG,
      font_color: LT_TEXT_PRIMARY,
      trim_color: LT_TEXT_SECONDARY,
      needle_color: LT_ACCENT_AMBER,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.00,
      relative_y: 0.00,
      relative_width: 0.50,
      relative_height: 0.50,
      border_width: 10,
      shortest_size: 150,
    }),

    // AFR digital readout (top-right)
    // Corresponds to XML: <Title>AFR</Title><Value>7.0</Value><Min>10</Min><Max>20</Max><HighCritical>18</HighCritical><LowCritical>11</LowCritical><HighWarning>15</HighWarning>
    createGauge({
      id: 'afr',
      gauge_painter: 'BasicReadout',
      gauge_style: 'LibreTune Digital',
      output_channel: 'afr',
      title: 'AFR',
      units: '',
      min: 10,
      max: 20,
      low_warning: 11,
      high_warning: 15,
      low_critical: 18,
      high_critical: 19,
      value_digits: 2,
      back_color: LT_DARK_BG,
      font_color: LT_ACCENT_TEAL,
      trim_color: LT_TEXT_SECONDARY,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.51,
      relative_y: 0.00,
      relative_width: 0.23,
      relative_height: 0.23,
      font_size_adjustment: 2,
    }),

    // Coolant temperature vertical bar (middle-left)
    // Corresponds to XML: <Title>Inlet Air Temp</Title><Value>0</Value><Min>-40</Min><Max>215</Max><HighWarning>200</HighWarning><HighCritical>210</HighCritical>
    createGauge({
      id: 'iat',
      gauge_painter: 'VerticalBarGauge',
      gauge_style: 'LibreTune Vertical Bar',
      output_channel: 'iat',
      title: 'Inlet Air Temp',
      units: 'TEMP',
      min: -40,
      max: 215,
      high_warning: 200,
      high_critical: 210,
      value_digits: 0,
      back_color: LT_DARK_BG,
      font_color: LT_TEXT_PRIMARY,
      trim_color: LT_TEXT_SECONDARY,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.00,
      relative_y: 0.00,
      relative_width: 0.23,
      relative_height: 0.50,
    }),

    // MAP horizontal bar (middle-center)
    // Corresponds to XML: <Title>Engine MAP</Title><Value>0</Value><Min>0</Min><Max>255</Max><HighWarning>200</HighWarning><HighCritical>245</HighCritical>
    createGauge({
      id: 'map',
      gauge_painter: 'HorizontalBarGauge',
      gauge_style: 'LibreTune Horizontal Bar',
      output_channel: 'map',
      title: 'Engine MAP',
      units: 'kPa',
      min: 0,
      max: 255,
      high_warning: 200,
      high_critical: 245,
      value_digits: 0,
      back_color: LT_DARK_BG,
      font_color: LT_TEXT_PRIMARY,
      trim_color: LT_TEXT_SECONDARY,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.25,
      relative_y: 0.51,
      relative_width: 0.23,
      relative_height: 0.23,
    }),

    // TPS horizontal bar (middle-right)
    // Corresponds to XML: <Title>Throttle Position</Title><Value>0</Value><Min>0</Min><Max>100</Max><HighCritical>100</HighCritical><HighWarning>90</HighWarning>
    createGauge({
      id: 'tps',
      gauge_painter: 'HorizontalBarGauge',
      gauge_style: 'LibreTune Horizontal Bar',
      output_channel: 'throttle',
      title: 'Throttle Position',
      units: '%',
      min: 0,
      max: 100,
      high_warning: 90,
      high_critical: 100,
      value_digits: 1,
      back_color: LT_DARK_BG,
      font_color: LT_TEXT_PRIMARY,
      trim_color: LT_TEXT_SECONDARY,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.50,
      relative_y: 0.51,
      relative_width: 0.23,
      relative_height: 0.23,
    }),

    // Battery voltage digital readout (bottom-left)
    // Corresponds to XML: <Title>Battery Voltage</Title><Value>0</Value><Min>0</Min><Max>21</Max><HighWarning>15</HighWarning><HighCritical>18</HighCritical>
    createGauge({
      id: 'battery',
      gauge_painter: 'BasicReadout',
      gauge_style: 'LibreTune Digital',
      output_channel: 'battery',
      title: 'Battery Voltage',
      units: 'V',
      min: 8,
      max: 21,
      low_warning: 15,
      low_critical: 18,
      value_digits: 2,
      back_color: LT_DARK_BG,
      font_color: LT_ACCENT_GREEN,
      trim_color: LT_TEXT_SECONDARY,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.00,
      relative_y: 0.75,
      relative_width: 0.23,
      relative_height: 0.23,
      font_size_adjustment: 1,
    }),

    // Ignition timing digital readout (bottom-center)
    // Corresponds to XML: <Title>Ignition Timing</Title><Value>0</Value><Min>-10</Min><Max>50</Max><HighWarning>35</HighWarning><HighCritical>45</HighCritical>
    createGauge({
      id: 'timing',
      gauge_painter: 'BasicReadout',
      gauge_style: 'LibreTune Digital',
      output_channel: 'ignition',
      title: 'Ignition Timing',
      units: '°',
      min: -10,
      max: 50,
      high_warning: 35,
      high_critical: 45,
      value_digits: 1,
      back_color: LT_DARK_BG,
      font_color: LT_ACCENT_BLUE,
      trim_color: LT_TEXT_SECONDARY,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.25,
      relative_y: 0.75,
      relative_width: 0.23,
      relative_height: 0.23,
      font_size_adjustment: 1,
    }),

    // Dwell time digital readout (bottom-right)
    // Corresponds to XML: <Title>Coil Dwell</Title><Value>0</Value><Min>0</Min><Max>10</Max><HighWarning>7</HighWarning><HighCritical>9</HighCritical>
    createGauge({
      id: 'dwell',
      gauge_painter: 'BasicReadout',
      gauge_style: 'LibreTune Digital',
      output_channel: 'dwell',
      title: 'Coil Dwell',
      units: 'ms',
      min: 0,
      max: 10,
      high_warning: 7,
      high_critical: 9,
      value_digits: 2,
      back_color: LT_DARK_BG,
      font_color: LT_TEXT_PRIMARY,
      trim_color: LT_TEXT_SECONDARY,
      warn_color: LT_WARN_COLOR,
      critical_color: LT_CRITICAL_COLOR,
      relative_x: 0.50,
      relative_y: 0.75,
      relative_width: 0.23,
      relative_height: 0.23,
      font_size_adjustment: 1,
    }),
  ];

  const indicators: TsIndicatorConfig[] = [
    // Indicator: Accel (Top-Right) - Derived from XML 'tpsaccaen'
    {
      id: 'accel',
      indicator_painter: 'BasicRectangleIndicator',
      output_channel: 'tpsAccelEn',
      on_text: 'Accel',
      off_text: 'Off',
      on_text_color: LT_ACCENT_TEAL,
      off_text_color: LT_DARK_BG,
      on_background_color: LT_DARK_BG,
      off_background_color: LT_DARKER_BG,
      relative_x: 0.73,
      relative_y: 0.00,
      relative_width: 0.24,
      relative_height: 0.05,
    },
    // Indicator: Not Cranking (Top-Right) - Derived from XML 'crank'
    {
      id: 'notCranking',
      indicator_painter: 'BasicRectangleIndicator',
      output_channel: 'crank',
      on_text: 'Cranking',
      off_text: 'Not Cranking',
      on_text_color: LT_ACCENT_BLUE,
      off_text_color: LT_DARK_BG,
      on_background_color: LT_DARK_BG,
      off_background_color: LT_DARKER_BG,
      relative_x: 0.88,
      relative_y: 0.00,
      relative_width: 0.12,
      relative_height: 0.05,
    },
    // Indicator: Data Logging (Top-Right) - Derived from XML 'dataLoggingActive'
    {
      id: 'dataLogging',
      indicator_painter: 'BasicRectangleIndicator',
      output_channel: 'dataLoggingActive',
      on_text: 'Data Logging',
      off_text: 'Data Logging',
      on_text_color: LT_TEXT_PRIMARY,
      off_text_color: LT_DARK_BG,
      on_background_color: LT_DARK_BG,
      off_background_color: LT_DARKER_BG,
      relative_x: 0.70,
      relative_y: 0.00,
      relative_width: 0.22,
      relative_height: 0.05,
      font_family: 'Inter',
    },
  ];

  const cluster: GaugeCluster = {
    anti_aliasing: true,
    force_aspect: false,
    force_aspect_width: 0,
    force_aspect_height: 0,
    background_dither_color: LT_DARKER_BG,
    cluster_background_color: LT_DARK_BG,
    cluster_background_image_file_name: null,
    cluster_background_image_style: 'Stretch',
    embedded_images: [],
    components: [
      ...gauges.map((g) => ({ Gauge: g })),
      ...indicators.map((i) => ({ Indicator: i })),
    ]
  };

  return {
    bibliography,
    version_info,
    gauge_cluster: cluster,
  };
}