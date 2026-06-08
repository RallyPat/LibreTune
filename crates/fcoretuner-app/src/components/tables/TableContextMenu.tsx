import { useState, useRef, useEffect } from 'react';

interface TableContextProps {
  x: number;
  y: number;
  cellValue: number;
  position: { top: number; left: number };
  visible: boolean;
  onClose: () => void;
  onSetEqual: (value: number) => void;
  onScale: (factor: number) => void;
  onInterpolate: () => void;
  onSmooth: () => void;
  onLock: () => void;
  onUnlock: () => void;
  isLocked: boolean;
}

export default function TableContextMenu({
  x,
  y,
  cellValue,
  position,
  visible,
  onClose,
  onSetEqual,
  onScale,
  onInterpolate,
  onSmooth,
  onLock,
  onUnlock,
  isLocked
}: TableContextProps) {
  const [scaleFactor, setScaleFactor] = useState('1.5');
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  const handleSetEqual = () => {
    onSetEqual(cellValue);
    onClose();
  };

  const handleScale = () => {
    const factor = parseFloat(scaleFactor);
    onScale(factor);
    onClose();
  };

  if (!visible) return null;

  return (
    <div
      ref={menuRef}
      className="table-context-menu"
      style={{
        top: `${position.top}px`,
        left: `${position.left}px`
      }}
    >
      <div className="context-menu-header">
        <span>单元格 [{x}, {y}]</span>
        <span>值: {cellValue.toFixed(1)}</span>
      </div>

      <div className="context-menu-item" onClick={handleSetEqual}>
        <span>Set Equal to Selected</span>
        <span className="context-shortcut">=</span>
      </div>

      <div className="context-menu-separator" />

      <div className="context-menu-item" onClick={onInterpolate}>
        <span>插值单元格</span>
        <span className="context-shortcut">I</span>
      </div>

      <div className="context-menu-item" onClick={onSmooth}>
        <span>平滑单元格</span>
        <span className="context-shortcut">S</span>
      </div>

      <div className="context-menu-separator" />

      <div className="context-menu-item" onClick={handleScale}>
        <span>缩放单元格...</span>
        <span className="context-shortcut">*</span>
        <div className="scale-input-container">
          <input
            type="number"
            step="0.1"
            min="0.1"
            max="10"
            value={scaleFactor}
            onClick={e => e.stopPropagation()}
            onChange={e => setScaleFactor(e.target.value)}
          />
          <button 
            className="scale-apply-btn"
            onClick={e => { e.stopPropagation(); handleScale(); }}
          >
            Apply
          </button>
        </div>
      </div>

      <div className="context-menu-separator" />

      <div 
        className={`context-menu-item ${isLocked ? 'danger' : ''}`} 
        onClick={isLocked ? onUnlock : onLock}
      >
        {isLocked ? (
          <>
            <span>解锁单元格</span>
            <span className="context-shortcut">U</span>
          </>
        ) : (
          <>
            <span>锁定单元格</span>
            <span className="context-shortcut">L</span>
          </>
        )}
      </div>
    </div>
  );
}
