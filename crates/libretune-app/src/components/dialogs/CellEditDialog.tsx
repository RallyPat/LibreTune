import React, { useState, useEffect, useRef } from 'react';
import './CellEditDialog.css';

export interface CellEditDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onApply: (value: number) => void;
  currentValue: number;
  cellRow: number;
  cellCol: number;
  xBinValue: number;
  yBinValue: number;
  xAxisName: string;
  yAxisName: string;
  units?: string;
  minValue?: number;
  maxValue?: number;
  decimals?: number;
}

export default function CellEditDialog({
  isOpen,
  onClose,
  onApply,
  currentValue,
  cellRow,
  cellCol,
  xBinValue,
  yBinValue,
  xAxisName,
  yAxisName,
  units = '',
  minValue,
  maxValue,
  decimals = 2,
}: CellEditDialogProps) {
  const [inputValue, setInputValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Reset when dialog opens
  useEffect(() => {
    if (isOpen) {
      setInputValue(currentValue.toFixed(decimals));
      setError(null);
      // Focus input after dialog opens
      setTimeout(() => inputRef.current?.select(), 50);
    }
  }, [isOpen, currentValue, decimals]);

  if (!isOpen) return null;

  const validate = (value: string): string | null => {
    const num = parseFloat(value);
    if (isNaN(num)) {
      return 'Please enter a valid number';
    }
    if (minValue !== undefined && num < minValue) {
      return `Value must be at least ${minValue}`;
    }
    if (maxValue !== undefined && num > maxValue) {
      return `Value must be at most ${maxValue}`;
    }
    return null;
  };

  const handleInputChange = (value: string) => {
    setInputValue(value);
    setError(validate(value));
  };

  const handleApply = () => {
    const validationError = validate(inputValue);
    if (validationError) {
      setError(validationError);
      return;
    }
    const numValue = parseFloat(inputValue);
    onApply(numValue);
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleApply();
    } else if (e.key === 'Escape') {
      onClose();
    }
  };

  const handleIncrement = (amount: number) => {
    const current = parseFloat(inputValue) || 0;
    const newValue = current + amount;
    setInputValue(newValue.toFixed(decimals));
    setError(validate(newValue.toString()));
  };

  return (
    <div className="cell-edit-overlay" onClick={onClose}>
      <div className="cell-edit-dialog glass-card" onClick={e => e.stopPropagation()}>
        <div className="cell-edit-header">
          <h3>Edit Cell Value</h3>
          <span className="cell-location">
            [{cellCol}, {cellRow}]
          </span>
        </div>

        <div className="cell-edit-info">
          <div className="info-row">
            <span className="info-label">{xAxisName}:</span>
            <span className="info-value">{xBinValue}</span>
          </div>
          <div className="info-row">
            <span className="info-label">{yAxisName}:</span>
            <span className="info-value">{yBinValue}</span>
          </div>
        </div>

        <div className="cell-edit-input-section">
          <div className="input-with-buttons">
            <button 
              className="adjust-btn" 
              onClick={() => handleIncrement(-1)}
              title="Decrease by 1"
            >
              −1
            </button>
            <button 
              className="adjust-btn" 
              onClick={() => handleIncrement(-0.1)}
              title="Decrease by 0.1"
            >
              −.1
            </button>
            <input
              ref={inputRef}
              type="number"
              value={inputValue}
              onChange={e => handleInputChange(e.target.value)}
              onKeyDown={handleKeyDown}
              step="any"
              className={`cell-value-input ${error ? 'error' : ''}`}
            />
            <button 
              className="adjust-btn" 
              onClick={() => handleIncrement(0.1)}
              title="Increase by 0.1"
            >
              +.1
            </button>
            <button 
              className="adjust-btn" 
              onClick={() => handleIncrement(1)}
              title="Increase by 1"
            >
              +1
            </button>
          </div>
          {units && <span className="units-label">{units}</span>}
        </div>

        {error && <div className="cell-edit-error">{error}</div>}

        <div className="cell-edit-range">
          {minValue !== undefined && (
            <span>Min: {minValue}</span>
          )}
          {maxValue !== undefined && (
            <span>Max: {maxValue}</span>
          )}
        </div>

        <div className="cell-edit-actions">
          <button className="secondary-btn" onClick={onClose}>
            Cancel
          </button>
          <button 
            className="primary-btn" 
            onClick={handleApply}
            disabled={!!error}
          >
            Apply
          </button>
        </div>
      </div>
    </div>
  );
}
