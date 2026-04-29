/**
 * Shared types used across multiple tuner-ui dialogs.
 */

export interface DialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export interface TuneInfo {
  path: string | null;
  signature: string;
  modified: boolean;
  has_tune: boolean;
}

export interface BuildInfo {
  version: string;
  build_id: string;
}
