import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { AlertTriangle, Cpu } from 'lucide-react';
import { Dialog, Button } from '../common';
import type { IniCapabilities } from '../../types/app';
import { requestReconnect } from '../../utils/connectionWorkflow';
import './FirmwareUpdateDialog.css';

interface FirmwareFlasherInfo {
  stm32_programmer_cli: string | null;
  dfu_util: string | null;
  bootcommander: string | null;
  objcopy: string | null;
}

interface FirmwareCompanionSuggestion {
  companion_path: string | null;
  companion_kind: string;
  message: string;
}

interface FirmwareUpdateGuidance {
  recommended_method: string;
  file_kind: string;
  risk_level: string;
  warnings: string[];
  requires_risk_acknowledgement: boolean;
  suggested_file_hint: string;
  openblt_available: boolean;
  dfu_available: boolean;
}

interface FirmwareUpdateResult {
  success: boolean;
  log: string[];
  message: string;
  should_reconnect: boolean;
}

export interface FirmwareUpdateDialogProps {
  isOpen: boolean;
  onClose: () => void;
  isConnected: boolean;
  iniCapabilities: IniCapabilities | null;
}

type UpdateMethod = 'dfu' | 'openblt';
type DialogMode = 'update' | 'recovery';

export function FirmwareUpdateDialog({
  isOpen,
  onClose,
  isConnected,
  iniCapabilities,
}: FirmwareUpdateDialogProps) {
  const [mode, setMode] = useState<DialogMode>('update');
  const [firmwarePath, setFirmwarePath] = useState<string | null>(null);
  const [bootloaderPath, setBootloaderPath] = useState<string | null>(null);
  const [binFlashAddress, setBinFlashAddress] = useState('0x08008000');
  const [fullErase, setFullErase] = useState(true);
  const [method, setMethod] = useState<UpdateMethod>('openblt');
  const [flasherInfo, setFlasherInfo] = useState<FirmwareFlasherInfo | null>(null);
  const [guidance, setGuidance] = useState<FirmwareUpdateGuidance | null>(null);
  const [companion, setCompanion] = useState<FirmwareCompanionSuggestion | null>(null);
  const [acknowledgeRisk, setAcknowledgeRisk] = useState(false);
  const [log, setLog] = useState<string[]>([]);
  const [isUpdating, setIsUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resultMessage, setResultMessage] = useState<string | null>(null);
  const [shouldReconnect, setShouldReconnect] = useState(false);

  const dfuAvailable = !!iniCapabilities?.dfu_command_name;
  const openbltAvailable = !!iniCapabilities?.openblt_command_name;

  useEffect(() => {
    if (!isOpen) return;
    setLog([]);
    setError(null);
    setResultMessage(null);
    setShouldReconnect(false);
    setAcknowledgeRisk(false);
    setCompanion(null);
    invoke<FirmwareFlasherInfo>('get_firmware_flasher_info')
      .then(setFlasherInfo)
      .catch((e) => setError(String(e)));
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    if (dfuAvailable) {
      setMethod('dfu');
    }
  }, [isOpen, dfuAvailable, openbltAvailable]);

  useEffect(() => {
    if (!isOpen || mode !== 'update') {
      setGuidance(null);
      return;
    }
    invoke<FirmwareUpdateGuidance>('get_firmware_update_guidance', {
      firmwarePath: firmwarePath ?? null,
      method,
    })
      .then(setGuidance)
      .catch(() => setGuidance(null));
  }, [isOpen, mode, firmwarePath, method]);

  useEffect(() => {
    if (!isOpen || !firmwarePath) {
      setCompanion(null);
      return;
    }
    invoke<FirmwareCompanionSuggestion>('suggest_firmware_companion', {
      firmwarePath,
    })
      .then(setCompanion)
      .catch(() => setCompanion(null));
  }, [isOpen, firmwarePath]);

  useEffect(() => {
    setAcknowledgeRisk(false);
  }, [firmwarePath, method]);

  useEffect(() => {
    if (!isOpen) return undefined;
    const unlisten = listen<{ line: string }>('firmware-update:log', (event) => {
      setLog((prev) => [...prev, event.payload.line]);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [isOpen]);

  const browseFirmware = useCallback(async () => {
    const filters =
      method === 'dfu'
        ? [
            { name: 'Recommended', extensions: ['hex', 'dfu'] },
            { name: 'Raw binary (advanced)', extensions: ['bin'] },
            { name: 'All Files', extensions: ['*'] },
          ]
        : [
            { name: 'Recommended (rusEFI Console)', extensions: ['bin'] },
            { name: 'Other formats', extensions: ['hex', 'srec', 's19'] },
            { name: 'All Files', extensions: ['*'] },
          ];
    const selected = await open({
      title: 'Select Firmware File',
      multiple: false,
      filters,
    });
    if (selected && typeof selected === 'string') {
      setFirmwarePath(selected);
      setError(null);
    }
  }, [method]);

  const browseBootloader = useCallback(async () => {
    const selected = await open({
      title: 'Select OpenBLT Bootloader',
      multiple: false,
      filters: [
        { name: 'Bootloader Images', extensions: ['bin', 'hex'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    });
    if (selected && typeof selected === 'string') {
      setBootloaderPath(selected);
      setError(null);
    }
  }, []);

  const browseRecoveryApp = useCallback(async () => {
    const selected = await open({
      title: 'Select Application Firmware',
      multiple: false,
      filters: [
        { name: 'Application Images', extensions: ['bin', 'hex'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    });
    if (selected && typeof selected === 'string') {
      setFirmwarePath(selected);
      setError(null);
    }
  }, []);

  const isBinFirmware = firmwarePath?.toLowerCase().endsWith('.bin') ?? false;
  const riskAckRequired = guidance?.requires_risk_acknowledgement ?? false;
  const riskAckSatisfied = !riskAckRequired || acknowledgeRisk;
  const openbltNeedsObjcopy = method === 'openblt' && isBinFirmware && !flasherInfo?.objcopy;

  const canFlash =
    mode === 'recovery'
      ? bootloaderPath &&
        firmwarePath &&
        binFlashAddress.trim().length > 0 &&
        !!flasherInfo?.stm32_programmer_cli
      : isConnected &&
        firmwarePath &&
        riskAckSatisfied &&
        !openbltNeedsObjcopy &&
        (method !== 'dfu' || !isBinFirmware || binFlashAddress.trim().length > 0) &&
        ((method === 'dfu' &&
          dfuAvailable &&
          (flasherInfo?.stm32_programmer_cli || flasherInfo?.dfu_util)) ||
          (method === 'openblt' && openbltAvailable && flasherInfo?.bootcommander));

  const handleUpdate = useCallback(async () => {
    if (!firmwarePath) return;
    setIsUpdating(true);
    setError(null);
    setResultMessage(null);
    setShouldReconnect(false);
    setLog([]);
    try {
      const result =
        mode === 'recovery'
          ? await invoke<FirmwareUpdateResult>('recover_ecu_firmware_dfu', {
              bootloaderPath,
              appFirmwarePath: firmwarePath,
              appFlashAddress: binFlashAddress,
              fullErase,
            })
          : await invoke<FirmwareUpdateResult>('update_ecu_firmware', {
              firmwarePath,
              method,
              binFlashAddress: isBinFirmware ? binFlashAddress : null,
              acknowledgeRisk,
            });
      setLog(result.log);
      setResultMessage(result.message);
      setShouldReconnect(result.should_reconnect);
      if (!result.success) {
        setError(result.message);
      } else if (result.should_reconnect) {
        requestReconnect({
          source: 'firmware-update',
          delayMs: 8000,
          retries: 10,
        });
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsUpdating(false);
    }
  }, [
    firmwarePath,
    mode,
    bootloaderPath,
    binFlashAddress,
    fullErase,
    method,
    isBinFirmware,
    acknowledgeRisk,
  ]);

  const missingFlasher =
    mode === 'recovery'
      ? !flasherInfo?.stm32_programmer_cli
      : method === 'dfu'
        ? !flasherInfo?.stm32_programmer_cli && !flasherInfo?.dfu_util
        : !flasherInfo?.bootcommander;

  const showMethodHint =
    guidance &&
    guidance.recommended_method !== method &&
    mode === 'update' &&
    firmwarePath;

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="Update ECU Firmware"
      size="lg"
      closeOnBackdrop={!isUpdating}
      className="firmware-update-dialog"
    >
      <Dialog.Body>
        <div className="firmware-update-intro">
          <Cpu size={18} aria-hidden />
          <p>
            {mode === 'recovery'
              ? 'Recover an ECU that no longer boots after a DFU flash. Put the board in DFU mode manually (PROG button + power cycle) — no tuning connection required.'
              : 'Flash new firmware to your ECU. The tuning connection will drop while the ECU reboots into bootloader mode.'}
          </p>
        </div>

        <div className="firmware-update-field">
          <label>Mode</label>
          <div className="firmware-update-methods">
            <label className="firmware-method-option">
              <input
                type="radio"
                name="fw-mode"
                value="update"
                checked={mode === 'update'}
                onChange={() => setMode('update')}
                disabled={isUpdating}
              />
              <span>
                <strong>Normal update</strong> — ECU is running and connected
              </span>
            </label>
            <label className="firmware-method-option">
              <input
                type="radio"
                name="fw-mode"
                value="recovery"
                checked={mode === 'recovery'}
                onChange={() => setMode('recovery')}
                disabled={isUpdating}
              />
              <span>
                <strong>DFU recovery</strong> — ECU won&apos;t boot (re-flash OpenBLT + app)
              </span>
            </label>
          </div>
        </div>

        {mode === 'recovery' && (
          <>
            <div className="firmware-update-warning">
              Use <code>rusefi.hex</code> or <code>rusefi.bin</code> from{' '}
              <code>firmware/build/</code>. Recovery re-flashes the OpenBLT bootloader at{' '}
              <code>0x08000000</code> and the application at <code>0x08008000</code>.
            </div>

            <div className="firmware-update-field">
              <label>OpenBLT bootloader</label>
              <div className="firmware-file-row">
                <code className="firmware-file-path">
                  {bootloaderPath ?? 'No file selected'}
                </code>
                <Button
                  variant="secondary"
                  onClick={() => void browseBootloader()}
                  disabled={isUpdating}
                >
                  Browse…
                </Button>
              </div>
              <p className="firmware-flasher-hint">
                From your firmware build:{' '}
                <code>firmware/bootloader/blbuild/openblt_&lt;board&gt;.bin</code>
              </p>
            </div>

            <div className="firmware-update-field">
              <label>Application firmware</label>
              <div className="firmware-file-row">
                <code className="firmware-file-path">
                  {firmwarePath ?? 'No file selected'}
                </code>
                <Button
                  variant="secondary"
                  onClick={() => void browseRecoveryApp()}
                  disabled={isUpdating}
                >
                  Browse…
                </Button>
              </div>
              <p className="firmware-flasher-hint">
                Recommended: <code>firmware/build/rusefi.hex</code>
              </p>
            </div>

            <div className="firmware-update-field">
              <label htmlFor="recovery-app-address">Application flash address</label>
              <input
                id="recovery-app-address"
                className="firmware-address-input"
                value={binFlashAddress}
                onChange={(e) => setBinFlashAddress(e.target.value)}
                disabled={isUpdating}
                spellCheck={false}
                placeholder="0x08008000"
              />
            </div>

            <label className="firmware-checkbox-option">
              <input
                type="checkbox"
                checked={fullErase}
                onChange={(e) => setFullErase(e.target.checked)}
                disabled={isUpdating}
              />
              <span>
                Full chip erase before flash (recommended for STM32F7 — required after a bad
                update)
              </span>
            </label>

            <div className="firmware-update-field">
              <label>Detected flash tools</label>
              <ul className="firmware-flasher-list">
                <li className={flasherInfo?.stm32_programmer_cli ? 'ok' : 'missing'}>
                  STM32CubeProgrammer CLI: {flasherInfo?.stm32_programmer_cli ?? 'not found'}
                </li>
              </ul>
            </div>
          </>
        )}

        {mode === 'update' && (
          <>
            {!dfuAvailable && !openbltAvailable && (
              <div className="firmware-update-warning">
                This ECU definition has no firmware update commands (<code>cmd_dfu</code> /{' '}
                <code>cmd_openblt</code>).
              </div>
            )}

            {(dfuAvailable || openbltAvailable) && (
              <>
                <div className="firmware-update-warning caution">
                  <AlertTriangle size={16} aria-hidden />
                  <span>
                    <strong>Serial firmware update is disabled.</strong> Use rusEFI Console with{' '}
                    <code>firmware/build/rusefi.bin</code>. Below is DFU recovery only (ECU must be
                    in STM32 bootloader mode).
                  </span>
                </div>

                <div className="firmware-update-field">
                  <label>Firmware file</label>
                  <div className="firmware-file-row">
                    <code className="firmware-file-path">
                      {firmwarePath ?? 'No file selected'}
                    </code>
                    <Button
                      variant="secondary"
                      onClick={() => void browseFirmware()}
                      disabled={isUpdating}
                    >
                      Browse…
                    </Button>
                  </div>
                  <p className="firmware-flasher-hint">
                    From your build folder:{' '}
                    <code>{guidance?.suggested_file_hint ?? 'firmware/build/rusefi.bin'}</code>
                  </p>
                </div>

                {companion?.message && (
                  <div className="firmware-update-warning note">
                    <p>{companion.message}</p>
                  </div>
                )}

                {guidance && guidance.warnings.length > 0 && (
                  <div
                    className={`firmware-update-warning caution firmware-risk-${guidance.risk_level}`}
                  >
                    <AlertTriangle size={16} aria-hidden />
                    <div>
                      <strong className="firmware-risk-label">
                        {guidance.risk_level === 'high'
                          ? 'High risk'
                          : guidance.risk_level === 'medium'
                            ? 'Caution'
                            : 'Note'}
                      </strong>
                      <ul className="firmware-guidance-warnings">
                        {guidance.warnings.map((warning) => (
                          <li key={warning}>{warning}</li>
                        ))}
                      </ul>
                    </div>
                  </div>
                )}

                {showMethodHint && (
                  <div className="firmware-update-field">
                    <Button
                      variant="secondary"
                      onClick={() => setMethod(guidance!.recommended_method as UpdateMethod)}
                      disabled={isUpdating}
                    >
                      Switch to recommended method (
                      {guidance!.recommended_method === 'openblt'
                        ? 'Serial / rusefi.bin'
                        : 'DFU / rusefi.hex'}
                      )
                    </Button>
                  </div>
                )}

                {method === 'dfu' && isBinFirmware && (
                  <div className="firmware-update-field">
                    <label htmlFor="bin-flash-address">Binary flash address</label>
                    <input
                      id="bin-flash-address"
                      className="firmware-address-input"
                      value={binFlashAddress}
                      onChange={(e) => setBinFlashAddress(e.target.value)}
                      disabled={isUpdating}
                      spellCheck={false}
                      placeholder="0x08008000"
                    />
                    <p className="firmware-flasher-hint">
                      Only needed for raw .bin — prefer <code>rusefi.hex</code> instead.
                    </p>
                  </div>
                )}

                {riskAckRequired && (
                  <label className="firmware-checkbox-option firmware-risk-ack">
                    <input
                      type="checkbox"
                      checked={acknowledgeRisk}
                      onChange={(e) => setAcknowledgeRisk(e.target.checked)}
                      disabled={isUpdating}
                    />
                    <span>
                      I understand this update is high risk and may brick the ECU if the
                      bootloader is missing or corrupt.
                    </span>
                  </label>
                )}

                <div className="firmware-update-field">
                  <label>Detected flash tools</label>
                  <ul className="firmware-flasher-list">
                    {method === 'dfu' ? (
                      <>
                        <li className={flasherInfo?.stm32_programmer_cli ? 'ok' : 'missing'}>
                          STM32CubeProgrammer CLI:{' '}
                          {flasherInfo?.stm32_programmer_cli ?? 'not found'}
                        </li>
                        <li className={flasherInfo?.dfu_util ? 'ok' : 'missing'}>
                          dfu-util: {flasherInfo?.dfu_util ?? 'not found'}
                        </li>
                      </>
                    ) : (
                      <>
                        <li className={flasherInfo?.bootcommander ? 'ok' : 'missing'}>
                          BootCommander: {flasherInfo?.bootcommander ?? 'not found'}
                        </li>
                        <li className={flasherInfo?.objcopy ? 'ok' : 'missing'}>
                          arm-none-eabi-objcopy: {flasherInfo?.objcopy ?? 'not found'}
                        </li>
                      </>
                    )}
                  </ul>
                  {missingFlasher && (
                    <p className="firmware-flasher-hint">
                      {method === 'dfu' ? (
                        <>
                          Install{' '}
                          <a
                            href="https://www.st.com/en/development-tools/stm32cubeprog.html"
                            target="_blank"
                            rel="noreferrer"
                          >
                            STM32CubeProgrammer
                          </a>{' '}
                          for DFU recovery with <code>rusefi.hex</code>.
                        </>
                      ) : (
                        <>
                          Install{' '}
                          <a
                            href="https://www.feaser.com/openblt/doku.php?id=manual:bootcommander"
                            target="_blank"
                            rel="noreferrer"
                          >
                            OpenBLT BootCommander
                          </a>{' '}
                          for serial updates. <code>arm-none-eabi-objcopy</code> (from your rusEFI
                          build toolchain) converts <code>rusefi.bin</code> automatically.
                        </>
                      )}
                    </p>
                  )}
                  {openbltNeedsObjcopy && (
                    <p className="firmware-flasher-hint">
                      Add your ARM GCC toolchain to PATH — the same one used to compile rusEFI
                      (contains <code>arm-none-eabi-objcopy</code>).
                    </p>
                  )}
                </div>

                {!isConnected && (
                  <div className="firmware-update-warning">
                    Connect to the ECU before starting a firmware update.
                  </div>
                )}

                <div className="firmware-update-warning caution">
                  <AlertTriangle size={16} aria-hidden />
                  <span>
                    Do not disconnect USB power during flashing. After DFU updates, power-cycle
                    the ECU before reconnecting.
                  </span>
                </div>
              </>
            )}
          </>
        )}

        {missingFlasher && mode === 'recovery' && (
          <p className="firmware-flasher-hint">
            Install STM32CubeProgrammer and ensure STM32_Programmer_CLI is available.
          </p>
        )}

        {error && <div className="firmware-update-error">{error}</div>}
        {resultMessage && !error && (
          <div className="firmware-update-success">
            {resultMessage}
            {shouldReconnect && (
              <p className="firmware-reconnect-hint">
                Reconnecting automatically… If the ECU does not come back, click Reconnect below.
              </p>
            )}
          </div>
        )}

        {log.length > 0 && (
          <div className="firmware-update-log">
            {log.map((line, idx) => (
              <div key={`${idx}-${line}`} className="firmware-update-log-line">
                {line}
              </div>
            ))}
          </div>
        )}
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose} disabled={isUpdating}>
          Close
        </Button>
        {shouldReconnect && !isUpdating && (
          <Button
            variant="secondary"
            onClick={() =>
              requestReconnect({
                source: 'firmware-update-manual',
                delayMs: 0,
                retries: 6,
              })
            }
          >
            Reconnect
          </Button>
        )}
        <Button
          variant="primary"
          onClick={() => void handleUpdate()}
          disabled={!canFlash || isUpdating}
        >
          {isUpdating
            ? mode === 'recovery'
              ? 'Recovering…'
              : 'Updating…'
            : mode === 'recovery'
              ? 'Recover ECU'
              : 'Update Firmware'}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
