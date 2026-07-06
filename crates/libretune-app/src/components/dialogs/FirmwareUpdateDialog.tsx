import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { AlertTriangle, Cpu } from 'lucide-react';
import { Dialog, Button } from '../common';
import type { IniCapabilities } from '../../types/app';
import './FirmwareUpdateDialog.css';

interface FirmwareFlasherInfo {
  stm32_programmer_cli: string | null;
  dfu_util: string | null;
  bootcommander: string | null;
}

interface FirmwareUpdateResult {
  success: boolean;
  log: string[];
  message: string;
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
  const [log, setLog] = useState<string[]>([]);
  const [isUpdating, setIsUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resultMessage, setResultMessage] = useState<string | null>(null);

  const dfuAvailable = !!iniCapabilities?.dfu_command_name;
  const openbltAvailable = !!iniCapabilities?.openblt_command_name;

  useEffect(() => {
    if (!isOpen) return;
    setLog([]);
    setError(null);
    setResultMessage(null);
    invoke<FirmwareFlasherInfo>('get_firmware_flasher_info')
      .then(setFlasherInfo)
      .catch((e) => setError(String(e)));
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    if (openbltAvailable) {
      setMethod('openblt');
    } else if (dfuAvailable) {
      setMethod('dfu');
    }
  }, [isOpen, dfuAvailable, openbltAvailable]);

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
    const extensions =
      method === 'dfu'
        ? ['dfu', 'hex', 'bin', 'srec', 's19']
        : ['srec', 's19', 'hex'];
    const selected = await open({
      title: 'Select Firmware File',
      multiple: false,
      filters: [
        { name: 'Firmware Files', extensions },
        { name: 'All Files', extensions: ['*'] },
      ],
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

  const canFlash =
    mode === 'recovery'
      ? bootloaderPath &&
        firmwarePath &&
        binFlashAddress.trim().length > 0 &&
        !!flasherInfo?.stm32_programmer_cli
      : isConnected &&
        firmwarePath &&
        (!isBinFirmware || binFlashAddress.trim().length > 0) &&
        ((method === 'dfu' &&
          dfuAvailable &&
          (flasherInfo?.stm32_programmer_cli || flasherInfo?.dfu_util)) ||
          (method === 'openblt' && openbltAvailable && flasherInfo?.bootcommander));

  const handleUpdate = useCallback(async () => {
    if (!firmwarePath) return;
    setIsUpdating(true);
    setError(null);
    setResultMessage(null);
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
            });
      setLog(result.log);
      setResultMessage(result.message);
      if (!result.success) {
        setError(result.message);
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
  ]);

  const missingFlasher =
    mode === 'recovery'
      ? !flasherInfo?.stm32_programmer_cli
      : method === 'dfu'
        ? !flasherInfo?.stm32_programmer_cli && !flasherInfo?.dfu_util
        : !flasherInfo?.bootcommander;

  const showBinDfuWarning = mode === 'update' && method === 'dfu' && isBinFirmware;

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
              Flashing only <code>rusefi.bin</code> via DFU writes the application at{' '}
              <code>0x08008000</code> but leaves the OpenBLT bootloader at{' '}
              <code>0x08000000</code> untouched — unless a full erase wiped it. Recovery
              re-flashes both regions.
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
                <code>firmware/bootloader/blbuild/openblt_&lt;board&gt;.bin</code> (build the
                bootloader target first). Flashed at <code>0x08000000</code>.
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
                <div className="firmware-update-field">
                  <label>Update method</label>
                  <div className="firmware-update-methods">
                    {openbltAvailable && (
                      <label className="firmware-method-option">
                        <input
                          type="radio"
                          name="fw-method"
                          value="openblt"
                          checked={method === 'openblt'}
                          onChange={() => setMethod('openblt')}
                          disabled={isUpdating}
                        />
                        <span>
                          <strong>OpenBLT</strong> — recommended (.srec from{' '}
                          <code>deliver/</code>, via serial)
                        </span>
                      </label>
                    )}
                    {dfuAvailable && (
                      <label className="firmware-method-option">
                        <input
                          type="radio"
                          name="fw-method"
                          value="dfu"
                          checked={method === 'dfu'}
                          onChange={() => setMethod('dfu')}
                          disabled={isUpdating}
                        />
                        <span>
                          <strong>DFU</strong> — emergency only (.dfu from{' '}
                          <code>deliver/</code> preferred)
                        </span>
                      </label>
                    )}
                  </div>
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
                  {method === 'openblt' && (
                    <p className="firmware-flasher-hint">
                      Use <code>rusefi_*_update.srec</code> from your firmware{' '}
                      <code>deliver/</code> folder — it includes CRC and the correct layout for
                      OpenBLT.
                    </p>
                  )}
                  {method === 'dfu' && (
                    <p className="firmware-flasher-hint">
                      Prefer the packaged <code>.dfu</code> from <code>deliver/</code>. Do not
                      use raw <code>build/rusefi.bin</code> unless you know the ECU still has a
                      valid OpenBLT bootloader.
                    </p>
                  )}
                </div>

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
                  </div>
                )}

                {showBinDfuWarning && (
                  <div className="firmware-update-warning caution">
                    <AlertTriangle size={16} aria-hidden />
                    <span>
                      Raw <code>.bin</code> via DFU only updates the application region. If the
                      OpenBLT bootloader at <code>0x08000000</code> is missing or corrupt, the ECU
                      will not boot. Switch to <strong>DFU recovery</strong> mode or use OpenBLT
                      + <code>*_update.srec</code> instead.
                    </span>
                  </div>
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
                      <li className={flasherInfo?.bootcommander ? 'ok' : 'missing'}>
                        BootCommander: {flasherInfo?.bootcommander ?? 'not found'}
                      </li>
                    )}
                  </ul>
                  {missingFlasher && (
                    <p className="firmware-flasher-hint">
                      Install{' '}
                      {method === 'dfu'
                        ? 'STM32CubeProgrammer (recommended on Windows) or dfu-util'
                        : 'OpenBLT BootCommander'}{' '}
                      and ensure it is on your PATH.
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
          <div className="firmware-update-success">{resultMessage}</div>
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
