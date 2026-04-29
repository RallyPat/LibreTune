
import { Plus, Copy, Pencil, Trash2, Save, RotateCw, AlertTriangle, Compass } from 'lucide-react';
import { ValidationReport } from '../utils/validation';

interface Props {
  title: string;
  showSelector: boolean;
  onToggleSelector: () => void;
  onNew: () => void;
  onDuplicate: () => void;
  onRename: () => void;
  onDelete: () => void;
  onExport: () => void;
  onSyncRanges: () => void;
  validationReport: ValidationReport | null;
  onToggleValidationPanel: () => void;
  legacyMode: boolean;
  onToggleLegacyMode: () => void;
}

/**
 * Dashboard toolbar header with title and action buttons.
 * Extracted from TsDashboard during Phase C4.
 */
export default function DashboardHeader({
  title,
  onToggleSelector,
  onNew,
  onDuplicate,
  onRename,
  onDelete,
  onExport,
  onSyncRanges,
  validationReport,
  onToggleValidationPanel,
  legacyMode,
  onToggleLegacyMode,
}: Props) {
  return (
    <div className="ts-dashboard-header">
      <div className="ts-dashboard-header-left">
        <span className="ts-dashboard-title">{title}</span>
        <button className="ts-dashboard-selector-btn" onClick={onToggleSelector}>
          Change ▼
        </button>
      </div>
      <div className="ts-dashboard-header-right">
        <button className="ts-dashboard-action-btn" onClick={onNew} title="New Dashboard">
          <Plus size={14} /> New
        </button>
        <button className="ts-dashboard-action-btn" onClick={onDuplicate} title="Duplicate Dashboard">
          <Copy size={14} /> Duplicate
        </button>
        <button className="ts-dashboard-action-btn" onClick={onRename} title="Rename Dashboard">
          <Pencil size={14} /> Rename
        </button>
        <button className="ts-dashboard-action-btn danger" onClick={onDelete} title="Delete Dashboard">
          <Trash2 size={14} /> Delete
        </button>
        <button className="ts-dashboard-action-btn" onClick={onExport} title="Export Dashboard">
          <Save size={14} /> Export
        </button>
        <button
          className="ts-dashboard-action-btn"
          onClick={onSyncRanges}
          title="Sync gauge ranges from INI"
        >
          <RotateCw size={14} /> Sync Ranges
        </button>
        {validationReport && (
          <button
            className={`ts-dashboard-action-btn ${
              validationReport.errors.length > 0
                ? 'danger'
                : validationReport.warnings.length > 0
                  ? 'warn'
                  : ''
            }`}
            onClick={onToggleValidationPanel}
            title="Dashboard validation issues"
          >
            <AlertTriangle size={14} /> Validate ({validationReport.errors.length}E/
            {validationReport.warnings.length}W)
          </button>
        )}
        <button
          className={`ts-dashboard-action-btn ${legacyMode ? 'active' : ''}`}
          onClick={onToggleLegacyMode}
          title={legacyMode ? 'Legacy TS layout enabled' : 'Enable legacy TS layout'}
        >
          <Compass size={14} /> Legacy: {legacyMode ? 'On' : 'Off'}
        </button>
      </div>
    </div>
  );
}
