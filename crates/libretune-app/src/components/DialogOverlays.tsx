import { invoke } from "@tauri-apps/api/core";
import {
  SaveDialog,
  LoadDialog,
  BurnDialog,
  NewTuneDialog,
  SettingsDialog,
  AboutDialog,
  ConnectionDialog,
  Tab,
} from "./tuner-ui";
import HelpViewer, { HelpTopicData } from "./dialogs/HelpViewer";
import UserManualViewer from "./dialogs/UserManualViewer";
import SignatureMismatchDialog, { SignatureMismatchInfo } from "./dialogs/SignatureMismatchDialog";
import TuneMismatchDialog, { TuneMismatchInfo } from "./dialogs/TuneMismatchDialog";
import TuneComparisonDialog from "./dialogs/TuneComparisonDialog";
import TableComparisonDialog from "./dialogs/TableComparisonDialog";
import PerformanceFieldsDialog from "./dialogs/PerformanceFieldsDialog";
import RestorePointsDialog from "./dialogs/RestorePointsDialog";
import ImportProjectWizard from "./dialogs/ImportProjectWizard";
import MathChannelsDialog from "./dialogs/MathChannelsDialog";
import MigrationReportDialog from "./dialogs/MigrationReportDialog";
import TuneFileDiffDialog from "./dialogs/TuneFileDiffDialog";
import DynoOverlay from "./tuner-ui/DynoOverlay";
import NewProjectDialog from "./dialogs/NewProjectDialog";
import BaseMapDialog, { BaseMapResult } from "./dialogs/BaseMapDialog";
import TuneHistoryPanel from "./TuneHistoryPanel";
import ErrorDetailsDialog from "./dialogs/ErrorDetailsDialog";
import OnboardingDialog from "./dialogs/OnboardingDialog";
import { PluginPanel } from "./PluginPanel";
import { ControllerCommandDialog } from "./console/ControllerCommandDialog";
import { FirmwareUpdateDialog } from "./dialogs/FirmwareUpdateDialog";
import { ThemeName } from "../themes";
import {
  type ConnectionStatus,
  type ConnectResult,
  type SyncResult,
  type CurrentProject,
  type IniEntry,
  type ProjectInfo,
  type ProtocolDefaults,
  type TabContent,
  type IniCapabilities,
} from "../types/app";

type SyncProgress = { percent: number } | null;

export interface DialogOverlaysProps {
  // Generic
  status: ConnectionStatus;
  currentProject: CurrentProject | null;
  theme: ThemeName;
  setTheme: (theme: ThemeName) => void;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;

  // Save / Load / Burn
  saveDialogOpen: boolean;
  setSaveDialogOpen: (v: boolean) => void;
  autoBurnOnClose: boolean;
  loadDialogOpen: boolean;
  setLoadDialogOpen: (v: boolean) => void;
  burnDialogOpen: boolean;
  setBurnDialogOpen: (v: boolean) => void;
  firmwareUpdateDialogOpen: boolean;
  setFirmwareUpdateDialogOpen: (v: boolean) => void;
  iniCapabilities: IniCapabilities | null;
  newTuneDialogOpen: boolean;
  setNewTuneDialogOpen: (v: boolean) => void;

  // Settings
  settingsDialogOpen: boolean;
  setSettingsDialogOpen: (v: boolean) => void;
  setUnitsSystem: (v: 'metric' | 'imperial') => void;
  setAutoBurnOnClose: (v: boolean) => void;
  setStatus: React.Dispatch<React.SetStateAction<ConnectionStatus>>;
  setStatusBarChannels: (v: string[]) => void;
  setDefaultRuntimePacketMode: (v: any) => void;

  // Math channels / About
  mathChannelsDialogOpen: boolean;
  setMathChannelsDialogOpen: (v: boolean) => void;
  aboutDialogOpen: boolean;
  setAboutDialogOpen: (v: boolean) => void;

