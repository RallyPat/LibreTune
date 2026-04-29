import {
  Undo2, Redo2, Copy, Clipboard, Trash2, Grid3X3, Save, X,
  AlignStartHorizontal, AlignCenterHorizontal, AlignEndHorizontal,
  AlignStartVertical, AlignCenterVertical, AlignEndVertical,
} from 'lucide-react';

interface DesignerToolbarProps {
  canUndo: boolean;
  canRedo: boolean;
  hasClipboard: boolean;
  hasSelection: boolean;
  showGrid: boolean;
  gridSnap: number;
  onUndo: () => void;
  onRedo: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onDelete: () => void;
  onShowGridChange: (v: boolean) => void;
  onGridSnapChange: (v: number) => void;
  onSave: () => void;
  onExit: () => void;
  /** Plan v2 / D-7a — align selection to canvas. Disabled when no selection. */
  onAlign?: (edge: 'left' | 'hcenter' | 'right' | 'top' | 'vcenter' | 'bottom') => void;
}

export default function DesignerToolbar({
  canUndo,
  canRedo,
  hasClipboard,
  hasSelection,
  showGrid,
  gridSnap,
  onUndo,
  onRedo,
  onCopy,
  onPaste,
  onDelete,
  onShowGridChange,
  onGridSnapChange,
  onSave,
  onExit,
  onAlign,
}: DesignerToolbarProps) {
  return (
    <div className="designer-toolbar">
      <div className="toolbar-group">
        <button className="toolbar-btn" onClick={onUndo} disabled={!canUndo} title="Undo (Ctrl+Z)">
          <Undo2 size={16} />
        </button>
        <button className="toolbar-btn" onClick={onRedo} disabled={!canRedo} title="Redo (Ctrl+Y)">
          <Redo2 size={16} />
        </button>
      </div>

      <div className="toolbar-separator" />

      <div className="toolbar-group">
        <button className="toolbar-btn" onClick={onCopy} disabled={!hasSelection} title="Copy (Ctrl+C)">
          <Copy size={16} />
        </button>
        <button className="toolbar-btn" onClick={onPaste} disabled={!hasClipboard} title="Paste (Ctrl+V)">
          <Clipboard size={16} />
        </button>
        <button className="toolbar-btn danger" onClick={onDelete} disabled={!hasSelection} title="Delete (Del)">
          <Trash2 size={16} />
        </button>
      </div>

      <div className="toolbar-separator" />

      <div className="toolbar-group">
        <button
          className={`toolbar-btn ${showGrid ? 'active' : ''}`}
          onClick={() => onShowGridChange(!showGrid)}
          title="Toggle Grid"
        >
          <Grid3X3 size={16} />
        </button>
        <select
          className="toolbar-select"
          value={gridSnap}
          onChange={(e) => onGridSnapChange(parseInt(e.target.value))}
          title="Grid Snap Size"
        >
          <option value={0}>No Snap</option>
          <option value={1}>1%</option>
          <option value={2}>2%</option>
          <option value={5}>5%</option>
          <option value={10}>10%</option>
        </select>
      </div>

      <div className="toolbar-separator" />

      {onAlign && (
        <>
          <div className="toolbar-group" title="Align selection to canvas">
            <button className="toolbar-btn" disabled={!hasSelection} onClick={() => onAlign('left')} title="Align Left">
              <AlignStartVertical size={16} />
            </button>
            <button className="toolbar-btn" disabled={!hasSelection} onClick={() => onAlign('hcenter')} title="Center Horizontally">
              <AlignCenterVertical size={16} />
            </button>
            <button className="toolbar-btn" disabled={!hasSelection} onClick={() => onAlign('right')} title="Align Right">
              <AlignEndVertical size={16} />
            </button>
            <button className="toolbar-btn" disabled={!hasSelection} onClick={() => onAlign('top')} title="Align Top">
              <AlignStartHorizontal size={16} />
            </button>
            <button className="toolbar-btn" disabled={!hasSelection} onClick={() => onAlign('vcenter')} title="Center Vertically">
              <AlignCenterHorizontal size={16} />
            </button>
            <button className="toolbar-btn" disabled={!hasSelection} onClick={() => onAlign('bottom')} title="Align Bottom">
              <AlignEndHorizontal size={16} />
            </button>
          </div>
          <div className="toolbar-separator" />
        </>
      )}

      <div className="toolbar-group">
        <button className="toolbar-btn primary" onClick={onSave} title="Save Dashboard (Ctrl+S)">
          <Save size={16} />
          <span>Save</span>
        </button>
        <button className="toolbar-btn" onClick={onExit} title="Exit Designer Mode">
          <X size={16} />
          <span>Exit</span>
        </button>
      </div>
    </div>
  );
}
