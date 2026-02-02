import { TsColor, TsGaugeConfig, TsIndicatorConfig, GaugeCluster, DashFile, Bibliography, VersionInfo } from './dashTypes';

// Helper to create a TsColor
function color(r: number, g: number, b: number, a: number = 255): TsColor {
  return { red: r, green: g, blue: b, alpha: a };
}

// LibreTune default dashboard: legally distinct, original, but config-compatible
export function createLibreTuneDefaultDashboard(): DashFile {
    const version_info: VersionInfo = {
      file_format: '3.0',
      firmware_signature: null,
    };

    // 8 round analog gauges in 2x4 grid, config inspired by TS screenshot
    const gauges: TsGaugeConfig[] = [
      {
        id: 'rpm', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'RPM', title: 'RPM', units: 'x1000 RPM', value: 0, min: 0, max: 9000, default_min: 0, default_max: 9000, peg_limits: true, low_warning: 7000, high_warning: 8000, low_critical: 8500, high_critical: 9000, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(255,80,80), value_digits: 0, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 9, minor_ticks: 3, relative_x: 0.00, relative_y: 0.00, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
      {
        id: 'clt', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'CLT', title: 'Coolant temp', units: 'C', value: 0, min: -40, max: 140, default_min: -40, default_max: 140, peg_limits: true, low_warning: 100, high_warning: 120, low_critical: 130, high_critical: 140, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(80,180,255), value_digits: 1, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 10, minor_ticks: 2, relative_x: 0.25, relative_y: 0.00, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
      {
        id: 'tps', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'TPS', title: 'Throttle position', units: '%', value: 0, min: 0, max: 100, default_min: 0, default_max: 100, peg_limits: true, low_warning: null, high_warning: 90, low_critical: null, high_critical: 100, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(255,80,80), value_digits: 1, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 10, minor_ticks: 2, relative_x: 0.50, relative_y: 0.00, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
      {
        id: 'map', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'MAP', title: 'MAP', units: 'kPa', value: 0, min: 0, max: 300, default_min: 0, default_max: 300, peg_limits: true, low_warning: 200, high_warning: 250, low_critical: 280, high_critical: 300, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(80,255,80), value_digits: 0, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 10, minor_ticks: 2, relative_x: 0.75, relative_y: 0.00, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
      {
        id: 'afr', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'AFR', title: 'Air/Fuel Ratio', units: '', value: 0, min: 10, max: 19, default_min: 10, default_max: 19, peg_limits: true, low_warning: 16, high_warning: 17, low_critical: 18, high_critical: 19, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(255,80,80), value_digits: 2, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 9, minor_ticks: 3, relative_x: 0.00, relative_y: 0.52, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
      {
        id: 'battery', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'Battery', title: 'Battery', units: 'V', value: 0, min: 8, max: 21, default_min: 8, default_max: 21, peg_limits: true, low_warning: 15, high_warning: 17, low_critical: 18, high_critical: 21, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(255,255,80), value_digits: 2, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 7, minor_ticks: 2, relative_x: 0.25, relative_y: 0.52, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
      {
        id: 'dwell', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'Dwell', title: 'Dwell', units: 'mSec', value: 0, min: 0, max: 9, default_min: 0, default_max: 9, peg_limits: true, low_warning: 7, high_warning: 8, low_critical: 9, high_critical: 9, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(255,80,80), value_digits: 2, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 9, minor_ticks: 3, relative_x: 0.50, relative_y: 0.52, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
      {
        id: 'timing', gauge_painter: 'AnalogGauge', gauge_style: 'Classic', output_channel: 'Timing', title: 'Timing: ignition', units: 'deg', value: 0, min: -100, max: 100, default_min: -100, default_max: 100, peg_limits: true, low_warning: 60, high_warning: 80, low_critical: 90, high_critical: 100, back_color: color(245,245,245), font_color: color(30,30,30), trim_color: color(120,120,120), warn_color: color(255,255,40), critical_color: color(255,60,60), needle_color: color(255,80,80), value_digits: 2, label_digits: 0, font_family: 'Inter', font_size_adjustment: 0, italic_font: false, sweep_angle: 270, start_angle: 225, face_angle: 0, sweep_begin_degree: 0, counter_clockwise: false, major_ticks: 10, minor_ticks: 2, relative_x: 0.75, relative_y: 0.52, relative_width: 0.24, relative_height: 0.48, border_width: 8, shortest_size: 120, shape_locked_to_aspect: true, antialiasing_on: true, background_image_file_name: null, needle_image_file_name: null, show_history: false, history_value: 0, history_delay: 0, needle_smoothing: 0, short_click_action: null, long_click_action: null, display_value_at_180: false, min_vp: null, max_vp: null, low_warning_vp: null, high_warning_vp: null, low_critical_vp: null, high_critical_vp: null },
    ];
  const bibliography: Bibliography = {
    author: 'LibreTune Team',
    company: 'LibreTune',
    write_date: new Date().toISOString(),
  };

  // Indicators: dynamic, driven by INI (not hardcoded)
  const indicators: TsIndicatorConfig[] = [];

  const cluster: GaugeCluster = {
    anti_aliasing: true,
    force_aspect: false,
    force_aspect_width: 0,
    force_aspect_height: 0,
    background_dither_color: color(24, 24, 32, 64),
    cluster_background_color: color(18, 20, 28),
    cluster_background_image_file_name: null,
    cluster_background_image_style: 'Stretch',
    embedded_images: [],
    components: [
      ...gauges.map((g) => ({ Gauge: g })),
      ...indicators.map((i) => ({ Indicator: i })),
    ],
  };

  return {
    bibliography,
    version_info,
    gauge_cluster: cluster,
  };
}
