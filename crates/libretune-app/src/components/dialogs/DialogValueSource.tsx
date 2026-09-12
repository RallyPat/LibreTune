import { createContext, useContext } from 'react';
import type { BackendTableData, CurveData } from './types';

export interface DialogValueSource {
  readOnly: boolean;
  changedNames: Set<string>;
  numbers: Record<string, number>;
  strings: Record<string, string>;
  tables: Record<string, BackendTableData>;
  curves: Record<string, CurveData>;
}

const DialogValueSourceContext = createContext<DialogValueSource | null>(null);

export const DialogValueSourceProvider = DialogValueSourceContext.Provider;

export function useDialogValueSource(): DialogValueSource | null {
  return useContext(DialogValueSourceContext);
}
