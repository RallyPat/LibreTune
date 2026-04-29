/**
 * Shape of a single validation issue returned by the
 * `validate_dashboard` Tauri command. Issues are tagged maps where the
 * single key identifies the rule and the value carries detail fields.
 */
export type ValidationIssue = Record<string, unknown>;

export interface ValidationReport {
  errors: ValidationIssue[];
  warnings: ValidationIssue[];
  stats: {
    gauge_count: number;
    indicator_count: number;
    unique_channels: number;
    embedded_image_count: number;
    has_embedded_fonts: boolean;
  };
}

/** Render a single validation issue map as a human-readable string. */
export function formatValidationIssue(issue: ValidationIssue): string {
  const entries = Object.entries(issue);
  if (entries.length === 0) return 'Unknown issue';
  const [kind, details] = entries[0];
  if (details && typeof details === 'object') {
    const parts = Object.entries(details as Record<string, unknown>)
      .map(([key, value]) => `${key}: ${String(value)}`)
      .join(', ');
    return `${kind} (${parts})`;
  }
  return kind;
}
