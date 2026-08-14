/**
 * Dashboard filename helpers.
 *
 * Dashboard paths come from the Rust backend and use the OS separator
 * (backslash on Windows), so basename extraction must handle both. The
 * `.ltdash.xml` suffix is a double extension and must be stripped as one
 * unit, otherwise "My Dash.ltdash" leaks into titles and default names.
 */

const DASH_EXT_RE = /\.(ltdash\.xml|dash|gauge)$/i;

/** Basename of a dashboard path, with extension(s) removed. */
export function dashBaseName(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? '';
  return base.replace(DASH_EXT_RE, '');
}

/** Full filename (with extension) of a dashboard path. */
export function dashFileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? '';
}

/** True when the path refers to a legacy TunerStudio file. */
export function isLegacyDashPath(path: string): boolean {
  const lower = path.toLowerCase();
  return lower.endsWith('.dash') || lower.endsWith('.gauge');
}
