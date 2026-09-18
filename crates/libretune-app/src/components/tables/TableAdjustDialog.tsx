import { Dialog, Button, FormField } from '../common';
import type { TableAdjustKind } from '../../utils/tableAdjustInput';

const COPY: Record<TableAdjustKind, { title: string; label: string; help: string }> = {
  add: {
    title: 'Increase',
    label: 'Amount to add',
    help: 'Added to every selected cell. Example: 10',
  },
  sub: {
    title: 'Decrease',
    label: 'Amount to subtract',
    help: 'Subtracted from every selected cell. Example: 10',
  },
  mul: {
    title: 'Multiply',
    label: 'Multiplier',
    help: '1.1 = +10%, 0.9 = −10%.',
  },
};

interface Props {
  open: boolean;
  kind: TableAdjustKind;
  raw: string;
  error: string | null;
  cellCount: number;
  onChange: (raw: string) => void;
  onClose: () => void;
  onApply: () => void;
}

export default function TableAdjustDialog({
  open,
  kind,
  raw,
  error,
  cellCount,
  onChange,
  onClose,
  onApply,
}: Props) {
  const copy = COPY[kind];
  return (
    <Dialog open={open} onClose={onClose} size="sm" title={copy.title}>
      <Dialog.Body>
        <FormField
          label={copy.label}
          help={`${copy.help} ${cellCount} cell(s).`}
          error={error}
        >
          {(id) => (
            <input
              id={id}
              type="text"
              inputMode="decimal"
              autoFocus
              value={raw}
              onChange={(e) => onChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  e.stopPropagation();
                  onApply();
                }
              }}
            />
          )}
        </FormField>
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" onClick={onApply}>
          Apply
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
