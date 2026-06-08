import { useState, useEffect } from 'react';
import { Plus, Minus, RotateCcw } from 'lucide-react';
import './RebinDialog.css';

export interface RebinDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onApply: (newXBins: number[], newYBins: number[], interpolate: boolean) => void;
  currentXBins: number[];
  currentYBins: number[];
  xAxisName: string;
  yAxisName: string;
}

export default function RebinDialog({
  isOpen,
  onClose,
  onApply,
  currentXBins,
  currentYBins,
  xAxisName,
  yAxisName,
}: RebinDialogProps) {
  const [newXBins, setNewXBins] = useState<number[]>([]);
  const [newYBins, setNewYBins] = useState<number[]>([]);
  const [interpolateZ, setInterpolateZ] = useState(true);

  // Reset bins when dialog opens
  useEffect(() => {
    if (isOpen) {
      setNewXBins([...currentXBins]);
      setNewYBins([...currentYBins]);
      setInterpolateZ(true);
    }
  }, [isOpen, currentXBins, currentYBins]);

  if (!isOpen) return null;

  const handleXBinChange = (index: number, value: string) => {
    const numValue = parseFloat(value);
    if (!isNaN(numValue)) {
      const updated = [...newXBins];
      updated[index] = numValue;
      setNewXBins(updated);
    }
  };

  const handleYBinChange = (index: number, value: string) => {
    const numValue = parseFloat(value);
    if (!isNaN(numValue)) {
      const updated = [...newYBins];
      updated[index] = numValue;
      setNewYBins(updated);
    }
  };

  const addXBin = () => {
    const lastValue = newXBins[newXBins.length - 1] || 0;
    const step = newXBins.length > 1 ? newXBins[newXBins.length - 1] - newXBins[newXBins.length - 2] : 100;
    setNewXBins([...newXBins, lastValue + step]);
  };

  const removeXBin = () => {
    if (newXBins.length > 1) {
      setNewXBins(newXBins.slice(0, -1));
    }
  };

  const addYBin = () => {
    const lastValue = newYBins[newYBins.length - 1] || 0;
    const step = newYBins.length > 1 ? newYBins[newYBins.length - 1] - newYBins[newYBins.length - 2] : 10;
    setNewYBins([...newYBins, lastValue + step]);
  };

  const removeYBin = () => {
    if (newYBins.length > 1) {
      setNewYBins(newYBins.slice(0, -1));
    }
  };

  const resetToOriginal = () => {
    setNewXBins([...currentXBins]);
    setNewYBins([...currentYBins]);
  };

  const generateLinearBins = (count: number, min: number, max: number): number[] => {
    const bins: number[] = [];
    const step = (max - min) / (count - 1);
    for (let i = 0; i < count; i++) {
      bins.push(Math.round((min + step * i) * 100) / 100);
    }
    return bins;
  };

  const handleGenerateX = () => {
    const min = Math.min(...currentXBins);
    const max = Math.max(...currentXBins);
    setNewXBins(generateLinearBins(newXBins.length, min, max));
  };

  const handleGenerateY = () => {
    const min = Math.min(...currentYBins);
    const max = Math.max(...currentYBins);
    setNewYBins(generateLinearBins(newYBins.length, min, max));
  };

  const handleApply = () => {
    // Sort bins before applying
    const sortedX = [...newXBins].sort((a, b) => a - b);
    const sortedY = [...newYBins].sort((a, b) => a - b);
    onApply(sortedX, sortedY, interpolateZ);
    onClose();
  };

  return (
    <div className="rebin-dialog-overlay" onClick={onClose}>
      <div className="rebin-dialog glass-card" onClick={e => e.stopPropagation()}>
        <div className="rebin-dialog-header">
          <h2>重新分箱表格</h2>
          <button className="reset-btn" onClick={resetToOriginal} title="重置为原始分箱">
            <RotateCcw size={16} />
          </button>
        </div>

        <div className="rebin-dialog-content">
          {/* X Axis Section */}
          <div className="rebin-section">
            <div className="rebin-section-header">
              <h3>{xAxisName} 分箱 ({newXBins.length})</h3>
              <div className="rebin-section-actions">
                <button className="icon-btn" onClick={handleGenerateX} title="生成线性间距">
                  线性
                </button>
                <button className="icon-btn" onClick={removeXBin} disabled={newXBins.length <= 1}>
                  <Minus size={14} />
                </button>
                <button className="icon-btn" onClick={addXBin}>
                  <Plus size={14} />
                </button>
              </div>
            </div>
            <div className="rebin-bins-grid">
              {newXBins.map((val, i) => (
                <input
                  key={`x-${i}`}
                  type="number"
                  value={val}
                  step="any"
                  onChange={e => handleXBinChange(i, e.target.value)}
                  className="bin-input"
                />
              ))}
            </div>
          </div>

          {/* Y Axis Section */}
          <div className="rebin-section">
            <div className="rebin-section-header">
              <h3>{yAxisName} 分箱 ({newYBins.length})</h3>
              <div className="rebin-section-actions">
                <button className="icon-btn" onClick={handleGenerateY} title="生成线性间距">
                  线性
                </button>
                <button className="icon-btn" onClick={removeYBin} disabled={newYBins.length <= 1}>
                  <Minus size={14} />
                </button>
                <button className="icon-btn" onClick={addYBin}>
                  <Plus size={14} />
                </button>
              </div>
            </div>
            <div className="rebin-bins-grid">
              {newYBins.map((val, i) => (
                <input
                  key={`y-${i}`}
                  type="number"
                  value={val}
                  step="any"
                  onChange={e => handleYBinChange(i, e.target.value)}
                  className="bin-input"
                />
              ))}
            </div>
          </div>

          {/* Options */}
          <div className="rebin-options">
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={interpolateZ}
                onChange={e => setInterpolateZ(e.target.checked)}
              />
              插值 Z 值（推荐）
            </label>
            <p className="option-hint">
              启用后，现有值将双线性插值到新的分箱位置。
              禁用时，新单元格将初始化为零。
            </p>
          </div>

          {/* Preview Info */}
          <div className="rebin-preview">
            <div className="preview-item">
              <span className="preview-label">原始尺寸:</span>
              <span className="preview-value">{currentXBins.length} × {currentYBins.length}</span>
            </div>
            <div className="preview-item">
              <span className="preview-label">新尺寸:</span>
              <span className="preview-value">{newXBins.length} × {newYBins.length}</span>
            </div>
          </div>
        </div>

        <div className="rebin-dialog-actions">
          <button className="secondary-btn" onClick={onClose}>
            取消
          </button>
          <button className="primary-btn" onClick={handleApply}>
            应用重新分箱
          </button>
        </div>
      </div>
    </div>
  );
}
