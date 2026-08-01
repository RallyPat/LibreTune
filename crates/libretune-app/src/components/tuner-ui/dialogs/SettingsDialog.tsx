import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { HeatmapScheme, getAvailableSchemes } from '../../../utils/heatmapColors';
import { useUnitPreferences } from '../../../contexts/useUnitPreferences';
import { TemperatureUnit, PressureUnit, AfrUnit, SpeedUnit, FuelType, STOICH_AFR } from '../../../utils/unitConversions';
import { createFocusTrap, focusFirstElement } from '../../../utils/focusManagement';
import HotkeyEditor from '../../dialogs/HotkeyEditor';
import ThemePicker from '../../dialogs/ThemePicker';
import StatusBarChannelSelector from '../../dialogs/StatusBarChannelSelector';
import { Dialog, Button, FormField, RiskAcknowledgement } from '../../common';
import { ThemeName } from '../../../themes';
import { SUPPORTED_LANGUAGES, LANGUAGE_STORAGE_KEY, type SupportedLanguageCode } from '../../../i18n/languages';
import ConnectionMetrics from '../../layout/ConnectionMetrics';
import '../Dialogs.css';

interface DialogProps {
  isOpen: boolean;
  onClose: () => void;
}

interface CurrentProject {
  name: string;
  path: string;
  signature: string;
  has_tune: boolean;
  tune_modified: boolean;
  connection: {
    port: string | null;
    baud_rate: number;
    auto_connect: boolean;
  };
}

interface SettingsDialogProps extends DialogProps {
  theme: string;
  onThemeChange: (theme: string) => void;
  onSettingsChange?: (settings: { units?: string; autoBurnOnClose?: boolean; demoMode?: boolean; indicatorColumnCount?: string; indicatorFillEmpty?: boolean; indicatorTextFit?: string; statusBarChannels?: string[]; runtimePacketMode?: string; autoSyncGaugeRanges?: boolean }) => void;
  currentProject?: CurrentProject | null;
  /** Whether ECU-derived (INI) tuning menus are shown in the top menu bar. */
  showEcuMenusInMenubar?: boolean;
  /** Called when the user toggles the ECU-menus-in-menubar setting. */
  onEcuMenusInMenubarChange?: (visible: boolean) => void;
}

