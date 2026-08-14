
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X, ArrowRight } from 'lucide-react';
import { formatValidationIssue, ValidationReport } from '../utils/validation';
import type { DashFile } from '../dashTypes';

/** Suggested channel substitution returned by `suggest_channel_remaps`. */
export interface ChannelRemap {
  component_id: string;
  from_channel: string;
  to_channel: string;
  match_kind: string;
}

interface Props {
  report: ValidationReport;
  dashFile: DashFile;
  onApplyRemap: (remap: ChannelRemap) => void;
  onClose: () => void;
}

/**
 * Validation panel that lists errors and warnings for the current dashboard,
 * plus suggested channel remaps (cross-firmware synonyms) for unknown
 * channels, each applicable with one click.
 */
export default function ValidationPanel({ report, dashFile, onApplyRemap, onClose }: Props) {
  const [remaps, setRemaps] = useState<ChannelRemap[]>([]);

  // Fetch remap suggestions whenever the underlying file (and thus the
  // report) changes — applied remaps change the file, so the list
  // refreshes and applied entries drop out.
  useEffect(() => {
    let cancelled = false;
    invoke<ChannelRemap[]>('suggest_channel_remaps', { dashFile })
      .then((suggestions) => {
        if (!cancelled) setRemaps(suggestions ?? []);
      })
      .catch(() => {
        if (!cancelled) setRemaps([]);
      });
    return () => {
      cancelled = true;
    };
  }, [dashFile]);

  return (
    <div className="ts-dashboard-validation">
      <div className="ts-dashboard-validation-header">
        <div>
          Validation: {report.errors.length} error(s), {report.warnings.length} warning(s)
        </div>
        <button
          className="ts-dashboard-compat-close"
          onClick={onClose}
          title="Dismiss"
          aria-label="Dismiss"
        >
          <X size={14} />
        </button>
      </div>
      {report.errors.length === 0 && report.warnings.length === 0 && remaps.length === 0 ? (
        <div className="ts-dashboard-validation-empty">No issues detected.</div>
      ) : (
        <div className="ts-dashboard-validation-body">
          {remaps.length > 0 && (
            <div className="ts-dashboard-validation-section">
              <h4>Suggested channel remaps</h4>
              <ul>
                {remaps.map((remap) => (
                  <li key={`${remap.component_id}-${remap.to_channel}`}>
                    <code>{remap.from_channel}</code>
                    <ArrowRight size={12} />
                    <code>{remap.to_channel}</code>
                    <button
                      className="ts-dashboard-action-btn"
                      onClick={() => onApplyRemap(remap)}
                      title={`Apply ${remap.match_kind} match`}
                    >
                      Apply
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {report.errors.length > 0 && (
            <div className="ts-dashboard-validation-section">
              <h4>Errors</h4>
              <ul>
                {report.errors.map((issue, idx) => (
                  <li key={`err-${idx}`}>{formatValidationIssue(issue)}</li>
                ))}
              </ul>
            </div>
          )}
          {report.warnings.length > 0 && (
            <div className="ts-dashboard-validation-section">
              <h4>Warnings</h4>
              <ul>
                {report.warnings.map((issue, idx) => (
                  <li key={`warn-${idx}`}>{formatValidationIssue(issue)}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
