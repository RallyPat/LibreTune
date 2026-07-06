/** Map INI indicator color names to flat, vibrant UI colors. */
export function resolveIndicatorColor(name?: string | null): string | undefined {
  if (!name) return undefined;
  const key = name.trim().toLowerCase();
  const palette: Record<string, string> = {
    green: '#16F529',
    lime: '#16F529',
    red: '#FF453A',
    yellow: '#FFD60A',
    white: '#F2F2F7',
    black: '#000000',
    orange: '#FF9500',
    blue: '#0A84FF',
    gray: '#8E8E93',
    grey: '#8E8E93',
  };
  return palette[key] ?? name;
}
