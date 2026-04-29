
import { X } from 'lucide-react';

interface Props {
  onClose: () => void;
}

/**
 * Compatibility warning banner for legacy/unsupported features.
 * Extracted from TsDashboard during Phase C4.
 */
export default function CompatibilityBar({ onClose }: Props) {
  return (
    <div className="ts-dashboard-compat warn">
      <span>Compatibility: some features not yet supported</span>
      <button
        className="ts-dashboard-compat-close"
        onClick={onClose}
        title="Dismiss"
        aria-label="Dismiss"
      >
        <X size={14} />
      </button>
    </div>
  );
}
