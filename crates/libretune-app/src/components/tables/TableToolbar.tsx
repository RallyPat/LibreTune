import {
  Plus,
  Minus,
  Equal,
  X,
  Undo2,
  Redo2,
  Copy,
  ClipboardPaste,
  Sparkles,
  TrendingUp,
  Grid3X3,
  Crosshair,
  Palette,
  Box,
  Scaling,
  Wand2,
  Upload,
  Download,
  Bot,
  ArrowUpDown
} from 'lucide-react';

interface TableToolbarProps {
  onSetEqual: () => void;
  onIncrease: (amount: number) => void;
  onDecrease: (amount: number) => void;
  onScale: () => void;
  onInterpolate: () => void;
  onSmooth: () => void;
  onRebin: () => void;
  onSetSize?: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onUndo: () => void;
  onRedo?: () => void;
  canUndo: boolean;
  canRedo?: boolean;
  canPaste: boolean;
  followMode?: boolean;
  onFollowModeToggle?: () => void;
  /** Y axis zero at bottom (rows reversed). Toggles the global table_y_axis_bottom setting. */
  yAxisBottom?: boolean;
  onYAxisBottomToggle?: () => void;
  showColorShade?: boolean;
  onColorShadeToggle?: () => void;
  show3D?: boolean;
  onToggle3D?: () => void;
  /** Generate the whole table from engine specs (VE/ignition/AFR only). */
  onGenerate?: () => void;
  generatableLabel?: string;
  /** TunerStudio-compatible per-table .table file import/export. */
  onImportTable?: () => void;
  onExportTable?: () => void;
  /** Open the AI assistant with this table's context pre-loaded. */
  onAskAi?: () => void;
}

export default function TableToolbar({ 
  onSetEqual, 
  onIncrease, 
  onDecrease, 
  onScale, 
  onInterpolate, 
  onSmooth, 
  onRebin,
  onSetSize,
  onCopy,
  onPaste,
  onUndo,
  onRedo,
  canUndo,
  canRedo,
  canPaste,
  followMode = false,
  onFollowModeToggle,
  yAxisBottom = false,
  onYAxisBottomToggle,
  showColorShade = false,
  onColorShadeToggle,
  show3D = false,
  onToggle3D,
  onGenerate,
  generatableLabel,
  onImportTable,
  onExportTable,
  onAskAi,
}: TableToolbarProps) {
  return (
    <div className="ts-toolbar">
      {/* Cell Operations */}
      <div className="ts-toolbar-group">
        <button 
          className="ts-toolbar-btn" 
          title="Set Equal (=) - Set selected cells to average"
          onClick={onSetEqual}
        >
          <Equal size={14} />
          <span className="ts-toolbar-key">=</span>
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Increase by 1% (> or .)"
          onClick={() => onIncrease(0.01)}
        >
          <Plus size={14} />
          <span className="ts-toolbar-key">&gt;</span>
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Decrease by 1% (<)"
          onClick={() => onDecrease(0.01)}
        >
          <Minus size={14} />
          <span className="ts-toolbar-key">&lt;</span>
        </button>
      </div>

      <div className="ts-toolbar-divider" />

      {/* Bulk Operations */}
      <div className="ts-toolbar-group">
        <button 
          className="ts-toolbar-btn" 
          title="Scale selected cells (*)"
          onClick={onScale}
        >
          <X size={14} />
          <span className="ts-toolbar-key">*</span>
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Interpolate between corners (/)"
          onClick={onInterpolate}
        >
          <TrendingUp size={14} />
          <span className="ts-toolbar-key">/</span>
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Smooth selected cells (s)"
          onClick={onSmooth}
        >
          <Sparkles size={14} />
          <span className="ts-toolbar-key">s</span>
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Re-bin table - change axis values"
          onClick={onRebin}
        >
          <Grid3X3 size={14} />
        </button>
        {onSetSize && (
          <button
            className="ts-toolbar-btn"
            title="Set table size (resizable VE / ignition / lambda)"
            onClick={onSetSize}
          >
            <Scaling size={14} />
          </button>
        )}
        {onGenerate && (
          <button
            className="ts-toolbar-btn"
            title={`Generate ${generatableLabel ?? 'table'} from engine specs`}
            onClick={onGenerate}
          >
            <Wand2 size={14} />
            <span className="ts-toolbar-label">Generate</span>
          </button>
        )}
      </div>

      <div className="ts-toolbar-divider" />

      {/* Clipboard */}
      <div className="ts-toolbar-group">
        <button 
          className="ts-toolbar-btn" 
          title="Copy selected (Ctrl+C)"
          onClick={onCopy}
        >
          <Copy size={14} />
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Paste (Ctrl+V)"
          onClick={onPaste}
          disabled={!canPaste}
        >
          <ClipboardPaste size={14} />
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Undo (Ctrl+Z)"
          onClick={onUndo}
          disabled={!canUndo}
        >
          <Undo2 size={14} />
        </button>
        {onRedo && (
          <button 
            className="ts-toolbar-btn" 
            title="Redo (Ctrl+Y)"
            onClick={onRedo}
            disabled={!canRedo}
          >
            <Redo2 size={14} />
          </button>
        )}
      </div>

      <div className="ts-toolbar-divider" />

      {/* View Controls */}
      <div className="ts-toolbar-group">
        {onFollowModeToggle && (
          <button 
            className={`ts-toolbar-btn ${followMode ? 'ts-toolbar-btn-active' : ''}`}
            title="Follow Mode (F) - Track live ECU position"
            onClick={onFollowModeToggle}
          >
            <Crosshair size={14} />
            <span className="ts-toolbar-key">F</span>
          </button>
        )}
        {onColorShadeToggle && (
          <button 
            className={`ts-toolbar-btn ${showColorShade ? 'ts-toolbar-btn-active' : ''}`}
            title="Color Shading - Show value heat map"
            onClick={onColorShadeToggle}
          >
            <Palette size={14} />
          </button>
        )}
        {onYAxisBottomToggle && (
          <button
            className={`ts-toolbar-btn ${yAxisBottom ? 'ts-toolbar-btn-active' : ''}`}
            title={yAxisBottom ? 'Y axis: zero at bottom (click for zero at top)' : 'Y axis: zero at top (click for zero at bottom)'}
            onClick={onYAxisBottomToggle}
            aria-pressed={yAxisBottom}
            aria-label="Y axis zero at bottom"
          >
            <ArrowUpDown size={14} />
          </button>
        )}
        {onToggle3D && (
          <button
            className={`ts-toolbar-btn ${show3D ? 'ts-toolbar-btn-active' : ''}`}
            title="Toggle 3D View"
            onClick={onToggle3D}
          >
            <Box size={14} />
          </button>
        )}
      </div>

      {(onImportTable || onExportTable) && (
        <>
          <div className="ts-toolbar-divider" />
          <div className="ts-toolbar-group">
            {onImportTable && (
              <button
                className="ts-toolbar-btn"
                title="Load Table from File... (.table)"
                onClick={onImportTable}
              >
                <Upload size={14} />
              </button>
            )}
            {onExportTable && (
              <button
                className="ts-toolbar-btn"
                title="Save Table to File... (.table)"
                onClick={onExportTable}
              >
                <Download size={14} />
              </button>
            )}
          </div>
        </>
      )}
      {onAskAi && (
        <>
          <div className="ts-toolbar-divider" />
          <div className="ts-toolbar-group">
            <button
              className="ts-toolbar-btn"
              title="Ask the AI assistant about this table"
              onClick={onAskAi}
            >
              <Bot size={14} />
            </button>
          </div>
        </>
      )}
    </div>
  );
}
