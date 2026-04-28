/**
 * useDashboardValidation — runs the `validate_dashboard` Tauri command
 * whenever the dash file changes and exposes the report.
 */

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DashFile } from '../dashTypes';
import type { ValidationReport } from '../utils/validation';

export function useDashboardValidation(dashFile: DashFile | null): ValidationReport | null {
  const [report, setReport] = useState<ValidationReport | null>(null);

  useEffect(() => {
    if (!dashFile) {
      setReport(null);
      return;
    }

    invoke<ValidationReport>('validate_dashboard', {
      dashFile,
      projectName: null,
    })
      .then(setReport)
      .catch((err) => {
        console.warn('[useDashboardValidation] Validation failed:', err);
        setReport(null);
      });
  }, [dashFile]);

  return report;
}
