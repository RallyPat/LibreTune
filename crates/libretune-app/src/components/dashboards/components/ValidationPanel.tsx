
import { X } from 'lucide-react';
import { formatValidationIssue, ValidationReport } from '../utils/validation';

interface Props {
  report: ValidationReport;
  onClose: () => void;
}

/**
 * Validation panel that lists errors and warnings for the current dashboard.
 * Extracted from TsDashboard during Phase C4.
 */
export default function ValidationPanel({ report, onClose }: Props) {
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
      {report.errors.length === 0 && report.warnings.length === 0 ? (
        <div className="ts-dashboard-validation-empty">No issues detected.</div>
      ) : (
        <div className="ts-dashboard-validation-body">
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
