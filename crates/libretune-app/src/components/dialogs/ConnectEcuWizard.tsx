import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Dialog, Button } from "../common";
import type { IniEntry } from "../../types/app";
import {
  WizardTransport,
  WizardStep,
  wizardSteps,
  nextStep,
  prevStep,
  isLastStep,
  transportLabel,
  stepTitle,
  isSerialTransport,
  paramsComplete,
  bestLocalMatch,
  deriveOnlineIniUrl,
  deriveSpeeduinoIniUrl,
  sanitizeSignature,
  WIZARD_BAUD_RATES,
  type WizardIniMatch,
} from "../../utils/connectEcuWizard";

interface ConnectResult {
  signature: string;
}
interface OnlineIniEntry {
  source: string;
  name: string;
  signature: string | null;
  download_url: string;
  repo_path: string;
  size: number | null;
}
/** The INI the wizard has resolved for the ECU (from a local, online or manual source). */
interface ResolvedIni {
  /** Repository ID, usable directly with `create_project`. */
  id: string;
  path: string;
  name: string;
  source: string;
}

interface ConnectEcuWizardProps {
  isOpen: boolean;
  onClose: () => void;
  /** Locally installed INI definitions, offered as a picker on the offline path. */
  inis: IniEntry[];
  /** Mirrors New Project's creation flow (close prior project, load menus/tabs, toast). */
  onCreateProject: (name: string, iniId: string) => Promise<boolean>;
  /** Connects using the params this wizard already collected, reusing the
   * app's normal connect+sync flow (signature-mismatch handling, automatic
   * tune read on a match) instead of leaving the project merely created. */
  onConnect: (params: {
    port: string;
    baud: number;
    connectionType: "Serial" | "Tcp";
    tcpHost: string;
    tcpPort: number;
  }) => Promise<void>;
}

/**
 * Connect-ECU wizard.
 *
 * Guided flow: transport → connection params → connect & read signature →
 * resolve the INI definition (auto local match → online search/download →
 * manual upload) → name the project, then create it. Reuses existing backend
 * commands (`get_serial_ports`, `connect_to_ecu`, `find_matching_inis`,
 * `search_online_inis` / `download_ini`, `import_ini`, `create_project`). The
 * offline path skips straight to naming, where the user picks an installed INI
 * by hand (same flow as New Project) and Finish creates the project.
 */
