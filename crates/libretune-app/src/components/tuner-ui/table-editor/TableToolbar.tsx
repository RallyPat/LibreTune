import { Copy, Clipboard, Undo2, Redo2, Flame, Crosshair, Box, Wand2, Upload, Download } from 'lucide-react';
import '../TableEditor.css';

interface TableToolbarProps {
  onSetEqual: () => void;
  onIncrease: () => void;
  onDecrease: () => void;
  onIncreaseMore: () => void;
  onDecreaseMore: () => void;
  onScale: () => void;
  onInterpolate: () => void;
  onSmooth: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onBurn?: () => void;
  hasSelection: boolean;
  hasClipboard: boolean;
  canUndo: boolean;
  canRedo: boolean;
  followMode: boolean;
  onToggleFollowMode: () => void;
  hasOutputChannels: boolean;
  show3D: boolean;
  onToggle3D: () => void;
  /** Generate the whole table from engine specs (VE/ignition/AFR only). */
  onGenerate?: () => void;
  generatableLabel?: string;
  /** TunerStudio-compatible per-table .table file import/export. */
  onImportTable?: () => void;
  onExportTable?: () => void;
}

export default function TableToolbar({
  onSetEqual,
  onIncrease,
  onDecrease,
  onIncreaseMore,
  onDecreaseMore,
  onScale,
  onInterpolate,
  onSmooth,
  onCopy,
  onPaste,
  onUndo,
  onRedo,
  onBurn,
  hasSelection,
  hasClipboard,
  canUndo,
  canRedo,
  followMode,
  onToggleFollowMode,
  hasOutputChannels,
  show3D,
  onToggle3D,
  onGenerate,
  generatableLabel,
  onImportTable,
  onExportTable,
}: TableToolbarProps) {
  return (
    <div className="table-toolbar">
      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onSetEqual}
          disabled={!hasSelection}
          title="Set Equal (=)"
        >
          =
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onDecrease}
          disabled={!hasSelection}
          title="Decrease (&lt;)"
        >
          &lt;
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onIncrease}
          disabled={!hasSelection}
          title="Increase (&gt;)"
        >
          &gt;
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onDecreaseMore}
          disabled={!hasSelection}
          title="Decrease More (-)"
        >
          −
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onIncreaseMore}
          disabled={!hasSelection}
          title="Increase More (+)"
        >
          +
        </button>
      </div>

      <div className="table-toolbar-separator" />

      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onScale}
          disabled={!hasSelection}
          title="Scale (*)"
        >
          ×
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onInterpolate}
          disabled={!hasSelection}
          title="Interpolate (/)"
        >
          /
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onSmooth}
          disabled={!hasSelection}
          title="Smooth (s)"
        >
          s
        </button>
      </div>

      <div className="table-toolbar-separator" />

      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onCopy}
          disabled={!hasSelection}
          title="Copy (Ctrl+C)"
          aria-label="Copy"
        >
          <Copy size={14} />
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onPaste}
          disabled={!hasClipboard}
          title="Paste (Ctrl+V)"
          aria-label="Paste"
        >
          <Clipboard size={14} />
        </button>
      </div>

      <div className="table-toolbar-separator" />

      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onUndo}
          disabled={!canUndo}
          title="Undo (Ctrl+Z)"
          aria-label="Undo"
        >
          <Undo2 size={14} />
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onRedo}
          disabled={!canRedo}
          title="Redo (Ctrl+Y)"
          aria-label="Redo"
        >
          <Redo2 size={14} />
        </button>
      </div>

      {onBurn && (
        <>
          <div className="table-toolbar-separator" />
          <button
            className="table-toolbar-btn table-toolbar-btn-burn"
            onClick={onBurn}
            title="Burn to ECU"
          >
            <Flame size={14} /> Burn
          </button>
        </>
      )}

      <div className="table-toolbar-separator" />
      
      <button
        className={`table-toolbar-btn table-toolbar-btn-follow ${followMode ? 'active' : ''}`}
        onClick={onToggleFollowMode}
        disabled={!hasOutputChannels}
        title={hasOutputChannels ? `Follow Mode (F) - ${followMode ? 'ON' : 'OFF'}` : 'Follow Mode unavailable (no output channels defined)'}
      >
        <Crosshair size={14} /> Follow
      </button>
      
      <button
        className={`table-toolbar-btn table-toolbar-btn-3d ${show3D ? 'active' : ''}`}
        onClick={onToggle3D}
        title={`3D View - ${show3D ? 'ON' : 'OFF'}`}
      >
        <Box size={14} /> 3D
      </button>

      {onGenerate && (
        <>
          <div className="table-toolbar-separator" />
          <button
            className="table-toolbar-btn table-toolbar-btn-generate"
            onClick={onGenerate}
            title={`Generate ${generatableLabel ?? 'table'} from engine specs`}
          >
            <Wand2 size={14} /> Generate
          </button>
        </>
      )}

      {(onImportTable || onExportTable) && (
        <>
          <div className="table-toolbar-separator" />
          {onImportTable && (
            <button
              className="table-toolbar-btn"
              onClick={onImportTable}
              title="Load Table from File... (.table)"
              aria-label="Load Table from File"
            >
              <Upload size={14} />
            </button>
          )}
          {onExportTable && (
            <button
              className="table-toolbar-btn"
              onClick={onExportTable}
              title="Save Table to File... (.table)"
              aria-label="Save Table to File"
            >
              <Download size={14} />
            </button>
          )}
        </>
      )}
    </div>
  );
}

