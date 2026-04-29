/**
 * Barrel re-export of tuner-ui dialogs.
 *
 * Each dialog has been split into its own file under `./dialogs/`. This
 * file preserves the historical import path `'./Dialogs'` so existing
 * call sites and tests do not need to change.
 */
export { SaveDialog } from './dialogs/SaveDialog';
export { LoadDialog } from './dialogs/LoadDialog';
export { BurnDialog } from './dialogs/BurnDialog';
export { NewTuneDialog } from './dialogs/NewTuneDialog';
export { AboutDialog } from './dialogs/AboutDialog';
export { SettingsDialog } from './dialogs/SettingsDialog';
export { ConnectionDialog } from './dialogs/ConnectionDialog';
export type { DialogProps, TuneInfo, BuildInfo } from './dialogs/types';
