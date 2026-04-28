/**
 * Default `TsGaugeConfig` used when a new gauge is created via drag-and-drop
 * onto the dashboard canvas. Position is overridden by the drop handler.
 */

import type { TsGaugeConfig } from '../dashTypes';

export interface DefaultGaugeOptions {
  id: string;
  channel: string;
  title: string;
  units: string;
  /** Drop position in 0..1 dashboard coordinates (top-left of the new gauge). */
  relativeX: number;
  relativeY: number;
}

export function buildDefaultGauge(opts: DefaultGaugeOptions): TsGaugeConfig {
  return {
    id: opts.id,
    gauge_painter: 'BasicReadout',
    gauge_style: '',
    output_channel: opts.channel,
    title: opts.title,
    units: opts.units,
    value: 0,
    min: 0,
    max: 100,
    min_vp: null,
    max_vp: null,
    default_min: null,
    default_max: null,
    peg_limits: false,
    low_warning: null,
    high_warning: null,
    low_critical: null,
    high_critical: null,
    low_warning_vp: null,
    high_warning_vp: null,
    low_critical_vp: null,
    high_critical_vp: null,
    back_color: { alpha: 0, red: 40, green: 40, blue: 40 },
    font_color: { alpha: 0, red: 255, green: 255, blue: 255 },
    trim_color: { alpha: 0, red: 100, green: 100, blue: 100 },
    warn_color: { alpha: 0, red: 255, green: 165, blue: 0 },
    critical_color: { alpha: 0, red: 255, green: 0, blue: 0 },
    needle_color: { alpha: 0, red: 200, green: 200, blue: 200 },
    value_digits: 2,
    label_digits: 0,
    font_family: 'Arial',
    font_size_adjustment: 0,
    italic_font: false,
    sweep_angle: 270,
    start_angle: 225,
    face_angle: 0,
    sweep_begin_degree: 0,
    counter_clockwise: false,
    major_ticks: 5,
    minor_ticks: 0,
    relative_x: opts.relativeX,
    relative_y: opts.relativeY,
    relative_width: 0.2,
    relative_height: 0.2,
    border_width: 1,
    shortest_size: 0,
    shape_locked_to_aspect: false,
    antialiasing_on: true,
    background_image_file_name: null,
    needle_image_file_name: null,
    show_history: false,
    history_value: 0,
    history_delay: 0,
    needle_smoothing: 0,
    short_click_action: null,
    long_click_action: null,
    display_value_at_180: false,
  };
}
