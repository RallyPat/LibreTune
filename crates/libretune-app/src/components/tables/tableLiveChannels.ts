/** Resolve user-table output channel from table / axis channel names (User tables 1–8). */
export function resolveUserTableOutputChannel(
  tableName: string,
  xChannel?: string | null,
  yChannel?: string | null,
): string | null {
  const fromTable = tableName.match(/^userTable(\d)(?:Tbl|Map)$/i);
  if (fromTable) return `userTableOutput${fromTable[1]}`;

  const fromX = xChannel?.match(/^userTableXAxis(\d+)$/i);
  if (fromX) return `userTableOutput${fromX[1]}`;

  const fromY = yChannel?.match(/^userTableYAxis(\d+)$/i);
  if (fromY) return `userTableOutput${fromY[1]}`;

  return null;
}

export function isUserTable(
  tableName: string,
  xChannel?: string | null,
  yChannel?: string | null,
): boolean {
  return resolveUserTableOutputChannel(tableName, xChannel, yChannel) !== null;
}

export function hasEmbeddedTableLiveReadout(
  tableName: string,
  xChannel?: string | null,
  yChannel?: string | null,
): boolean {
  return isGppwmTable(tableName, xChannel, yChannel) || isUserTable(tableName, xChannel, yChannel);
}

/** Output channel for embedded table live cursor / readout (GPPWM or user table). */
export function resolveEmbeddedTableOutputChannel(
  tableName: string,
  xChannel?: string | null,
  yChannel?: string | null,
): string | null {
  return (
    resolveGppwmOutputChannel(tableName, xChannel, yChannel) ??
    resolveUserTableOutputChannel(tableName, xChannel, yChannel)
  );
}

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

  if (channel?.match(/^userTableOutput\d$/i)) {
    if (Number.isInteger(value) || Math.abs(value - Math.round(value)) < 0.001) {
      return String(Math.round(value));
    }
    return value.toFixed(3);
  }

  if (channel?.match(/^gppwmOutput\d$/i) || kind === 'output') {
    return `${value.toFixed(1)}%`;
  }

  if (channel?.match(/^userTable[XY]Axis\d$/i)) {
    if (Number.isInteger(value) || Math.abs(value - Math.round(value)) < 0.001) {
      return String(Math.round(value));
    }
    return value.toFixed(1);
  }

  if (channel?.match(/^gppwmYAxis\d$/i)) {
    return value.toFixed(1);
  }

  if (Number.isInteger(value) || Math.abs(value - Math.round(value)) < 0.001) {
    return String(Math.round(value));
  }

  return value.toFixed(1);
}
