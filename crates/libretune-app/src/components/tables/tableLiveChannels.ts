/** Resolve GPPWM output channel from table / axis channel names (GP#1–4). */
export function resolveGppwmOutputChannel(
  tableName: string,
  xChannel?: string | null,
  yChannel?: string | null,
): string | null {
  const fromTable = tableName.match(/^gppwm(\d)(?:Tbl|Map)$/i);
  if (fromTable) return `gppwmOutput${fromTable[1]}`;

  const fromX = xChannel?.match(/^gppwmXAxis(\d)$/i);
  if (fromX) return `gppwmOutput${fromX[1]}`;

  const fromY = yChannel?.match(/^gppwmYAxis(\d)$/i);
  if (fromY) return `gppwmOutput${fromY[1]}`;

  return null;
}

export function isGppwmTable(
  tableName: string,
  xChannel?: string | null,
  yChannel?: string | null,
): boolean {
  return resolveGppwmOutputChannel(tableName, xChannel, yChannel) !== null;
}

export function formatLiveReadoutValue(
  value: number | undefined,
  kind: 'switch' | 'axis' | 'output',
  channel?: string,
): string {
  if (value === undefined || Number.isNaN(value)) return '—';

  if (kind === 'switch') {
    return value > 0 ? '1' : '0';
  }

  if (kind === 'output' || channel?.match(/^gppwmOutput\d$/i)) {
    return `${value.toFixed(1)}%`;
  }

  if (channel?.match(/^gppwmYAxis\d$/i)) {
    return value.toFixed(1);
  }

  if (Number.isInteger(value) || Math.abs(value - Math.round(value)) < 0.001) {
    return String(Math.round(value));
  }

  return value.toFixed(1);
}
