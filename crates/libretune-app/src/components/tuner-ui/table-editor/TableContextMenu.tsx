import { Check } from 'lucide-react';
import '../TableEditor.css';

interface TableContextMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  hasSelection: boolean;
  hasClipboard: boolean;
  onResetToOriginal: () => void;
  onSetValue: () => void;
  onStepUp: () => void;
  onStepDown: () => void;
  onAddAmount: () => void;
  onSubtractAmount: () => void;
  onMultiplyBy: () => void;
  onInterpolate: () => void;
  onInterpolateHorizontal: () => void;
  onInterpolateVertical: () => void;
  onSmooth: () => void;
  onFloodFill: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onSetStepAmount: () => void;
  onSetStepCount: () => void;
  onSetStepPercent: () => void;
  onToggleHeatmap: () => void;
  heatmapEnabled: boolean;
}

export default function TableContextMenu({
  x,
  y,
  onClose: _onClose,
  hasSelection,
  hasClipboard,
  onResetToOriginal,
  onSetValue,
  onStepUp,
  onStepDown,
  onAddAmount,
  onSubtractAmount,
  onMultiplyBy,
  onInterpolate,
  onInterpolateHorizontal,
  onInterpolateVertical,
  onSmooth,
  onFloodFill,
  onCopy,
  onPaste,
  onSetStepAmount,
  onSetStepCount,
  onSetStepPercent,
  onToggleHeatmap,
  heatmapEnabled,
}: TableContextMenuProps) {
  return (
    <div 
      className="table-context-menu" 
      style={{ left: x, top: y }}
      onClick={(e) => e.stopPropagation()}
    >
      <button className="context-menu-item" onClick={onResetToOriginal} disabled={!hasSelection}>
        Reset to Original <span className="shortcut">Esc</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onSetValue} disabled={!hasSelection}>
        Set Value <span className="shortcut">=</span>
      </button>
      <button className="context-menu-item" onClick={onStepUp} disabled={!hasSelection}>
        Step Up <span className="shortcut">&gt; or ,</span>
      </button>
      <button className="context-menu-item" onClick={onStepDown} disabled={!hasSelection}>
        Step Down <span className="shortcut">&lt; or .</span>
      </button>
      <button className="context-menu-item" onClick={onAddAmount} disabled={!hasSelection}>
        Add Amount <span className="shortcut">+</span>
      </button>
      <button className="context-menu-item" onClick={onSubtractAmount} disabled={!hasSelection}>
        Subtract Amount <span className="shortcut">-</span>
      </button>
      <button className="context-menu-item" onClick={onMultiplyBy} disabled={!hasSelection}>
        Multiply By <span className="shortcut">*</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onInterpolate} disabled={!hasSelection}>
        Auto-Fill <span className="shortcut">/</span>
      </button>
      <button className="context-menu-item" onClick={onInterpolateHorizontal} disabled={!hasSelection}>
        Fill Horizontal <span className="shortcut">H</span>
      </button>
      <button className="context-menu-item" onClick={onInterpolateVertical} disabled={!hasSelection}>
        Fill Vertical <span className="shortcut">V</span>
      </button>
      <button className="context-menu-item" onClick={onSmooth} disabled={!hasSelection}>
        Blend Selection <span className="shortcut">S</span>
      </button>
      <button className="context-menu-item" onClick={onFloodFill} disabled={!hasSelection}>
        Flood Fill <span className="shortcut">F</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onCopy} disabled={!hasSelection}>
        Copy <span className="shortcut">Ctrl+C</span>
      </button>
      <button className="context-menu-item" onClick={onPaste} disabled={!hasClipboard}>
        Paste <span className="shortcut">Ctrl+V</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onSetStepAmount}>
        Set Step Amount...
      </button>
      <button className="context-menu-item" onClick={onSetStepCount}>
        Set Step Multiplier (Ctrl)...
      </button>
      <button className="context-menu-item" onClick={onSetStepPercent}>
        Set Step Percent (Shift)...
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onToggleHeatmap}>
        {heatmapEnabled && <Check size={12} style={{ marginRight: 4 }} />}Cell Color By Value
      </button>
    </div>
  );
}
