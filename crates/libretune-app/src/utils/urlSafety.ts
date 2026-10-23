/**
 * URL safety helpers for external content.
 *
 * Values that originate outside the app (INI definition files, project
 * metadata, ...) must never be handed to `window.open`, the opener plugin,
 * or an anchor `href` without restricting the scheme. A `javascript:` or
 * `data:` URL from a crafted INI would execute with the page's origin —
 * inside a Tauri webview that means access to the full IPC command
 * surface.
 */

/** Only allow http(s) URLs from untrusted sources (mailto allowed optionally). */
export function isSafeWebUrl(url: string | undefined | null, allowMailto = false): boolean {
  if (!url) return false;
  const trimmed = url.trim().toLowerCase();
  if (trimmed.startsWith('javascript:') || trimmed.startsWith('data:') || trimmed.startsWith('vbscript:')) {
    return false;
  }
  if (allowMailto && trimmed.startsWith('mailto:')) return true;
  try {
    const parsed = new URL(trimmed);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    // Not a parseable absolute URL (relative path, fragment, etc.) — reject.
    return false;
  }
}