  // Connection
  connectionDialogOpen: boolean;
  setConnectionDialogOpen: (v: boolean) => void;
  ports: string[];
  selectedPort: string;
  baudRate: number;
  timeoutMs: number;
  connectionType: 'Serial' | 'Tcp';
  setConnectionType: (v: 'Serial' | 'Tcp') => void;
  tcpHost: string;
  setTcpHost: (v: string) => void;
  tcpPort: number;
  setTcpPort: (v: number) => void;
  setSelectedPort: (v: string) => void;
  handleBaudChange: (v: number) => void;
  handleTimeoutChange: (v: number) => void;
  connect: () => Promise<void> | void;
  disconnect: () => Promise<void> | void;
  refreshPorts: () => Promise<void> | void;
  connecting: boolean;
  syncing: boolean;
  syncProgress: SyncProgress;
  iniDefaults: ProtocolDefaults | null;
  applyIniDefaults: () => void;
  connectionRuntimePacketMode: any;
  setConnectionRuntimePacketMode: (v: any) => void;

  // Project
  newProjectDialogOpen: boolean;
  setNewProjectDialogOpen: (v: boolean) => void;
  repositoryInis: IniEntry[];
  setRepositoryInis: React.Dispatch<React.SetStateAction<IniEntry[]>>;
  createProject: (name: string, iniId: string) => Promise<boolean>;
  handleImportTuneIntoProject: (path: string) => Promise<void> | void;
  baseMapDialogOpen: boolean;
  setBaseMapDialogOpen: (v: boolean) => void;
  handleBaseMapApply: (baseMap: BaseMapResult) => Promise<void> | void;

  // Comparison dialogs
  tuneComparisonOpen: boolean;
  setTuneComparisonOpen: (v: boolean) => void;
  checkStatus: () => Promise<void> | void;
  tableComparisonOpen: boolean;
  setTableComparisonOpen: (v: boolean) => void;
  tuneFileDiffOpen: boolean;
  setTuneFileDiffOpen: (v: boolean) => void;

  // Misc overlays
  dynoOverlayOpen: boolean;
  setDynoOverlayOpen: (v: boolean) => void;
  performanceDialogOpen: boolean;
  setPerformanceDialogOpen: (v: boolean) => void;

  // Signature mismatch
  signatureMismatchOpen: boolean;
  signatureMismatchInfo: SignatureMismatchInfo | null;
  setSignatureMismatchOpen: (v: boolean) => void;
  setSignatureMismatchInfo: (v: SignatureMismatchInfo | null) => void;
  fetchConstants: () => Promise<Record<string, number>>;
  fetchMenuTree: (ctx?: Record<string, number>) => Promise<void> | void;
  doSync: () => Promise<SyncResult | null>;

  // Help / manual
  helpTopic: HelpTopicData | null;
  setHelpTopic: (v: HelpTopicData | null) => void;
  userManualOpen: boolean;
  setUserManualOpen: (v: boolean) => void;
  userManualSection: string | undefined;
  setUserManualSection: (v: string | undefined) => void;

  // Tune mismatch
  tuneMismatchOpen: boolean;
  tuneMismatchInfo: TuneMismatchInfo | null;
  setTuneMismatchOpen: (v: boolean) => void;
  setTuneMismatchInfo: (v: TuneMismatchInfo | null) => void;

  // Error dialog
  errorDialogOpen: boolean;
  errorInfo: { title: string; message: string; details?: string };
  hideError: () => void;

  // Restore points / Tune history / Import / Migration / Onboarding
  restorePointsOpen: boolean;
  setRestorePointsOpen: (v: boolean) => void;
  tuneHistoryOpen: boolean;
  setTuneHistoryOpen: (v: boolean) => void;
  importProjectOpen: boolean;
  setImportProjectOpen: (v: boolean) => void;
  setAvailableProjects: (v: ProjectInfo[]) => void;
  setCurrentProject: (v: CurrentProject | null) => void;
  setTabs: (v: Tab[]) => void;
  setTabContents: (v: Record<string, TabContent>) => void;
  setActiveTabId: (v: string) => void;
  migrationReportOpen: boolean;
  setMigrationReportOpen: (v: boolean) => void;
  onboardingOpen: boolean;
  setOnboardingOpen: (v: boolean) => void;

