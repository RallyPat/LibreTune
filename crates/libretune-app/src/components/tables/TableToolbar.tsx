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
  Palette
} from 'lucide-react';

interface TableToolbarProps {
  onSetEqual: () => void;
  onIncrease: (amount: number) => void;
  onDecrease: (amount: number) => void;
  onScale: () => void;
  onInterpolate: () => void;
  onSmooth: () => void;
  onRebin: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onUndo: () => void;
  onRedo?: () => void;
  canUndo: boolean;
  canRedo?: boolean;
  canPaste: boolean;
  followMode?: boolean;
  onFollowModeToggle?: () => void;
  showColorShade?: boolean;
  onColorShadeToggle?: () => void;
}

export default function TableToolbar({ 
  onSetEqual, 
  onIncrease, 
  onDecrease, 
  onScale, 
  onInterpolate, 
  onSmooth, 
  onRebin,
  onCopy,
  onPaste,
  onUndo,
  onRedo,
  canUndo,
  canRedo,
  canPaste,
  followMode = false,
  onFollowModeToggle,
  showColorShade = false,
  onColorShadeToggle,
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
          title="Increase by 1 (> or .)"
          onClick={() => onIncrease(1)}
        >
          <Plus size={14} />
          <span className="ts-toolbar-key">&gt;</span>
        </button>
        <button 
          className="ts-toolbar-btn" 
          title="Decrease by 1 (<)"
          onClick={() => onDecrease(1)}
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
      </div>
    </div>
  );
}