export default function ConnectEcuWizard({ isOpen, onClose, inis, onCreateProject, onConnect }: ConnectEcuWizardProps) {
  const [transport, setTransport] = useState<WizardTransport | null>(null);
  const [step, setStep] = useState<WizardStep>("transport");
  const [projectName, setProjectName] = useState("");

  // Connection parameters (Phase 2).
  const [ports, setPorts] = useState<string[]>([]);
  const [scanningPorts, setScanningPorts] = useState(false);
  const [port, setPort] = useState("");
  const [baud, setBaud] = useState(115200);
  const [host, setHost] = useState("");
  const [tcpPort, setTcpPort] = useState(29000);

  const params = { port, baud, host, tcpPort };

  async function scanPorts() {
    setScanningPorts(true);
    try {
      const found = await invoke<string[]>("get_serial_ports");
      setPorts(found);
      if (found.length > 0 && !found.includes(port)) setPort(found[0]);
    } catch {
      setPorts([]);
    } finally {
      setScanningPorts(false);
    }
  }

  // Connect + detect (Phase 3).
  const [connecting, setConnecting] = useState(false);
  const [signature, setSignature] = useState<string | null>(null);
  const [connectError, setConnectError] = useState<string | null>(null);

  // Resolve INI (Phase 3).
  const [resolving, setResolving] = useState(false);
  const [localMatches, setLocalMatches] = useState<WizardIniMatch[]>([]);
  const [onlineResults, setOnlineResults] = useState<OnlineIniEntry[]>([]);
  const [derived, setDerived] = useState<{
    url: string;
    status: "downloading" | "ok" | "failed";
    error?: string;
  } | null>(null);
  const [resolvedIni, setResolvedIni] = useState<ResolvedIni | null>(null);
  const [resolveBusy, setResolveBusy] = useState<string | null>(null);

  // Project creation (Phase 4).
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  // Scan serial ports when entering the params step for a serial transport.
  useEffect(() => {
    if (step === "params" && isSerialTransport(transport)) void scanPorts();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, transport]);

  async function connectAndDetect() {
    setConnecting(true);
    setConnectError(null);
    setSignature(null);
    // Release any connection left open by a previous attempt (retry, or a
    // different ECU picked after going Back) before opening a new one — an
    // unclosed serial handle keeps Windows from freeing the COM port, so a
    // disconnected device's port lingers in later scans.
    await invoke("disconnect_ecu").catch(() => {});
    try {
      const result = await invoke<ConnectResult>("connect_to_ecu", {
        connectionType: transport === "wifi" ? "Tcp" : "Serial",
        portName: isSerialTransport(transport) ? port : "",
        baudRate: baud,
        tcpHost: transport === "wifi" ? host : null,
        tcpPort: transport === "wifi" ? tcpPort : null,
      });
      setSignature(sanitizeSignature(result.signature));
    } catch (e) {
      setConnectError(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(false);
      // We only needed the signature, not a live connection — closing it
      // immediately frees the port for the next step (or another device).
      await invoke("disconnect_ecu").catch(() => {});
    }
  }

  // Auto-connect when entering the connect step. A ref guard keyed by the
  // connection params (not state) makes this synchronous, so React
  // StrictMode's double-invoked effect in dev can't fire two concurrent
  // connect_to_ecu calls on the same port — Windows opens a COM port
  // exclusively, so the loser of that race failed with "Access denied" even
  // within a single process. Keying by the params (rather than a plain
  // boolean) still auto-retries when the user goes Back and picks a
  // different port/host, e.g. switching from one ECU to another. The manual
  // Retry button bypasses this guard by calling connectAndDetect() directly.
  const connectTriedForRef = useRef<string | null>(null);
  useEffect(() => {
    const attemptKey = `${transport}|${port}|${baud}|${host}|${tcpPort}`;
    if (step === "connect" && connectTriedForRef.current !== attemptKey) {
      connectTriedForRef.current = attemptKey;
      void connectAndDetect();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, transport, port, baud, host, tcpPort]);

  /** A downloaded/derived .ini is a filesystem path; re-importing it (idempotent
   * by signature) gets the repository ID `create_project` needs. */
  async function resolveIniId(path: string): Promise<string> {
    const entry = await invoke<{ id: string }>("import_ini", { sourcePath: path });
    return entry.id;
  }

  async function resolveIni(sig: string) {
    setResolving(true);
    setLocalMatches([]);
    setOnlineResults([]);
    setDerived(null);
    try {
      // 1) Local definition whose signature= matches (exact > partial).
      const local = await invoke<WizardIniMatch[]>("find_matching_inis", { ecuSignature: sig }).catch(
        () => [],
      );
      setLocalMatches(local);
      const best = bestLocalMatch(local);
      if (best) {
        setResolvedIni({ id: best.id, path: best.path, name: best.name, source: "local" });
        return;
      }

      // 2) Firmwares with a deterministic online definition: derive the URL
      // from the signature and download it directly (rusEFI/FOME get an
      // exact per-build URL; Speeduino has one canonical .ini per release).
      const deterministic: { deriver: (s: string) => string | null; label: string; source: string }[] = [
        { deriver: deriveOnlineIniUrl, label: "rusEFI (auto)", source: "rusefi" },
        { deriver: deriveSpeeduinoIniUrl, label: "Speeduino (auto)", source: "speeduino" },
      ];
      for (const { deriver, label, source } of deterministic) {
        const url = deriver(sig);
        if (!url) continue;
        setDerived({ url, status: "downloading" });
        try {
          const name = url.split("/").slice(-1)[0] || "definition.ini";
          const path = await invoke<string>("download_ini", {
            downloadUrl: url,
            name,
            source,
          });
          const id = await resolveIniId(path);
          setResolvedIni({ id, path, name, source: label });
          setDerived({ url, status: "ok" });
          return;
        } catch (e) {
          setDerived({ url, status: "failed", error: String(e) });
          // fall through to the repo search / manual upload
        }
      }

      // 3) Other firmwares: match against the repo listing.
      const online = await invoke<OnlineIniEntry[]>("search_online_inis", { signature: sig }).catch(
        () => [],
      );
      setOnlineResults(online);
    } finally {
      setResolving(false);
    }
  }

  // Kick off INI resolution when entering the resolve step (needs a signature).
  // A ref guard (not state) makes this synchronous, so React StrictMode's
  // double-invoked effect in dev can't fire two concurrent downloads that race
  // on the same target file.
  const resolveTriedRef = useRef<string | null>(null);
  useEffect(() => {
    if (step === "resolveIni" && signature && resolveTriedRef.current !== signature) {
      resolveTriedRef.current = signature;
      void resolveIni(signature);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, signature]);

  async function downloadOnline(entry: OnlineIniEntry) {
    setResolveBusy(entry.download_url);
    try {
      const path = await invoke<string>("download_ini", {
        downloadUrl: entry.download_url,
        name: entry.name,
        source: entry.source,
      });
      const id = await resolveIniId(path);
      setResolvedIni({ id, path, name: entry.name, source: entry.source });
    } catch {
      /* surfaced via lack of selection */
    } finally {
      setResolveBusy(null);
    }
  }

  async function pickManualIni() {
    const selected = await openFileDialog({
      filters: [{ name: "INI definition", extensions: ["ini"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    setResolveBusy("manual");
    try {
      const entry = await invoke<{ id: string; path: string; name: string }>("import_ini", {
        sourcePath: selected,
      });
      setResolvedIni({ id: entry.id, path: entry.path, name: entry.name, source: "manual" });
    } catch {
      /* ignore */
    } finally {
      setResolveBusy(null);
    }
  }

  const steps = wizardSteps(transport);
  const stepIndex = steps.indexOf(step);
  const last = isLastStep(step, transport);
  const canAdvance =
    step === "transport"
      ? transport !== null
      : step === "params"
        ? paramsComplete(transport, params)
        : step === "connect"
          ? signature !== null
          : step === "resolveIni"
            ? resolvedIni !== null
            : true;

  function reset() {
    setTransport(null);
    setStep("transport");
    setProjectName("");
    setPorts([]);
    setPort("");
    setHost("");
    setConnecting(false);
    setSignature(null);
    setConnectError(null);
    setLocalMatches([]);
    setOnlineResults([]);
    setDerived(null);
    setResolvedIni(null);
    resolveTriedRef.current = null;
    connectTriedForRef.current = null;
    setCreating(false);
    setCreateError(null);
  }
  function handleClose() {
    // Don't leave the port held if the user cancels mid-wizard.
    void invoke("disconnect_ecu").catch(() => {});
    reset();
    onClose();
  }

  /** Create the project from the resolved INI and close the wizard. If no INI
   * was resolved (only possible on the offline path when nothing is selected),
   * Finish just closes without creating anything. */
  async function finishAndCreate() {
    if (!projectName.trim()) return;
    if (!resolvedIni) {
      handleClose();
      return;
    }
    setCreating(true);
    setCreateError(null);
    try {
      const ok = await onCreateProject(projectName.trim(), resolvedIni.id);
      if (ok) {
        // Land the app actually connected (and, on a signature match, with the
        // ECU's current tune already read) instead of just having created the
        // project — reusing the params this wizard already collected so the
        // user isn't asked to pick the port/baud again right after the wizard.
        if (transport && transport !== "offline") {
          await onConnect({
            port: isSerialTransport(transport) ? port : "",
            baud,
            connectionType: transport === "wifi" ? "Tcp" : "Serial",
            tcpHost: host,
            tcpPort,
          }).catch(() => {
            // connect() already surfaces its own failure toast; the project
            // itself was created successfully, so don't block closing on it.
          });
        }
        reset();
        onClose();
      } else {
        setCreateError("Project creation failed — see the notification for details.");
      }
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : String(e));
    } finally {
      setCreating(false);
    }
  }

  const transports: WizardTransport[] = ["usb", "bluetooth", "wifi", "offline"];

  return (
    <Dialog open={isOpen} onClose={handleClose} title="Connect ECU / New Project" size="md">
      <Dialog.Body>
        <div style={{ opacity: 0.7, fontSize: 12, marginBottom: "0.75rem" }}>
          Step {stepIndex + 1} of {steps.length} — {stepTitle(step)}
        </div>

        {step === "transport" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            {transports.map((t) => (
              <label
                key={t}
                style={{ display: "flex", alignItems: "center", gap: "0.5rem", cursor: "pointer" }}
              >
                <input
                  type="radio"
                  name="wizard-transport"
                  checked={transport === t}
                  onChange={() => setTransport(t)}
                />
                <span>{transportLabel(t)}</span>
              </label>
            ))}
          </div>
        )}

        {step === "params" && isSerialTransport(transport) && (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
            {transport === "bluetooth" && (
              <p style={{ opacity: 0.7, fontSize: 12, margin: 0 }}>
                Bluetooth ECUs appear as a serial (COM) port — pair the device in your OS
                first, then pick its port below.
              </p>
            )}
            <div>
              <label style={{ display: "block", marginBottom: "0.25rem" }}>Port</label>
              <div style={{ display: "flex", gap: "0.5rem" }}>
                <select
                  value={port}
                  onChange={(e) => setPort(e.target.value)}
                  style={{ flex: 1 }}
                >
                  {ports.length === 0 ? (
                    <option value="">No ports found</option>
                  ) : (
                    ports.map((p) => (
                      <option key={p} value={p}>
                        {p}
                      </option>
                    ))
                  )}
                </select>
                <Button variant="secondary" onClick={scanPorts} disabled={scanningPorts}>
                  {scanningPorts ? "Scanning…" : "Refresh"}
                </Button>
              </div>
            </div>
            <div>
              <label style={{ display: "block", marginBottom: "0.25rem" }}>Baud rate</label>
              <select value={baud} onChange={(e) => setBaud(parseInt(e.target.value))}>
                {WIZARD_BAUD_RATES.map((b) => (
                  <option key={b} value={b}>
                    {b}
                  </option>
                ))}
              </select>
            </div>
          </div>
        )}

        {step === "params" && transport === "wifi" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
            <p style={{ opacity: 0.7, fontSize: 12, margin: 0 }}>
              For a networked ECU (e.g. rusEFI over WiFi), enter its host/IP and TCP port.
            </p>
            <div>
              <label style={{ display: "block", marginBottom: "0.25rem" }}>Host / IP</label>
              <input
                type="text"
                value={host}
                placeholder="192.168.4.1"
                onChange={(e) => setHost(e.target.value)}
                style={{ width: "100%" }}
              />
            </div>
            <div>
              <label style={{ display: "block", marginBottom: "0.25rem" }}>TCP port</label>
              <input
                type="number"
                value={tcpPort}
                onChange={(e) => setTcpPort(parseInt(e.target.value) || 0)}
              />
            </div>
          </div>
        )}

        {step === "connect" && (
          <div>
            {connecting && <div style={{ opacity: 0.8 }}>Connecting and reading the ECU signature…</div>}
            {signature && !connecting && (
              <div style={{ color: "var(--color-success, #2a2)" }}>
                ✓ ECU detected. Signature:
                <div style={{ fontFamily: "monospace", marginTop: 4, wordBreak: "break-all" }}>{signature}</div>
              </div>
            )}
            {connectError && !connecting && (
              <div>
                <div style={{ color: "var(--color-error, #d33)" }}>Could not connect: {connectError}</div>
                <Button variant="secondary" onClick={connectAndDetect} style={{ marginTop: "0.5rem" }}>
                  Retry
                </Button>
              </div>
            )}
          </div>
        )}

        {step === "resolveIni" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
            {resolving && <div style={{ opacity: 0.8 }}>Looking for a matching definition…</div>}

            {derived && (
              <div style={{ fontSize: 12, opacity: 0.85 }}>
                {derived.status === "downloading" && "Downloading the exact rusEFI/FOME definition for this signature…"}
                {derived.status === "failed" && (
                  <div style={{ color: "var(--color-error, #d33)" }}>
                    <div>Couldn't download the definition for this signature.</div>
                    <div style={{ opacity: 0.8, wordBreak: "break-all" }}>{derived.url}</div>
                    {derived.error && (
                      <div style={{ opacity: 0.8, marginTop: 2 }}>Reason: {derived.error}</div>
                    )}
                    <Button
                      variant="secondary"
                      onClick={() => signature && resolveIni(signature)}
                      disabled={resolving}
                      style={{ marginTop: 4 }}
                    >
                      Retry download
                    </Button>
                  </div>
                )}
              </div>
            )}

            {resolvedIni && (
              <div style={{ color: "var(--color-success, #2a2)" }}>
                ✓ Using <b>{resolvedIni.name}</b> ({resolvedIni.source}).
              </div>
            )}

            {localMatches.length > 0 && (
              <div>
                <div style={{ fontWeight: 600, marginBottom: 4 }}>Local matches</div>
                {localMatches.map((m) => (
                  <div key={m.path} style={{ display: "flex", alignItems: "center", gap: 8, padding: "2px 0" }}>
                    <span style={{ flex: 1 }}>
                      {m.name} <span style={{ opacity: 0.6, fontSize: 12 }}>({m.match_type})</span>
                    </span>
                    <Button variant="secondary" onClick={() => setResolvedIni({ id: m.id, path: m.path, name: m.name, source: "local" })}>
                      Use
                    </Button>
                  </div>
                ))}
              </div>
            )}

            {onlineResults.length > 0 && (
              <div>
                <div style={{ fontWeight: 600, marginBottom: 4 }}>Online matches</div>
                {onlineResults.slice(0, 8).map((e) => (
                  <div key={e.download_url} style={{ display: "flex", alignItems: "center", gap: 8, padding: "2px 0" }}>
                    <span style={{ flex: 1, wordBreak: "break-all" }}>
                      {e.name} <span style={{ opacity: 0.6, fontSize: 12 }}>({e.source})</span>
                    </span>
                    <Button variant="secondary" onClick={() => downloadOnline(e)} disabled={resolveBusy !== null}>
                      {resolveBusy === e.download_url ? "Downloading…" : "Download & use"}
                    </Button>
                  </div>
                ))}
              </div>
            )}

            {!resolving && !resolvedIni && localMatches.length === 0 && onlineResults.length === 0 && (
              <div style={{ opacity: 0.8 }}>
                No definition was resolved automatically for this signature. If you have your ECU's
                <code>.ini</code> file, load it as a last resort.
              </div>
            )}

            <div>
              <Button variant="secondary" onClick={pickManualIni} disabled={resolveBusy === "manual"}>
                {resolveBusy === "manual" ? "Importing…" : "Choose .ini file manually…"}
              </Button>
            </div>
          </div>
        )}

        {step === "name" && (
          <div>
            <label style={{ display: "block", marginBottom: "0.5rem" }}>Project name</label>
            <input
              type="text"
              value={projectName}
              placeholder="My ECU project"
              onChange={(e) => setProjectName(e.target.value)}
              style={{ width: "100%" }}
              disabled={creating}
            />
            {resolvedIni ? (
              <p style={{ opacity: 0.8, fontSize: 12, marginTop: "0.5rem" }}>
                Using <b>{resolvedIni.name}</b> ({resolvedIni.source}).
              </p>
            ) : transport === "offline" ? (
              <div style={{ marginTop: "0.75rem" }}>
                <label style={{ display: "block", marginBottom: "0.5rem" }}>ECU definition (INI)</label>
                {inis.length === 0 ? (
                  <p style={{ opacity: 0.7, fontSize: 12 }}>
                    No definitions installed yet — add one via New Project → Browse or the
                    online INI repository, then come back.
                  </p>
                ) : (
                  <select
                    value=""
                    onChange={(e) => {
                      const ini = inis.find((i) => i.id === e.target.value);
                      if (ini) {
                        setResolvedIni({ id: ini.id, path: ini.path, name: ini.name, source: "local" });
                      }
                    }}
                    style={{ width: "100%" }}
                    disabled={creating}
                  >
                    <option value="" disabled>
                      Select a definition…
                    </option>
                    {inis.map((ini) => (
                      <option key={ini.id} value={ini.id}>
                        {ini.name}
                      </option>
                    ))}
                  </select>
                )}
                <p style={{ opacity: 0.7, fontSize: 12, marginTop: "0.5rem" }}>
                  Pick a definition to create the project; Finish without one just closes.
                </p>
              </div>
            ) : (
              <p style={{ opacity: 0.7, fontSize: 12, marginTop: "0.5rem" }}>
                No ECU definition was resolved, so Finish will just close this wizard.
              </p>
            )}
            {createError && (
              <p style={{ color: "var(--color-error, #d33)", fontSize: 12, marginTop: "0.5rem" }}>
                {createError}
              </p>
            )}
          </div>
        )}
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={handleClose} disabled={creating}>
          Cancel
        </Button>
        {stepIndex > 0 && (
          <Button
            variant="secondary"
            onClick={() => setStep(prevStep(step, transport))}
            disabled={creating}
          >
            Back
          </Button>
        )}
        {last ? (
          <Button
            variant="primary"
            onClick={finishAndCreate}
            disabled={!projectName.trim() || creating}
          >
            {creating ? "Creating…" : resolvedIni ? "Create Project" : "Finish"}
          </Button>
        ) : (
          <Button
            variant="primary"
            onClick={() => setStep(nextStep(step, transport))}
            disabled={!canAdvance}
          >
            Next
          </Button>
        )}
      </Dialog.Footer>
    </Dialog>
  );
}
