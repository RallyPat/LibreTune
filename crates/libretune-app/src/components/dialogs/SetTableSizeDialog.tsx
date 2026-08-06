import { useEffect, useMemo, useState } from 'react';
import { Dialog, Button } from '../common';

export interface TableSizeLimits {
  min_cols: number;
  max_cols: number;
  min_rows: number;
  max_rows: number;
  max_elements: number;
  active_cols: number;
  active_rows: number;
}

interface SetTableSizeDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onApply: (cols: number, rows: number) => void;
  limits: TableSizeLimits;
  isBusy?: boolean;
}

export default function SetTableSizeDialog({
  isOpen,
  onClose,
  onApply,
  limits,
  isBusy = false,
}: SetTableSizeDialogProps) {
  const [cols, setCols] = useState(limits.active_cols);
  const [rows, setRows] = useState(limits.active_rows);

  useEffect(() => {
    if (isOpen) {
      setCols(limits.active_cols);
      setRows(limits.active_rows);
    }
  }, [isOpen, limits.active_cols, limits.active_rows]);

  const cells = cols * rows;
  const error = useMemo(() => {
    if (cols < limits.min_cols || cols > limits.max_cols) {
      return `Columns must be ${limits.min_cols}–${limits.max_cols}`;
    }
    if (rows < limits.min_rows || rows > limits.max_rows) {
      return `Rows must be ${limits.min_rows}–${limits.max_rows}`;
    }
    if (cells > limits.max_elements) {
      return `${rows}×${cols} exceeds cell budget ${limits.max_elements}`;
    }
    return null;
  }, [cols, rows, cells, limits]);

  if (!isOpen) return null;

  return (
    <Dialog open={isOpen} onClose={onClose} title="Set Table Size" size="sm" closeOnBackdrop={!isBusy}>
      <Dialog.Body>
        <p style={{ marginTop: 0 }}>
          Resize this table (and any tables that share its axes). Axes are regenerated between the
          current endpoints and Z values are interpolated. Works offline (saved to the tune); when
          connected, changes are written and burned. Disabled if the ECU signature does not match
          the INI.
        </p>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
          <label>
            Columns (X)
            <input
              type="number"
              min={limits.min_cols}
              max={limits.max_cols}
              value={cols}
              disabled={isBusy}
              onChange={(e) => setCols(Number(e.target.value))}
            />
          </label>
          <label>
            Rows (Y)
            <input
              type="number"
              min={limits.min_rows}
              max={limits.max_rows}
              value={rows}
              disabled={isBusy}
              onChange={(e) => setRows(Number(e.target.value))}
            />
          </label>
        </div>
        <p style={{ opacity: 0.8 }}>
          {rows}×{cols} = {cells} cells (budget {limits.max_elements})
        </p>
        {error && <p style={{ color: 'var(--color-danger, #e55)' }}>{error}</p>}
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="ghost" onClick={onClose} disabled={isBusy}>
          Cancel
        </Button>
        <Button
          onClick={() => onApply(cols, rows)}
          disabled={!!error || isBusy}
        >
          {isBusy ? 'Resizing…' : 'Apply'}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