export function SettingsDialog({ isOpen, onClose, theme, onThemeChange, onSettingsChange, currentProject, showEcuMenusInMenubar, onEcuMenusInMenubarChange }: SettingsDialogProps) {
  const [localTheme, setLocalTheme] = useState(theme);
  const [localLanguage, setLocalLanguage] = useState<SupportedLanguageCode>(() => {
    try {
      const stored = localStorage.getItem(LANGUAGE_STORAGE_KEY) as SupportedLanguageCode | null;
      if (stored && SUPPORTED_LANGUAGES.some(l => l.code === stored)) return stored;
    } catch { /* ignore */ }
    return 'en';
  });
  const [localUnits, setLocalUnits] = useState('metric');
  const [autoBurnOnClose, setAutoBurnOnClose] = useState(false);
  // Save feedback: 'idle' (nothing shown), 'saving', 'saved', 'error'.
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  const [saveError, setSaveError] = useState<string | null>(null);
  const [demoMode, setDemoMode] = useState(false);
  const [tableYAxisBottom, setTableYAxisBottom] = useState(false);
  // Local mirror of the ECU-menus-in-menubar setting. Seeded from the prop
  // (authoritative app state) and from get_settings on open; writes go to
  // both update_setting (persist) and the parent callback (live re-render).
  const [localShowEcuMenusInMenubar, setLocalShowEcuMenusInMenubar] = useState(true);
  const [tableCursorColor, setTableCursorColor] = useState('');
  const [tableTrailColor, setTableTrailColor] = useState('');
  const [tableTrailFadeSec, setTableTrailFadeSec] = useState(8);
  const [demoLoading, setDemoLoading] = useState(false);
  const [indicatorColumnCount, setIndicatorColumnCount] = useState('auto');
  const [indicatorFillEmpty, setIndicatorFillEmpty] = useState(false);
  const [indicatorTextFit, setIndicatorTextFit] = useState('scale');
  const [currentIniPath, setCurrentIniPath] = useState<string | null>(null);
  const [switchingIni, setSwitchingIni] = useState(false);
  
  // Status bar channel configuration
  const [statusBarChannels, setStatusBarChannels] = useState<string[]>([]);
  const [availableChannels, setAvailableChannels] = useState<string[]>([]);
  
  // Heatmap settings
  const [heatmapValueScheme, setHeatmapValueScheme] = useState<HeatmapScheme>('tunerstudio');
  const [heatmapChangeScheme, setHeatmapChangeScheme] = useState<HeatmapScheme>('tunerstudio');
  const [heatmapCoverageScheme, setHeatmapCoverageScheme] = useState<HeatmapScheme>('tunerstudio');
  
  // Gauge/Dashboard settings
  const [gaugeSnapToGrid, setGaugeSnapToGrid] = useState(true);
  const [gaugeFreeMove, setGaugeFreeMove] = useState(false);
  const [gaugeLock, setGaugeLock] = useState(false);
  const [autoSyncGaugeRanges, setAutoSyncGaugeRanges] = useState(true);
  
  // Version control settings
  const [autoCommitOnSave, setAutoCommitOnSave] = useState('never');
  const [commitMessageFormat, setCommitMessageFormat] = useState('Tune saved on {date} at {time}');
  const [runtimePacketMode, setRuntimePacketMode] = useState<'Auto'|'ForceBurst'|'ForceOCH'|'Disabled'>('Auto');

  // AI assistant settings (bring-your-own LLM). All gated on the risk ack.
  const [aiEnabled, setAiEnabled] = useState(false);
  const [aiRiskAcked, setAiRiskAcked] = useState(false);
  const [aiProvider, setAiProvider] = useState('openai');
  const [aiBaseUrl, setAiBaseUrl] = useState('');
  const [aiApiKey, setAiApiKey] = useState('');
  const [aiModel, setAiModel] = useState('');
  const [aiCapabilityTier, setAiCapabilityTier] = useState<'read' | 'tune' | 'config'>('read');
  // Auto-reconnect setting: whether to automatically sync & reconnect after controller commands
  const [autoReconnectAfterControllerCommand, setAutoReconnectAfterControllerCommand] = useState<boolean>(true);
  const [autoReconnectAfterFirmware, setAutoReconnectAfterFirmware] = useState<boolean>(true);
  
  // Auto-record settings for data logging
  const [autoRecordEnabled, setAutoRecordEnabled] = useState(false);
  const [keyOnThresholdRpm, setKeyOnThresholdRpm] = useState(100);
  const [keyOffTimeoutSec, setKeyOffTimeoutSec] = useState(2);

  // Alert rules settings
  const [alertLargeChangeEnabled, setAlertLargeChangeEnabled] = useState(true);
  const [alertLargeChangeAbs, setAlertLargeChangeAbs] = useState(5);
  const [alertLargeChangePercent, setAlertLargeChangePercent] = useState(10);
  
  // Project-specific settings
  const [autoConnect, setAutoConnect] = useState(false);
  
  // Settings dialog tabs
  const [currentTab, setCurrentTab] = useState<'general' | 'appearance' | 'definitions' | 'hotkeys'>('general');
  // Settings search: filters sections across ALL tabs as you type, mirroring
  // the sidebar search UX. Empty query = normal tabbed view.
  const [settingsQuery, setSettingsQuery] = useState('');
  const searchInputRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  // ECU Definitions tab state
  const [iniList, setIniList] = useState<{id: string; name: string; signature: string; path: string; imported: boolean; source: string}[]>([]);
  const [iniLoading, setIniLoading] = useState(false);
  const [deletingIni, setDeletingIni] = useState<string | null>(null);
  
  // Hotkey bindings
  const [hotkeyBindings, setHotkeyBindings] = useState<Record<string, string>>({});
  const [hotkeysLoading, setHotkeysLoading] = useState(false);
  
  // Unit preferences from context
  const unitPrefs = useUnitPreferences();
  
  // Available heatmap schemes
  const availableSchemes = getAvailableSchemes();

  useEffect(() => {
    setLocalTheme(theme);
    // Load settings from backend
    if (isOpen) {
      setSaveStatus('idle');
      setSaveError(null);
      invoke('get_settings').then((settings: any) => {
        if (settings.units_system !== undefined) setLocalUnits(settings.units_system);
        if (settings.language) {
          const lang = settings.language as SupportedLanguageCode;
          if (SUPPORTED_LANGUAGES.some(l => l.code === lang)) {
            setLocalLanguage(lang);
          }
        }
        if (settings.auto_burn_on_close !== undefined) setAutoBurnOnClose(!!settings.auto_burn_on_close);
        if (settings.table_y_axis_bottom !== undefined) setTableYAxisBottom(!!settings.table_y_axis_bottom);
        if (settings.show_ecu_menus_in_menubar !== undefined) setLocalShowEcuMenusInMenubar(!!settings.show_ecu_menus_in_menubar);
        if (settings.table_cursor_color) setTableCursorColor(settings.table_cursor_color);
        if (settings.table_trail_color) setTableTrailColor(settings.table_trail_color);
        if (settings.table_trail_fade_sec !== undefined) setTableTrailFadeSec(settings.table_trail_fade_sec);
        if (settings.indicator_column_count !== undefined) setIndicatorColumnCount(settings.indicator_column_count);
        if (settings.indicator_fill_empty !== undefined) setIndicatorFillEmpty(!!settings.indicator_fill_empty);
        if (settings.indicator_text_fit !== undefined) setIndicatorTextFit(settings.indicator_text_fit);
        if (settings.last_ini_path !== undefined) setCurrentIniPath(settings.last_ini_path);
        // Status bar channels
        if (settings.status_bar_channels !== undefined) setStatusBarChannels(settings.status_bar_channels);
        // Heatmap settings
        if (settings.heatmap_value_scheme !== undefined) setHeatmapValueScheme(settings.heatmap_value_scheme);
        if (settings.heatmap_change_scheme !== undefined) setHeatmapChangeScheme(settings.heatmap_change_scheme);
        if (settings.heatmap_coverage_scheme !== undefined) setHeatmapCoverageScheme(settings.heatmap_coverage_scheme);
        // Gauge settings
        if (settings.gauge_snap_to_grid !== undefined) setGaugeSnapToGrid(!!settings.gauge_snap_to_grid);
        if (settings.gauge_free_move !== undefined) setGaugeFreeMove(!!settings.gauge_free_move);
        if (settings.gauge_lock !== undefined) setGaugeLock(!!settings.gauge_lock);
        if (settings.auto_sync_gauge_ranges !== undefined) setAutoSyncGaugeRanges(!!settings.auto_sync_gauge_ranges);
        // Version control settings
        if (settings.auto_commit_on_save !== undefined) setAutoCommitOnSave(settings.auto_commit_on_save);
        if (settings.commit_message_format !== undefined) setCommitMessageFormat(settings.commit_message_format);
        if (settings.runtime_packet_mode !== undefined) setRuntimePacketMode(settings.runtime_packet_mode);
        // AI assistant settings
        if (settings.ai_assistant_enabled !== undefined) setAiEnabled(settings.ai_assistant_enabled);
        if (settings.ai_risk_acknowledged !== undefined) setAiRiskAcked(settings.ai_risk_acknowledged);
        if (settings.ai_provider !== undefined) {
          // Coerce empty/blank provider to the default so the dropdown always
          // shows a valid selection (older settings files could store "").
          const p = settings.ai_provider as string;
          setAiProvider(p && p.trim() ? p : 'openai');
        }
        if (settings.ai_base_url !== undefined) setAiBaseUrl(settings.ai_base_url);
        if (settings.ai_api_key !== undefined) setAiApiKey(settings.ai_api_key);
        if (settings.ai_model !== undefined) setAiModel(settings.ai_model);
        if (settings.ai_capability_tier !== undefined) setAiCapabilityTier(settings.ai_capability_tier);
        if (settings.auto_reconnect_after_controller_command !== undefined) setAutoReconnectAfterControllerCommand(!!settings.auto_reconnect_after_controller_command);
        if (settings.auto_reconnect_after_firmware !== undefined) setAutoReconnectAfterFirmware(!!settings.auto_reconnect_after_firmware);
        // Auto-record settings
        if (settings.auto_record_enabled !== undefined) setAutoRecordEnabled(!!settings.auto_record_enabled);
        if (settings.key_on_threshold_rpm !== undefined) setKeyOnThresholdRpm(settings.key_on_threshold_rpm);
        if (settings.key_off_timeout_sec !== undefined) setKeyOffTimeoutSec(settings.key_off_timeout_sec);
        // Alert rules settings
        if (settings.alert_large_change_enabled !== undefined) setAlertLargeChangeEnabled(!!settings.alert_large_change_enabled);
        if (settings.alert_large_change_abs !== undefined) setAlertLargeChangeAbs(settings.alert_large_change_abs);
        if (settings.alert_large_change_percent !== undefined) setAlertLargeChangePercent(settings.alert_large_change_percent);
      }).catch(console.error);

      // Load hotkey bindings
      setHotkeysLoading(true);
      invoke<Record<string, string>>('get_hotkey_bindings')
        .then(setHotkeyBindings)
        .catch(console.error)
        .finally(() => setHotkeysLoading(false));

      // Load project-specific settings
      if (currentProject) {
        setAutoConnect(currentProject.connection.auto_connect);
      }

      // Load available output channels from ECU definition
      // Backend returns ChannelInfo[]; normalize to string[] (channel names) to avoid render errors
      invoke<any[]>('get_available_channels').then((channels) => {
        try {
          const names = (channels || []).map((c) => (typeof c === 'string' ? c : c?.name ?? String(c)));
          setAvailableChannels(names);
        } catch (e) {
          console.error('[SettingsDialog] Failed to normalize channels:', e);
          setAvailableChannels([]);
        }
      }).catch((e) => {
        console.error('[SettingsDialog] get_available_channels failed:', e);
        setAvailableChannels([]);
      });

      // Load demo mode state (runtime flag)
      invoke<boolean>('get_demo_mode')
        .then((v) => setDemoMode(!!v))
        .catch(console.error);
    }
  }, [theme, isOpen, currentProject]);

  // Lazy-load the ECU definitions list the first time the user searches, so
  // the cross-tab search has that content to match against. (Not loaded on
  // open to avoid unnecessary backend calls when the user never searches.)
  useEffect(() => {
    if (!isOpen || !settingsQuery.trim()) return;
    setIniLoading(true);
    invoke<any[]>('list_repository_inis')
      .then((list) => setIniList(Array.isArray(list) ? list : []))
      .catch(console.error)
      .finally(() => setIniLoading(false));
  }, [isOpen, settingsQuery]);

  // Focus management for keyboard navigation
  useEffect(() => {
    if (!isOpen) return;

    // Focus first input when dialog opens
    focusFirstElement('.dialog');
    
    // Create focus trap to keep Tab within the dialog
    const cleanupFocusTrap = createFocusTrap('.dialog');

    // Handle Escape key to close dialog
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !e.defaultPrevented) {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener('keydown', handleEscape);

    return () => {
      cleanupFocusTrap();
      document.removeEventListener('keydown', handleEscape);
    };
  }, [isOpen, onClose]);

  // Keep the local checkbox in sync with authoritative app state (the prop),
  // so the control reflects the live value even if it changed elsewhere.
  useEffect(() => {
    if (showEcuMenusInMenubar !== undefined) {
      setLocalShowEcuMenusInMenubar(showEcuMenusInMenubar);
    }
  }, [showEcuMenusInMenubar]);

  // Settings search filter: mirrors the sidebar search UX. When the query is
  // non-empty, sections are matched across ALL tabs (the tab bar is hidden and
  // every panel is shown) and non-matching sections are hidden. Each section is
  // delimited by an <h3> header; a section spans from its <h3> to the next <h3>
  // (or the end of its panel). We match against the section's full textContent
  // so every label, option, and note is searchable without a curated index.
  useEffect(() => {
    const container = contentRef.current;
    if (!container) return;

    const query = settingsQuery.trim().toLowerCase();
    const tablist = container.querySelector('.dialog-tabs');
    const panels = Array.from(container.querySelectorAll<HTMLElement>('.dialog-tab-content'));
    const noResults = container.querySelector<HTMLElement>('.settings-search-no-results');

    // Helper: gather each <h3>-anchored section as the <h3> plus all siblings
    // that follow it until the next <h3> (or end of parent). Returns groups of
    // elements that should be shown/hidden together.
    const getSections = (): HTMLElement[][] => {
      const sections: HTMLElement[][] = [];
      for (const panel of panels) {
        // Top-of-panel fields before any <h3> form their own implicit section
        // (e.g. Language/Units on the General tab).
        const leading: HTMLElement[] = [];
        let current: HTMLElement[] | null = null;
        for (let i = 0; i < panel.children.length; i++) {
          const child = panel.children[i] as HTMLElement;
          if (child.tagName === 'H3') {
            if (current) sections.push(current);
            current = [child];
          } else if (current) {
            current.push(child);
          } else {
            leading.push(child);
          }
        }
        if (leading.length > 0) sections.push(leading);
        if (current) sections.push(current);
      }
      return sections;
    };

    if (!query) {
      // Restore normal tabbed view: re-show the tab bar, clear per-section
      // inline display styles, and let each panel's `hidden` prop (driven by
      // currentTab) control visibility again.
      if (tablist) tablist.removeAttribute('hidden');
      if (noResults) noResults.hidden = true;
      panels.forEach(panel => {
        Array.from(panel.children).forEach(child => {
          (child as HTMLElement).style.display = '';
        });
      });
      return;
    }

    // Searching: show all panels, hide the tab bar.
    if (tablist) tablist.setAttribute('hidden', '');
    panels.forEach(panel => panel.removeAttribute('hidden'));

    const sections = getSections();
    let matchCount = 0;
    for (const section of sections) {
      const text = section.map(el => el.textContent || '').join(' ').toLowerCase();
      const matches = text.includes(query);
      section.forEach(el => {
        el.style.display = matches ? '' : 'none';
      });
      if (matches) matchCount++;
    }

    if (noResults) noResults.hidden = matchCount > 0;
  }, [settingsQuery, currentTab, isOpen]);

  // Clear the search when the dialog closes so it reopens in the normal view.
  useEffect(() => {
    if (!isOpen) setSettingsQuery('');
  }, [isOpen]);

  const handleClearSearch = useCallback(() => {
    setSettingsQuery('');
    searchInputRef.current?.focus();
  }, []);

  const handleDemoToggle = useCallback(async (enabled: boolean) => {
    setDemoLoading(true);
    try {
      await invoke('set_demo_mode', { enabled });
      setDemoMode(enabled);
      onSettingsChange?.({ demoMode: enabled });
    } catch (e) {
      console.error('Failed to toggle demo mode:', e);
      alert(`Failed to toggle demo mode: ${e}`);
    } finally {
      setDemoLoading(false);
    }
  }, [onSettingsChange]);

  const handleSwitchIni = useCallback(async () => {
    if (!currentProject) {
      alert('No project is currently open');
      return;
    }

    setSwitchingIni(true);
    try {
      const selected = await open({
        title: 'Select ECU Definition File',
        multiple: false,
        filters: [
          { name: 'INI Files', extensions: ['ini'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });

      if (selected && typeof selected === 'string') {
        // Update the project's INI file
        await invoke('update_project_ini', { 
          iniPath: selected, 
          forceResync: false 
        });
        
        setCurrentIniPath(selected);
        
        // Show success message with helpful info
        const message = 'ECU definition updated successfully!\n\n' +
          'The project tune has been re-applied with the new definition. ' +
          'If tables appear empty, you may need to load a matching MSQ file ' +
          'that was created with this INI definition.';
        alert(message);
      }
    } catch (e) {
      console.error('Failed to switch INI:', e);
      alert(`Failed to switch INI file: ${e}`);
    } finally {
      setSwitchingIni(false);
    }
  }, [currentProject]);

  // Persist all settings to the backend WITHOUT closing the dialog. Sends all
  // settings in a SINGLE batched `update_settings` call (one disk read + one
  // disk write on the backend) instead of ~30 sequential `update_setting`
  // calls (which each did a full read+write cycle and took several seconds).
  // Returns the list of per-setting errors (empty on full success).
  const saveSettings = useCallback(async (): Promise<string[]> => {
    setSaveStatus('saving');
    setSaveError(null);
    const errors: string[] = [];

    // Wrap the whole save body so saveStatus can NEVER get stuck at 'saving'
    // (which would leave Apply/OK permanently disabled). Any unexpected throw
    // still resolves the status to 'error' via the finally.
    try {
      onThemeChange(localTheme);
      // Apply language change immediately and persist it. Dynamically import the
      // i18n instance so loading this dialog module doesn't drag in i18next on
      // first paint of the app.
      try {
        const { default: i18n } = await import('../../../i18n');
        if (i18n.language !== localLanguage) {
          await i18n.changeLanguage(localLanguage);
        }
      } catch (e) {
        console.error('Failed to switch language:', e);
      }
      try {
        localStorage.setItem(LANGUAGE_STORAGE_KEY, localLanguage);
      } catch { /* ignore */ }

      // Build the batch of [key, value] pairs. Order matters for the AI
      // assistant settings: provider/key are listed before risk-ack/enable so
      // the backend's enable-guard (which requires risk-ack) sees the ack
      // applied in the same in-memory cycle. Provider/key changes reset the
      // ack on the backend side; the ack/enable pairs after them re-set it.
      const updates: [string, string][] = [
        ['language', localLanguage],
        // Update units setting (normalize invalid to metric)
        ['units_system', (localUnits !== 'metric' && localUnits !== 'imperial') ? 'metric' : localUnits],
        ['auto_burn_on_close', autoBurnOnClose.toString()],
        ['status_bar_channels', JSON.stringify(statusBarChannels)],
        ['indicator_column_count', indicatorColumnCount],
        ['indicator_fill_empty', indicatorFillEmpty.toString()],
        ['indicator_text_fit', indicatorTextFit],
        ['heatmap_value_scheme', heatmapValueScheme],
        ['heatmap_change_scheme', heatmapChangeScheme],
        ['heatmap_coverage_scheme', heatmapCoverageScheme],
        ['gauge_snap_to_grid', gaugeSnapToGrid.toString()],
        ['gauge_free_move', gaugeFreeMove.toString()],
        ['gauge_lock', gaugeLock.toString()],
        ['auto_sync_gauge_ranges', autoSyncGaugeRanges.toString()],
        ['auto_commit_on_save', autoCommitOnSave],
        ['commit_message_format', commitMessageFormat],
        ['runtime_packet_mode', runtimePacketMode],
        ['ai_provider', aiProvider],
        ['ai_base_url', aiBaseUrl],
        ['ai_api_key', aiApiKey],
        ['ai_model', aiModel],
        ['ai_capability_tier', aiCapabilityTier],
        ['ai_risk_acknowledged', aiRiskAcked.toString()],
        ['ai_assistant_enabled', aiEnabled.toString()],
        ['auto_reconnect_after_controller_command', autoReconnectAfterControllerCommand.toString()],
        ['auto_reconnect_after_firmware', autoReconnectAfterFirmware.toString()],
        ['auto_record_enabled', autoRecordEnabled.toString()],
        ['key_on_threshold_rpm', keyOnThresholdRpm.toString()],
        ['key_off_timeout_sec', keyOffTimeoutSec.toString()],
        ['alert_large_change_enabled', alertLargeChangeEnabled.toString()],
        ['alert_large_change_abs', alertLargeChangeAbs.toString()],
        ['alert_large_change_percent', alertLargeChangePercent.toString()],
      ];

      // Send the entire batch in one call (1 load + 1 save on the backend).
      try {
        await invoke('update_settings', { updates });
      } catch (e) {
        // The backend returns a newline-joined list of "key: error" for any
        // individual failures. Split them back into the errors array.
        errors.push(...String(e).split('\n').filter(Boolean));
      }

      // Normalize units state if it was coerced
      if (localUnits !== 'metric' && localUnits !== 'imperial') {
        setLocalUnits('metric');
      }

      // Hotkey bindings (separate command, not part of the settings struct)
      try {
        await invoke('save_hotkey_bindings', { bindings: hotkeyBindings });
      } catch (e) {
        errors.push(`hotkey_bindings: ${String(e)}`);
      }

      // Project-specific settings
      if (currentProject) {
        try {
          await invoke('update_project_auto_connect', { autoConnect });
        } catch (e) {
          errors.push(`project_auto_connect: ${String(e)}`);
        }
      }

      onSettingsChange?.({ units: localUnits, autoBurnOnClose, indicatorColumnCount, indicatorFillEmpty, indicatorTextFit, statusBarChannels, runtimePacketMode, autoSyncGaugeRanges });
    } catch (e) {
      // Catch any unexpected throw so it is surfaced rather than leaving the
      // dialog stuck in the 'saving' state.
      errors.push(`unexpected: ${String(e)}`);
    } finally {
      if (errors.length > 0) {
        setSaveStatus('error');
        setSaveError(errors.join('\n'));
      } else {
        setSaveStatus('saved');
      }
    }
    return errors;
  }, [localTheme, localLanguage, localUnits, autoBurnOnClose, statusBarChannels, indicatorColumnCount, indicatorFillEmpty, indicatorTextFit, heatmapValueScheme, heatmapChangeScheme, heatmapCoverageScheme, gaugeSnapToGrid, gaugeFreeMove, gaugeLock, autoSyncGaugeRanges, autoCommitOnSave, commitMessageFormat, runtimePacketMode, aiProvider, aiBaseUrl, aiApiKey, aiModel, aiCapabilityTier, aiRiskAcked, aiEnabled, autoReconnectAfterControllerCommand, autoReconnectAfterFirmware, autoRecordEnabled, keyOnThresholdRpm, keyOffTimeoutSec, alertLargeChangeEnabled, alertLargeChangeAbs, alertLargeChangePercent, hotkeyBindings, autoConnect, currentProject, onThemeChange, onSettingsChange]);

  // Windows-convention buttons:
  //  - Apply: save WITHOUT closing (so the user can verify it worked). The
  //    buttons re-enable as soon as the save resolves (status leaves 'saving').
  //  - OK:     save AND close.
  const handleApply = useCallback(async () => {
    await saveSettings();
  }, [saveSettings]);

  const handleOk = useCallback(async () => {
    await saveSettings();
    onClose();
  }, [saveSettings, onClose]);

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="Settings"
      size="xl"
      className="settings-dialog"
      ariaLabel="Settings dialog"
      titleAdornment={
        <div className="settings-search">
          <svg className="settings-search-icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M11.5 7a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0Zm-.82 4.74a6 6 0 1 1 1.06-1.06l3.04 3.04a.75.75 0 1 1-1.06 1.06l-3.04-3.04Z" />
          </svg>
          <input
            ref={searchInputRef}
            type="search"
            className="settings-search-input"
            placeholder="Search settings..."
            value={settingsQuery}
            onChange={(e) => setSettingsQuery(e.target.value)}
            aria-label="Search settings"
          />
          {settingsQuery && (
            <button
              type="button"
              className="settings-search-clear"
              onClick={handleClearSearch}
              title="Clear search"
              aria-label="Clear search"
            >
              ✕
            </button>
          )}
        </div>
      }
    >
        {/* Tab Navigation */}
        <div className="dialog-tabs" role="tablist">
          <button 
            className={`dialog-tab ${currentTab === 'general' ? 'active' : ''}`}
            onClick={() => setCurrentTab('general')}
            role="tab"
            id="general-tab"
            aria-selected={currentTab === 'general'}
            aria-controls="general-panel"
          >
            General
          </button>
          <button
            className={`dialog-tab ${currentTab === 'appearance' ? 'active' : ''}`}
            onClick={() => setCurrentTab('appearance')}
            role="tab"
            id="appearance-tab"
            aria-selected={currentTab === 'appearance'}
            aria-controls="appearance-panel"
          >
            Appearance
          </button>
          <button 
            className={`dialog-tab ${currentTab === 'definitions' ? 'active' : ''}`}
            onClick={() => {
              setCurrentTab('definitions');
              setIniLoading(true);
              invoke<any[]>('list_repository_inis').then(setIniList).catch(console.error).finally(() => setIniLoading(false));
            }}
            role="tab"
            id="definitions-tab"
            aria-selected={currentTab === 'definitions'}
            aria-controls="definitions-panel"
          >
            ECU Definitions
          </button>
          <button 
            className={`dialog-tab ${currentTab === 'hotkeys' ? 'active' : ''}`}
            onClick={() => setCurrentTab('hotkeys')}
            role="tab"
            id="hotkeys-tab"
            aria-selected={currentTab === 'hotkeys'}
            aria-controls="hotkeys-panel"
          >
            Keyboard Shortcuts
          </button>
        </div>
        
        <div className="dialog-content" ref={contentRef}>
          <div className="settings-search-no-results" hidden>
            No settings found for "{settingsQuery}"
          </div>
          {(currentTab === 'general' || settingsQuery.trim()) && (
            <div className="dialog-tab-content" id="general-panel" role="tabpanel" aria-labelledby="general-tab" hidden={currentTab !== 'general' && !settingsQuery.trim()}>
              <FormField label="Language">
                {(id) => (
                  <select
                    id={id}
                    value={localLanguage}
                    onChange={(e) => setLocalLanguage(e.target.value as SupportedLanguageCode)}
                  >
                    {SUPPORTED_LANGUAGES.map(lang => (
                      <option key={lang.code} value={lang.code}>{lang.label}</option>
                    ))}
                  </select>
                )}
              </FormField>
          
          <FormField label="Units Preset">
            {(id) => (
              <select
                id={id}
                value={localUnits}
                onChange={(e) => {
                  setLocalUnits(e.target.value);
                  if (e.target.value === 'metric') {
                    unitPrefs.useMetricUnits();
                  } else if (e.target.value === 'imperial') {
                    unitPrefs.useUSUnits();
                  }
                }}
              >
                <option value="metric">Metric (°C, kPa)</option>
                <option value="imperial">Imperial (°F, PSI)</option>
                <option value="custom">Custom</option>
              </select>
            )}
          </FormField>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Unit Preferences</h3>

          <FormField label="Temperature">
            {(id) => (
              <select
                id={id}
                value={unitPrefs.preferences.temperature}
                onChange={(e) => {
                  unitPrefs.updatePreference('temperature', e.target.value as TemperatureUnit);
                  setLocalUnits('custom');
                }}
              >
                <option value="C">Celsius (°C)</option>
                <option value="F">Fahrenheit (°F)</option>
                <option value="K">Kelvin (K)</option>
              </select>
            )}
          </FormField>

          <FormField label="Pressure">
            {(id) => (
              <select
                id={id}
                value={unitPrefs.preferences.pressure}
                onChange={(e) => {
                  unitPrefs.updatePreference('pressure', e.target.value as PressureUnit);
                  setLocalUnits('custom');
                }}
              >
                <option value="kPa">Kilopascals (kPa)</option>
                <option value="PSI">PSI</option>
                <option value="bar">Bar</option>
                <option value="inHg">Inches of Mercury (inHg)</option>
              </select>
            )}
          </FormField>

          <FormField label="Air-Fuel Ratio">
            {(id) => (
              <select
                id={id}
                value={unitPrefs.preferences.afr}
                onChange={(e) => {
                  unitPrefs.updatePreference('afr', e.target.value as AfrUnit);
                  setLocalUnits('custom');
                }}
              >
                <option value="AFR">AFR (Air-Fuel Ratio)</option>
                <option value="Lambda">Lambda (λ)</option>
              </select>
            )}
          </FormField>

          <FormField label="Speed">
            {(id) => (
              <select
                id={id}
                value={unitPrefs.preferences.speed}
                onChange={(e) => {
                  unitPrefs.updatePreference('speed', e.target.value as SpeedUnit);
                  setLocalUnits('custom');
                }}
              >
                <option value="km/h">km/h</option>
                <option value="mph">mph</option>
              </select>
            )}
          </FormField>

          <FormField label="Fuel Type (for Lambda ↔ AFR)">
            {(id) => (
              <select
                id={id}
                value={unitPrefs.preferences.fuelType}
                onChange={(e) => unitPrefs.updatePreference('fuelType', e.target.value as FuelType)}
              >
                <option value="gasoline">Gasoline (λ=1 @ {STOICH_AFR.gasoline}:1)</option>
                <option value="e85">E85 (λ=1 @ {STOICH_AFR.e85}:1)</option>
                <option value="ethanol">Ethanol (λ=1 @ {STOICH_AFR.ethanol}:1)</option>
                <option value="methanol">Methanol (λ=1 @ {STOICH_AFR.methanol}:1)</option>
                <option value="diesel">Diesel (λ=1 @ {STOICH_AFR.diesel}:1)</option>
              </select>
            )}
          </FormField>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={autoBurnOnClose}
                onChange={(e) => setAutoBurnOnClose(e.target.checked)}
              />
              Auto-burn on close
            </label>
            <span className="dialog-form-note">Shows confirmation before burning</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={demoMode}
                disabled={demoLoading}
                onChange={(e) => handleDemoToggle(e.target.checked)}
              />
              Demo Mode (simulate ECU)
            </label>
            <span className="dialog-form-note">Simulate ECU data for testing (runtime-only)</span>
          </div>

          <FormField
            label="Default Runtime Packet Mode"
            help={<>
              Default runtime packet mode for new connections.{' '}
              OCH (On-Controller Block Read): use INI-defined block reads when supported by the ECU (configured via <code>ochGetCommand</code> / <code>ochBlockSize</code>).
            </>}
          >
            {(id) => (
              <select
                id={id}
                value={runtimePacketMode}
                onChange={(e) => setRuntimePacketMode(e.target.value as any)}
              >
                <option value={'Auto'}>Auto (recommended)</option>
                <option value={'ForceBurst'}>Force Burst</option>
                <option value={'ForceOCH'}>Force OCH</option>
                <option value={'Disabled'}>Disabled (use Burst)</option>
              </select>
            )}
          </FormField>

          {/* Auto-reconnect after controller commands */}
          <div className="dialog-form-group" style={{ marginTop: '0.5rem' }}>
            <label>
              <input
                type="checkbox"
                checked={autoReconnectAfterControllerCommand}
                onChange={(e) => setAutoReconnectAfterControllerCommand(e.target.checked)}
              />
              Auto-sync & reconnect after controller commands
            </label>
            <span className="dialog-form-note">When enabled, the app will automatically sync and reconnect to the ECU after executing controller commands that modify ECU settings (e.g., applying base maps).</span>
          </div>

          <div className="dialog-form-group" style={{ marginTop: '0.5rem' }}>
            <label>
              <input
                type="checkbox"
                checked={autoReconnectAfterFirmware}
                onChange={(e) => setAutoReconnectAfterFirmware(e.target.checked)}
              />
              Auto-reconnect after firmware updates
            </label>
            <span className="dialog-form-note">
              After OpenBLT updates, LibreTune waits for the ECU to reboot and retries the last known port automatically.
            </span>
          </div>

          {/* Show small live metrics in connection dialog too */}
          <div style={{ marginTop: '0.6rem' }}>
            <ConnectionMetrics compact />
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Status Bar</h3>
          
          <div className="dialog-form-group">
            <StatusBarChannelSelector 
              selectedChannels={statusBarChannels}
              availableChannels={availableChannels}
              onChannelsChange={setStatusBarChannels}
              maxChannels={64}
            />
            <span className="dialog-form-note">Select which realtime channels appear in the status bar. Use drag-drop to reorder, or leave empty for auto-detection from ECU definition.</span>
          </div>

          {currentProject && (
            <>
              <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Project Settings</h3>
              
              <div className="dialog-form-group">
                <label>
                  <input
                    type="checkbox"
                    checked={autoConnect}
                    onChange={(e) => setAutoConnect(e.target.checked)}
                  />
                  {' '}Auto-connect to ECU when port is available
                </label>
                <span className="dialog-form-note">
                  Remembers the last successful COM port for this project. When enabled,
                  LibreTune checks every few seconds whether that port is present and
                  attempts to connect automatically (on startup, project open, or when you
                  plug in the ECU).
                </span>
              </div>
              
              <div className="dialog-form-group">
                <label>ECU Definition (INI File)</label>
                <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                  <button type="button"
                    title={currentIniPath || 'Not set'}
                    className="ini-select-btn"
                    onClick={handleSwitchIni}
                    style={{ flex: 1, padding: '0.5rem', fontSize: '0.9rem', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '4px' }}
                  >
                    {currentIniPath ? currentIniPath.split(/[\\\/]/).pop() || currentIniPath : 'Not set'}
                    <span style={{ float: 'right', opacity: 0.85 }}>{switchingIni ? 'Switching...' : 'Change'}</span>
                  </button>

                </div>
                <span className="dialog-form-note">
                  Switch to a different ECU definition file. The project tune will be re-applied automatically.
                </span>
              </div>
            </>
          )}

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Dashboard</h3>
          
          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeSnapToGrid}
                onChange={(e) => setGaugeSnapToGrid(e.target.checked)}
              />
              Snap gauges to grid
            </label>
            <span className="dialog-form-note">Align gauges when dragging in designer mode</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeFreeMove}
                onChange={(e) => setGaugeFreeMove(e.target.checked)}
              />
              Free move (ignore snap)
            </label>
            <span className="dialog-form-note">Allow gauges to be placed anywhere</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeLock}
                onChange={(e) => setGaugeLock(e.target.checked)}
              />
              Lock gauge positions
            </label>
            <span className="dialog-form-note">Prevent accidental gauge movement</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={autoSyncGaugeRanges}
                onChange={(e) => setAutoSyncGaugeRanges(e.target.checked)}
              />
              Auto-sync gauge ranges from INI
            </label>
            <span className="dialog-form-note">Apply INI gauge min/max/units automatically when a project or INI changes</span>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Version Control</h3>
          
          <FormField
            label="Auto-Commit on Save"
            help="Automatically create a Git commit when saving the tune"
          >
            {(id) => (
              <select
                id={id}
                value={autoCommitOnSave}
                onChange={(e) => setAutoCommitOnSave(e.target.value)}
              >
                <option value="never">Never</option>
                <option value="always">Always</option>
                <option value="ask">Ask each time</option>
              </select>
            )}
          </FormField>

          <FormField
            label="Commit Message Format"
            help={<>Available placeholders: {'{date}'}, {'{time}'}, {'{table}'}</>}
          >
            {(id) => (
              <input
                id={id}
                type="text"
                value={commitMessageFormat}
                onChange={(e) => setCommitMessageFormat(e.target.value)}
                style={{ fontFamily: 'monospace' }}
              />
            )}
          </FormField>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>AI Assistant (at your own risk)</h3>
          <span className="dialog-form-note">
            Bring your own LLM provider. The assistant only ever <strong>proposes</strong> changes for
            your explicit approval — nothing is burned to the ECU automatically.
          </span>

          <FormField label="Enable AI Assistant" help="Requires acknowledging the risk warning below">
            {(id) => (
              <label className="dialog-checkbox-option" style={{ display: 'inline-flex', gap: '0.4rem' }}>
                <input
                  id={id}
                  type="checkbox"
                  checked={aiEnabled}
                  onChange={(e) => setAiEnabled(e.target.checked)}
                  disabled={!aiRiskAcked}
                />
                <span>{aiEnabled ? 'Enabled' : 'Disabled'}{!aiRiskAcked ? ' (acknowledge risk first)' : ''}</span>
              </label>
            )}
          </FormField>

          <RiskAcknowledgement
            acknowledged={aiRiskAcked}
            onAcknowledgedChange={setAiRiskAcked}
            risk="high"
            label="At your own risk"
            warning={
              <>
                The assistant sends tune/configuration data to the configured LLM provider and may
                propose changes that alter engine behavior. Proposals are validated and clamped, but
                a storable value can still be <em>wrong</em> (e.g. pin assignments). You must review
                every proposal before it is staged, and burning to the ECU is always a separate manual
                step. You assume all risk.
              </>
            }
            acknowledgementText="I understand the assistant only proposes changes, that I must approve them, and that I am responsible for verifying every change before it is applied or burned."
          />

          <FormField label="Provider" help="OpenAI = most hosted/local-compatible endpoints; Anthropic & Google are native protocols">
            {(id) => (
              <select
                id={id}
                value={aiProvider}
                onChange={(e) => setAiProvider(e.target.value)}
              >
                <option value="openai">OpenAI (and compatible: OpenRouter, Ollama, LM Studio)</option>
                <option value="anthropic">Anthropic (Claude)</option>
                <option value="google">Google (Gemini)</option>
              </select>
            )}
          </FormField>

          <FormField label="Base URL" help="Leave empty for the provider default. For local models use e.g. http://localhost:11434/v1 (Ollama)">
            {(id) => (
              <input
                id={id}
                type="text"
                value={aiBaseUrl}
                onChange={(e) => setAiBaseUrl(e.target.value)}
                placeholder="(provider default)"
                style={{ fontFamily: 'monospace' }}
              />
            )}
          </FormField>

          <FormField label="API Key" help="Stored locally in settings. Optional for local/no-auth providers">
            {(id) => (
              <input
                id={id}
                type="password"
                value={aiApiKey}
                onChange={(e) => setAiApiKey(e.target.value)}
                placeholder="sk-..."
                style={{ fontFamily: 'monospace' }}
                autoComplete="off"
              />
            )}
          </FormField>

          <FormField label="Model" help="e.g. gpt-4o, claude-3-5-sonnet-20241022, gemini-1.5-pro">
            {(id) => (
              <input
                id={id}
                type="text"
                value={aiModel}
                onChange={(e) => setAiModel(e.target.value)}
                style={{ fontFamily: 'monospace' }}
              />
            )}
          </FormField>

          <FormField label="Capability Tier" help="The scope the assistant is unlocked for">
            {(id) => (
              <select
                id={id}
                value={aiCapabilityTier}
                onChange={(e) => setAiCapabilityTier(e.target.value as 'read' | 'tune' | 'config')}
              >
                <option value="read">Read / diagnose only</option>
                <option value="config">Propose ECU configuration changes</option>
                <option value="tune">Propose tune edits</option>
              </select>
            )}
          </FormField>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Indicator Panel</h3>
          
          <FormField label="Column Count">
            {(id) => (
              <select
                id={id}
                value={indicatorColumnCount}
                onChange={(e) => setIndicatorColumnCount(e.target.value)}
              >
                <option value="auto">Auto (fill width)</option>
                <option value="8">8 columns</option>
                <option value="10">10 columns</option>
                <option value="12">12 columns</option>
                <option value="14">14 columns</option>
                <option value="16">16 columns</option>
              </select>
            )}
          </FormField>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={indicatorFillEmpty}
                onChange={(e) => setIndicatorFillEmpty(e.target.checked)}
              />
              Fill empty cells in last row
            </label>
            <span className="dialog-form-note">Add blank cells to complete the grid</span>
          </div>

          <FormField label="Text Fit Mode">
            {(id) => (
              <select
                id={id}
                value={indicatorTextFit}
                onChange={(e) => setIndicatorTextFit(e.target.value)}
              >
                <option value="scale">Scale to fit</option>
                <option value="wrap">Wrap text (2 lines)</option>
              </select>
            )}
          </FormField>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Data Logging</h3>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={autoRecordEnabled}
                onChange={(e) => setAutoRecordEnabled(e.target.checked)}
              />
              Enable auto-record
            </label>
            <span className="dialog-form-note">Automatically start/stop recording when ECU key is turned on/off</span>
          </div>

          <div className="dialog-form-group">
            <label>Key-On Threshold (RPM)</label>
            <input
              type="range"
              min="50"
              max="500"
              step="50"
              value={keyOnThresholdRpm}
              onChange={(e) => setKeyOnThresholdRpm(Number(e.target.value))}
              style={{ width: '100%' }}
            />
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.85rem', marginTop: '0.25rem' }}>
              <span>50</span>
              <span><strong>{keyOnThresholdRpm}</strong></span>
              <span>500</span>
            </div>
            <span className="dialog-form-note">RPM threshold for detecting key-on event; recording starts when RPM exceeds this value</span>
          </div>

          <div className="dialog-form-group">
            <label>Key-Off Timeout (seconds)</label>
            <input
              type="range"
              min="1"
              max="10"
              step="0.5"
              value={keyOffTimeoutSec}
              onChange={(e) => setKeyOffTimeoutSec(Number(e.target.value))}
              style={{ width: '100%' }}
            />
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.85rem', marginTop: '0.25rem' }}>
              <span>1 sec</span>
              <span><strong>{keyOffTimeoutSec.toFixed(1)}</strong></span>
              <span>10 sec</span>
            </div>
            <span className="dialog-form-note">Time to wait below threshold before stopping recording; prevents multiple stop/start cycles during brief RPM dips</span>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Alert Rules</h3>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={alertLargeChangeEnabled}
                onChange={(e) => setAlertLargeChangeEnabled(e.target.checked)}
              />
              Warn on large table changes
            </label>
            <span className="dialog-form-note">Shows a warning when changes exceed thresholds</span>
          </div>

          <FormField
            label="Absolute Change Threshold"
            help="Warn if a cell changes by more than this amount"
          >
            {(id) => (
              <input
                id={id}
                type="number"
                min="0"
                step="0.1"
                value={alertLargeChangeAbs}
                onChange={(e) => setAlertLargeChangeAbs(Number(e.target.value))}
              />
            )}
          </FormField>

          <FormField
            label="Percent Change Threshold (%)"
            help="Warn if a cell changes by more than this percent"
          >
            {(id) => (
              <input
                id={id}
                type="number"
                min="0"
                step="1"
                value={alertLargeChangePercent}
                onChange={(e) => setAlertLargeChangePercent(Number(e.target.value))}
              />
            )}
          </FormField>
            </div>
          )}

          {(currentTab === 'appearance' || settingsQuery.trim()) && (
            <div className="dialog-tab-content" id="appearance-panel" role="tabpanel" aria-labelledby="appearance-tab" hidden={currentTab !== 'appearance' && !settingsQuery.trim()}>
              <FormField label="Theme">
                {() => (
                  <ThemePicker
                    selectedTheme={localTheme as ThemeName}
                    onChange={(theme) => setLocalTheme(theme)}
                  />
                )}
              </FormField>

              <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Layout</h3>

              <div className="dialog-form-group">
                <label>
                  <input
                    type="checkbox"
                    checked={localShowEcuMenusInMenubar}
                    onChange={(e) => {
                      const enabled = e.target.checked;
                      setLocalShowEcuMenusInMenubar(enabled);
                      invoke('update_setting', { key: 'show_ecu_menus_in_menubar', value: String(enabled) }).catch(() => {});
                      onEcuMenusInMenubarChange?.(enabled);
                    }}
                  />
                  Show ECU menus in menu bar
                </label>
                <span className="dialog-form-note">When off, ECU tuning menus are still available in the sidebar</span>
              </div>

              <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Heatmap Colors</h3>

              <FormField label="Value Tables (VE, Timing)">
                {(id) => (
                  <select
                    id={id}
                    value={heatmapValueScheme}
                    onChange={(e) => setHeatmapValueScheme(e.target.value as HeatmapScheme)}
                  >
                    {availableSchemes.filter(s => s.id !== 'custom').map(scheme => (
                      <option key={scheme.id} value={scheme.id}>
                        {scheme.name} {scheme.colorblindSafe && '(colorblind-safe)'}
                      </option>
                    ))}
                  </select>
                )}
              </FormField>

              <FormField label="Change Display (AFR Correction)">
                {(id) => (
                  <select
                    id={id}
                    value={heatmapChangeScheme}
                    onChange={(e) => setHeatmapChangeScheme(e.target.value as HeatmapScheme)}
                  >
                    {availableSchemes.filter(s => s.id !== 'custom').map(scheme => (
                      <option key={scheme.id} value={scheme.id}>
                        {scheme.name} {scheme.colorblindSafe && '(colorblind-safe)'}
                      </option>
                    ))}
                  </select>
                )}
              </FormField>

              <FormField label="Coverage Display (Hit Weighting)">
                {(id) => (
                  <select
                    id={id}
                    value={heatmapCoverageScheme}
                    onChange={(e) => setHeatmapCoverageScheme(e.target.value as HeatmapScheme)}
                  >
                    {availableSchemes.filter(s => s.id !== 'custom').map(scheme => (
                      <option key={scheme.id} value={scheme.id}>
                        {scheme.name} {scheme.colorblindSafe && '(colorblind-safe)'}
                      </option>
                    ))}
                  </select>
                )}
              </FormField>

              <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Table Display</h3>

              <div className="dialog-form-group">
                <label>
                  <input
                    type="checkbox"
                    checked={tableYAxisBottom}
                    onChange={(e) => {
                      const enabled = e.target.checked;
                      setTableYAxisBottom(enabled);
                      invoke('update_setting', { key: 'table_y_axis_bottom', value: String(enabled) }).catch(() => {});
                    }}
                  />
                  Table Y axis zero at bottom
                </label>
                <span className="dialog-form-note">Show the lowest load row at the bottom of tables</span>
              </div>

              <div className="dialog-form-group">
                <label style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <input
                    type="color"
                    value={tableCursorColor || '#00ff00'}
                    onChange={(e) => {
                      setTableCursorColor(e.target.value);
                      invoke('update_setting', { key: 'table_cursor_color', value: e.target.value }).catch(() => {});
                    }}
                  />
                  Live cursor color
                  <input
                    type="color"
                    value={tableTrailColor || '#4A90E2'}
                    onChange={(e) => {
                      setTableTrailColor(e.target.value);
                      invoke('update_setting', { key: 'table_trail_color', value: e.target.value }).catch(() => {});
                    }}
                    style={{ marginLeft: 16 }}
                  />
                  Trail color
                </label>
                <span className="dialog-form-note">Colors of the operating-point marker and its trace on tables</span>
              </div>

              <div className="dialog-form-group">
                <label style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <input
                    type="number"
                    min={0}
                    step={1}
                    value={tableTrailFadeSec}
                    onChange={(e) => {
                      const v = Math.max(0, parseFloat(e.target.value) || 0);
                      setTableTrailFadeSec(v);
                      invoke('update_setting', { key: 'table_trail_fade_sec', value: String(v) }).catch(() => {});
                    }}
                    style={{ width: 70 }}
                  />
                  Trail fade time (seconds)
                </label>
                <span className="dialog-form-note">How long trace points stay on the table; 0 keeps them forever</span>
              </div>
            </div>
          )}

          {(currentTab === 'definitions' || settingsQuery.trim()) && (
            <div className="dialog-tab-content" id="definitions-panel" role="tabpanel" aria-labelledby="definitions-tab" hidden={currentTab !== 'definitions' && !settingsQuery.trim()}>
              <div className="dialog-form-group">
                <label>Imported ECU Definitions (INI Files)</label>
                <p style={{ fontSize: 12, color: 'var(--text-muted)', margin: '4px 0 12px' }}>
                  Manage the ECU definition files available for projects.
                </p>
                <button
                  style={{ marginBottom: 12, padding: '6px 14px', background: 'var(--primary)', color: 'white', border: 'none', borderRadius: 6, cursor: 'pointer' }}
                  onClick={async () => {
                    try {
                      const path = await import('@tauri-apps/plugin-dialog').then(d => d.open({
                        multiple: false,
                        filters: [{ name: 'INI Files', extensions: ['ini'] }],
                      }));
                      if (path && typeof path === 'string') {
                        await invoke('import_ini', { path });
                        const list = await invoke<any[]>('list_repository_inis');
                        setIniList(list);
                      }
                    } catch (e) {
                      console.error('Failed to import INI:', e);
                    }
                  }}
                >
                  Import INI File...
                </button>

                {iniLoading ? (
                  <div style={{ padding: 20, textAlign: 'center', color: 'var(--text-muted)' }}>Loading...</div>
                ) : iniList.length === 0 ? (
                  <div style={{ padding: 20, textAlign: 'center', color: 'var(--text-muted)' }}>
                    No ECU definitions imported yet.
                  </div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 400, overflowY: 'auto' }}>
                    {iniList.map((ini) => (
                      <div key={ini.id} style={{
                        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                        padding: '10px 12px', background: 'var(--bg-elevated)', borderRadius: 6,
                        border: '1px solid var(--border-default)',
                      }}>
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ fontWeight: 600, fontSize: 13 }}>{ini.name}</div>
                          <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>{ini.signature}</div>
                        </div>
                        <button
                          style={{
                            padding: '4px 10px', background: 'none', border: '1px solid var(--border-default)',
                            borderRadius: 4, color: deletingIni === ini.id ? 'var(--error)' : 'var(--text-muted)',
                            cursor: 'pointer', fontSize: 12, marginLeft: 8,
                          }}
                          onClick={async () => {
                            if (deletingIni === ini.id) {
                              try {
                                await invoke('remove_ini', { iniId: ini.id });
                                setIniList(prev => prev.filter(i => i.id !== ini.id));
                              } catch (e) {
                                console.error('Failed to remove INI:', e);
                              }
                              setDeletingIni(null);
                            } else {
                              setDeletingIni(ini.id);
                            }
                          }}
                        >
                          {deletingIni === ini.id ? 'Confirm Remove' : 'Remove'}
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {(currentTab === 'hotkeys' || settingsQuery.trim()) && (
            <div className="dialog-tab-content" id="hotkeys-panel" role="tabpanel" aria-labelledby="hotkeys-tab" hidden={currentTab !== 'hotkeys' && !settingsQuery.trim()}>
              {hotkeysLoading ? (
                <div className="dialog-loading">Loading keyboard shortcuts...</div>
              ) : (
                <HotkeyEditor 
                  bindings={hotkeyBindings}
                  onChange={setHotkeyBindings}
                />
              )}
            </div>
          )}
        </div>

      <Dialog.Footer>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', width: '100%' }}>
          {saveStatus === 'saving' && (
            <span style={{ fontSize: '12px', opacity: 0.7 }}>Saving…</span>
          )}
          {saveStatus === 'saved' && (
            <span style={{ fontSize: '12px', color: '#80d090' }}>Settings saved.</span>
          )}
          {saveStatus === 'error' && (
            <span
              title={saveError ?? ''}
              style={{ fontSize: '12px', color: '#f0a0a0', cursor: 'help', maxWidth: '60%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            >
              Some settings failed to save (hover for details)
            </span>
          )}
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <Button variant="secondary" onClick={onClose}>Cancel</Button>
          <Button variant="secondary" onClick={handleApply} disabled={saveStatus === 'saving'}>Apply</Button>
          <Button variant="primary" onClick={handleOk} disabled={saveStatus === 'saving'}>OK</Button>
        </div>
      </Dialog.Footer>
    </Dialog>
  );
}
