import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { lineGraphPainter } from '../lineGraph';
import { getChannelHistoryBuffer, useRealtimeStore } from '../../../../stores/realtimeStore';
import type { TsGaugeConfig, TsColor } from '../../../dashboards/dashTypes';

const mockCtx = (): CanvasRenderingContext2D =>
  ({
    setTransform: vi.fn(),
    scale: vi.fn(),
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    stroke: vi.fn(),
    fill: vi.fn(),
    fillText: vi.fn(),
    strokeText: vi.fn(),
    measureText: () => ({ width: 0 }),
    quadraticCurveTo: vi.fn(),
    createLinearGradient: () => ({ addColorStop: vi.fn() }),
    arc: vi.fn(),
    setLineDash: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    shadowColor: '',
    shadowBlur: 0,
    shadowOffsetY: 0,
    lineWidth: 1,
    lineCap: 'butt',
    lineJoin: 'miter',
    fillStyle: '',
    strokeStyle: '',
    textAlign: 'left',
    textBaseline: 'top',
    font: '',
  } as unknown as CanvasRenderingContext2D);

const baseConfig: TsGaugeConfig = {
  id: 'lambda_hist',
  title: 'LAMBDA TREND',
  units: 'λ',
  output_channel: 'lambda',
  min: 0.7,
  max: 1.3,
  value: 1.0,
  gauge_painter: 'LineGraph',
  gauge_style: '',
  relative_x: 0,
  relative_y: 0,
  relative_width: 1,
  relative_height: 1,
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
  back_color: { alpha: 255, red: 28, green: 32, blue: 40 },
  font_color: { alpha: 255, red: 34, green: 197, blue: 94 },
  needle_color: { alpha: 255, red: 34, green: 197, blue: 94 },
  trim_color: { alpha: 255, red: 148, green: 163, blue: 184 },
  warn_color: { alpha: 255, red: 234, green: 179, blue: 8 },
  critical_color: { alpha: 255, red: 239, green: 68, blue: 68 },
  value_digits: 3,
  label_digits: 0,
  font_family: 'Arial',
  font_size_adjustment: 0,
  italic_font: false,
  sweep_angle: 270,
  start_angle: 225,
  face_angle: 0,
  sweep_begin_degree: 0,
  counter_clockwise: false,
  major_ticks: 10,
  minor_ticks: 5,
  border_width: 0,
  shortest_size: 0,
  shape_locked_to_aspect: false,
  antialiasing_on: true,
  background_image_file_name: null,
  needle_image_file_name: null,
  peak_hold: false,
  history_value: 0,
  history_delay: 0,
  needle_smoothing: 0,
  short_click_action: null,
  long_click_action: null,
  display_value_at_180: false,
};

describe('lineGraphPainter', () => {
  beforeEach(() => {
    useRealtimeStore.getState().clearChannels();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('does not call Math.random when no channel history exists', () => {
    const randomSpy = vi.spyOn(Math, 'random').mockImplementation(() => 0.5);

    const ctx = mockCtx();
    lineGraphPainter({
      ctx,
      width: 200,
      height: 100,
      value: 1.0,
      peakValue: 1.0,
      config: baseConfig,
      legacyMode: false,
      bgImage: null,
      needleImage: null,
      getValueColor: () => baseConfig.font_color as TsColor,
      getFontSpec: (size) => `${size}px Arial`,
      getFontFamily: () => 'Arial',
      getEmbeddedImage: () => null,
    });

    expect(randomSpy).not.toHaveBeenCalled();
  });

  it('renders a deterministic flat line when history is empty', () => {
    const ctx = mockCtx();
    const lineToCalls: { x: number; y: number }[] = [];
    (ctx.lineTo as any).mockImplementation((x: number, y: number) => {
      lineToCalls.push({ x, y });
    });

    lineGraphPainter({
      ctx,
      width: 200,
      height: 100,
      value: 1.0,
      peakValue: 1.0,
      config: baseConfig,
      legacyMode: false,
      bgImage: null,
      needleImage: null,
      getValueColor: () => baseConfig.font_color as TsColor,
      getFontSpec: (size) => `${size}px Arial`,
      getFontFamily: () => 'Arial',
      getEmbeddedImage: () => null,
    });

    // With empty history the painter should emit trace points.
    expect(lineToCalls.length).toBeGreaterThan(0);

    // With numPoints=50 the line trace contributes 50 identical line-to calls,
    // plus one closing baseline point for the filled area. Verify the trace is
    // deterministic by counting the dominant y value.
    const ys = lineToCalls.map((p) => p.y);
    const counts = new Map<number, number>();
    for (const y of ys) {
      counts.set(y, (counts.get(y) ?? 0) + 1);
    }
    const largestCount = Math.max(...counts.values());
    expect(largestCount).toBeGreaterThanOrEqual(50);
  });

  it('uses actual history when available', () => {
    // Seed the history buffer for the lambda channel.
    useRealtimeStore.getState().updateChannels({ lambda: 0.75 });
    useRealtimeStore.getState().updateChannels({ lambda: 0.9 });
    useRealtimeStore.getState().updateChannels({ lambda: 1.1 });

    const history = getChannelHistoryBuffer('lambda');
    expect(history.length).toBeGreaterThan(0);

    const ctx = mockCtx();
    const lineToCalls: { x: number; y: number }[] = [];
    (ctx.lineTo as any).mockImplementation((x: number, y: number) => {
      lineToCalls.push({ x, y });
    });

    lineGraphPainter({
      ctx,
      width: 200,
      height: 100,
      value: 1.0,
      peakValue: 1.0,
      config: baseConfig,
      legacyMode: false,
      bgImage: null,
      needleImage: null,
      getValueColor: () => baseConfig.font_color as TsColor,
      getFontSpec: (size) => `${size}px Arial`,
      getFontFamily: () => 'Arial',
      getEmbeddedImage: () => null,
    });

    // With history we should get at least as many line points as history entries.
    expect(lineToCalls.length).toBeGreaterThanOrEqual(history.length);
  });
});