  // Plugins
  pluginPanelOpen: boolean;
  setPluginPanelOpen: (v: boolean) => void;
}

export function DialogOverlays(props: DialogOverlaysProps) {
  const {
    status, currentProject, theme, setTheme, showToast,
    saveDialogOpen, setSaveDialogOpen, autoBurnOnClose,
    loadDialogOpen, setLoadDialogOpen,
    burnDialogOpen, setBurnDialogOpen,
    firmwareUpdateDialogOpen, setFirmwareUpdateDialogOpen, iniCapabilities,
    newTuneDialogOpen, setNewTuneDialogOpen,
    settingsDialogOpen, setSettingsDialogOpen,
    setUnitsSystem, setAutoBurnOnClose, setStatus, setStatusBarChannels, setDefaultRuntimePacketMode,
    mathChannelsDialogOpen, setMathChannelsDialogOpen,
    aboutDialogOpen, setAboutDialogOpen,
    connectionDialogOpen, setConnectionDialogOpen,
    ports, selectedPort, baudRate, timeoutMs, connectionType, setConnectionType,
    tcpHost, setTcpHost, tcpPort, setTcpPort, setSelectedPort,
    handleBaudChange, handleTimeoutChange, connect, disconnect, refreshPorts,
    connecting, syncing, syncProgress, iniDefaults, applyIniDefaults,
    connectionRuntimePacketMode, setConnectionRuntimePacketMode,
    newProjectDialogOpen, setNewProjectDialogOpen,
    repositoryInis, setRepositoryInis, createProject, handleImportTuneIntoProject,
    baseMapDialogOpen, setBaseMapDialogOpen, handleBaseMapApply,
    tuneComparisonOpen, setTuneComparisonOpen, checkStatus,
    tableComparisonOpen, setTableComparisonOpen,
    tuneFileDiffOpen, setTuneFileDiffOpen,
    dynoOverlayOpen, setDynoOverlayOpen,
    performanceDialogOpen, setPerformanceDialogOpen,
    signatureMismatchOpen, signatureMismatchInfo, setSignatureMismatchOpen, setSignatureMismatchInfo,
    fetchConstants, fetchMenuTree, doSync,
    helpTopic, setHelpTopic,
    userManualOpen, setUserManualOpen, userManualSection, setUserManualSection,
    tuneMismatchOpen, tuneMismatchInfo, setTuneMismatchOpen, setTuneMismatchInfo,
    errorDialogOpen, errorInfo, hideError,
    restorePointsOpen, setRestorePointsOpen,
    tuneHistoryOpen, setTuneHistoryOpen,
    importProjectOpen, setImportProjectOpen, setAvailableProjects, setCurrentProject,
    setTabs, setTabContents, setActiveTabId,
    migrationReportOpen, setMigrationReportOpen,
    onboardingOpen, setOnboardingOpen,
    pluginPanelOpen, setPluginPanelOpen,
  } = props;

  return (
    <>
      <SaveDialog isOpen={saveDialogOpen} onClose={() => setSaveDialogOpen(false)} autoBurnOnClose={autoBurnOnClose} />
      <LoadDialog isOpen={loadDialogOpen} onClose={() => setLoadDialogOpen(false)} />
      <BurnDialog isOpen={burnDialogOpen} onClose={() => setBurnDialogOpen(false)} connected={status.state === "Connected"} />
      <FirmwareUpdateDialog
        isOpen={firmwareUpdateDialogOpen}
        onClose={() => setFirmwareUpdateDialogOpen(false)}
        isConnected={status.state === "Connected"}
        iniCapabilities={iniCapabilities}
      />
      <NewTuneDialog isOpen={newTuneDialogOpen} onClose={() => setNewTuneDialogOpen(false)} />
      <SettingsDialog
        isOpen={settingsDialogOpen}
        onClose={() => setSettingsDialogOpen(false)}
        theme={theme}
        onThemeChange={(t) => setTheme(t as ThemeName)}
        currentProject={currentProject}
        onSettingsChange={(settings) => {
          if (settings.units) setUnitsSystem(settings.units as 'metric' | 'imperial');
          if (settings.autoBurnOnClose !== undefined) setAutoBurnOnClose(settings.autoBurnOnClose);
          if (settings.demoMode !== undefined) setStatus(s => ({ ...s, demo_mode: settings.demoMode }));
          if (settings.statusBarChannels !== undefined) setStatusBarChannels(settings.statusBarChannels);
          if (settings.runtimePacketMode) setDefaultRuntimePacketMode(settings.runtimePacketMode);
        }}
      />
      {mathChannelsDialogOpen && (
        <MathChannelsDialog onClose={() => setMathChannelsDialogOpen(false)} />
      )}
      <AboutDialog isOpen={aboutDialogOpen} onClose={() => setAboutDialogOpen(false)} />
      <ConnectionDialog
        isOpen={connectionDialogOpen}
        onClose={() => setConnectionDialogOpen(false)}
        ports={ports}
        selectedPort={selectedPort}
        baudRate={baudRate}
        timeoutMs={timeoutMs}
        connectionType={connectionType}
        onConnectionTypeChange={setConnectionType}
        tcpHost={tcpHost}
        onTcpHostChange={setTcpHost}
        tcpPort={tcpPort}
        onTcpPortChange={setTcpPort}
        connected={status.state === "Connected"}
        connecting={connecting || syncing}
        onPortChange={setSelectedPort}
        onBaudChange={handleBaudChange}
        onTimeoutChange={handleTimeoutChange}
        onConnect={connect}
        onDisconnect={disconnect}
        onRefreshPorts={refreshPorts}
        statusMessage={syncing && syncProgress ? `Syncing ECU data... ${syncProgress.percent}%` : undefined}
        iniDefaults={iniDefaults ?? undefined}
        onApplyIniDefaults={applyIniDefaults}
        runtimePacketMode={connectionRuntimePacketMode}
        onRuntimePacketModeChange={setConnectionRuntimePacketMode}
      />
      <NewProjectDialog
        isOpen={newProjectDialogOpen}
        onClose={() => setNewProjectDialogOpen(false)}
        inis={repositoryInis}
        onIniImported={(entry) => setRepositoryInis((prev) => [...prev, entry])}
        onCreateProject={createProject}
        onImportTune={handleImportTuneIntoProject}
        onGenerateBaseMap={() => setBaseMapDialogOpen(true)}
      />
      <BaseMapDialog
        isOpen={baseMapDialogOpen}
        onClose={() => setBaseMapDialogOpen(false)}
        onApply={handleBaseMapApply}
        hasProject={!!currentProject}
      />
      <TuneComparisonDialog
        isOpen={tuneComparisonOpen}
        onClose={() => setTuneComparisonOpen(false)}
        onUseProjectTune={async () => { await checkStatus(); }}
        onUseEcuTune={async () => { await checkStatus(); }}
      />
      <TableComparisonDialog isOpen={tableComparisonOpen} onClose={() => setTableComparisonOpen(false)} />
      <TuneFileDiffDialog isOpen={tuneFileDiffOpen} onClose={() => setTuneFileDiffOpen(false)} />
      <DynoOverlay isOpen={dynoOverlayOpen} onClose={() => setDynoOverlayOpen(false)} />
      <PerformanceFieldsDialog isOpen={performanceDialogOpen} onClose={() => setPerformanceDialogOpen(false)} />
      <SignatureMismatchDialog
        isOpen={signatureMismatchOpen}
        mismatchInfo={signatureMismatchInfo}
        onClose={() => {
          setSignatureMismatchOpen(false);
          setSignatureMismatchInfo(null);
        }}
        onSelectIni={async (path) => {
          console.log("Selected INI:", path);
          setSignatureMismatchOpen(false);
          setSignatureMismatchInfo(null);
          const values = await fetchConstants();
          fetchMenuTree(values);
          await doSync();
        }}
        onContinue={async () => {
          console.log("Continuing with mismatched INI - syncing anyway");
          setSignatureMismatchOpen(false);
          await doSync();
        }}
      />
      {helpTopic && (
        <HelpViewer
          topic={helpTopic}
          onClose={() => setHelpTopic(null)}
          onOpenManual={() => {
            setHelpTopic(null);
            setUserManualOpen(true);
          }}
        />
      )}
      {userManualOpen && (
        <UserManualViewer
          section={userManualSection}
          onClose={() => {
            setUserManualOpen(false);
            setUserManualSection(undefined);
          }}
        />
      )}
      <TuneMismatchDialog
        isOpen={tuneMismatchOpen}
        mismatchInfo={tuneMismatchInfo}
        onClose={() => {
          setTuneMismatchOpen(false);
          setTuneMismatchInfo(null);
        }}
        onUseProject={async () => {
          const values = await fetchConstants();
          await fetchMenuTree(values);
        }}
        onUseECU={async () => {
          const values = await fetchConstants();
          await fetchMenuTree(values);
        }}
      />
      <ErrorDetailsDialog
        isOpen={errorDialogOpen}
        onClose={hideError}
        title={errorInfo.title}
        message={errorInfo.message}
        details={errorInfo.details}
      />
      <RestorePointsDialog
        isOpen={restorePointsOpen}
        onClose={() => setRestorePointsOpen(false)}
        tuneModified={currentProject?.tune_modified || false}
        onRestorePointLoaded={async () => {
          const values = await fetchConstants();
          await fetchMenuTree(values);
          showToast("Restore point loaded successfully", "success");
        }}
      />
      <TuneHistoryPanel isOpen={tuneHistoryOpen} onClose={() => setTuneHistoryOpen(false)} />
      <ImportProjectWizard
        isOpen={importProjectOpen}
        onClose={() => setImportProjectOpen(false)}
        onImportComplete={async (projectPath) => {
          showToast("Project imported successfully", "success");
          const projects = await invoke<ProjectInfo[]>("list_projects");
          setAvailableProjects(projects);
          try {
            const project = await invoke<CurrentProject>("open_project", { path: projectPath });
            setCurrentProject(project);
            const values = await fetchConstants();
            await fetchMenuTree(values);
            setTabs([{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }]);
            setTabContents({ dashboard: { type: "dashboard" } });
            setActiveTabId("dashboard");
          } catch (e) {
            console.error("Failed to open imported project:", e);
            showToast("Project imported but failed to open: " + e, "error");
          }
        }}
      />
      <MigrationReportDialog
        isOpen={migrationReportOpen}
        onClose={() => setMigrationReportOpen(false)}
        onProceed={() => {
          console.log("User proceeding with migration");
        }}
      />
      <OnboardingDialog
        isOpen={onboardingOpen}
        onClose={() => setOnboardingOpen(false)}
        onComplete={async () => {
          try {
            await invoke("mark_onboarding_completed");
          } catch (e) {
            console.error("Failed to mark onboarding as completed:", e);
          }
          setOnboardingOpen(false);
        }}
      />
      {pluginPanelOpen && (
        <div className="dialog-overlay" onClick={() => setPluginPanelOpen(false)}>
          <div
            className="dialog-content plugin-dialog"
            onClick={(e) => e.stopPropagation()}
            style={{ width: '900px', maxWidth: '95vw', height: '600px', maxHeight: '85vh' }}
          >
            <PluginPanel isConnected={status.state === "Connected"} />
          </div>
        </div>
      )}
      <ControllerCommandDialog />
    </>
  );
}

// Re-export ConnectResult so callers don't need a second import path.
export type { ConnectResult };
