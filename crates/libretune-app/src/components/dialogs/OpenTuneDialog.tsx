import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./OpenTuneDialog.css";

interface MsqInfo {
  signature: string;
  version: string;
  file_name: string;
  file_size: number;
  constant_count: number;
  ini_name?: string;
  saved_at?: string;
  author?: string;
  description?: string;
}

interface IniEntry {
  id: string;
  name: string;
  signature: string;
  path: string;
}

interface OpenTuneDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenTune: (projectName: string, iniId: string, tunePath: string) => void;
  inis: IniEntry[];
  onImportIni: () => void;
}

export default function OpenTuneDialog({
  isOpen,
  onClose,
  onOpenTune,
  inis,
  onImportIni,
}: OpenTuneDialogProps) {
  const [tunePath, setTunePath] = useState("");
  const [msqInfo, setMsqInfo] = useState<MsqInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [selectedIni, setSelectedIni] = useState("");
  const [matchedInis, setMatchedInis] = useState<IniEntry[]>([]);
  const [error, setError] = useState("");

  // Reset on open
  useEffect(() => {
    if (isOpen) {
      setTunePath("");
      setMsqInfo(null);
      setProjectName("");
      setSelectedIni("");
      setMatchedInis([]);
      setError("");
    }
  }, [isOpen]);

  // Auto-match INIs when msqInfo changes
  useEffect(() => {
    if (msqInfo) {
      const sig = msqInfo.signature.toLowerCase();
      const matched = inis.filter(
        (ini) =>
          ini.signature.toLowerCase() === sig ||
          ini.signature.toLowerCase().includes(sig) ||
          sig.includes(ini.signature.toLowerCase())
      );
      setMatchedInis(matched);
      if (matched.length === 1) {
        setSelectedIni(matched[0].id);
      } else if (matched.length === 0) {
        // Try partial match
        const partial = inis.filter((ini) => {
          const parts = sig.split(/\s+/);
          return parts.some((p) => ini.signature.toLowerCase().includes(p));
        });
        setMatchedInis(partial);
        if (partial.length === 1) {
          setSelectedIni(partial[0].id);
        }
      }
    }
  }, [msqInfo, inis]);

  async function browseTune() {
    try {
      const path = await open({
        multiple: false,
        filters: [
          { name: "Tune Files", extensions: ["msq", "xml"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });
      if (path && typeof path === "string") {
        setTunePath(path);
        setError("");
        setLoading(true);
        try {
          const info = await invoke<MsqInfo>("get_msq_info", { path });
          setMsqInfo(info);
          // Auto-generate project name from filename
          const fileName = info.file_name.replace(/\.(msq|xml)$/i, "");
          setProjectName(fileName);
        } catch (e) {
          setError(`Failed to read tune file: ${e}`);
          setMsqInfo(null);
        } finally {
          setLoading(false);
        }
      }
    } catch (e) {
      console.error("Error browsing for tune:", e);
    }
  }

  function handleCreate() {
    if (!projectName.trim() || !selectedIni || !tunePath) return;
    onOpenTune(projectName.trim(), selectedIni, tunePath);
  }

  if (!isOpen) return null;

  const canCreate = projectName.trim() && selectedIni && tunePath;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="open-tune-dialog" onClick={(e) => e.stopPropagation()}>
        <h2 className="dialog-title">Open Tune File</h2>
        <p className="dialog-subtitle">
          Select a tune file (.msq) and an ECU definition to create a project.
        </p>

        {/* File picker */}
        <div className="field-group">
          <label className="field-label">Tune File</label>
          <div className="file-picker">
            <input
              type="text"
              readOnly
              value={tunePath ? tunePath.split(/[\\/]/).pop() || tunePath : ""}
              placeholder="No file selected"
              className="file-input"
            />
            <button className="browse-btn" onClick={browseTune}>
              Browse...
            </button>
          </div>
        </div>

        {error && <div className="error-msg">{error}</div>}

        {loading && <div className="loading-msg">Reading tune file...</div>}

        {/* MSQ Info preview */}
        {msqInfo && (
          <div className="msq-preview">
            <div className="preview-row">
              <span className="preview-label">Signature:</span>
              <span className="preview-value">{msqInfo.signature}</span>
            </div>
            <div className="preview-row">
              <span className="preview-label">Constants:</span>
              <span className="preview-value">{msqInfo.constant_count}</span>
            </div>
            {msqInfo.author && (
              <div className="preview-row">
                <span className="preview-label">Author:</span>
                <span className="preview-value">{msqInfo.author}</span>
              </div>
            )}
            <div className="preview-row">
              <span className="preview-label">Size:</span>
              <span className="preview-value">
                {(msqInfo.file_size / 1024).toFixed(1)} KB
              </span>
            </div>
          </div>
        )}

        {/* INI selector */}
        {msqInfo && (
          <div className="field-group">
            <label className="field-label">ECU Definition (INI)</label>
            {matchedInis.length > 0 ? (
              <select
                className="ini-select"
                value={selectedIni}
                onChange={(e) => setSelectedIni(e.target.value)}
              >
                <option value="">Select ECU definition...</option>
                {matchedInis.map((ini) => (
                  <option key={ini.id} value={ini.id}>
                    {ini.name} — {ini.signature}
                  </option>
                ))}
                {/* Show all INIs below a divider if there are unmatched ones */}
                {inis.length > matchedInis.length && (
                  <>
                    <option disabled>── All definitions ──</option>
                    {inis
                      .filter((i) => !matchedInis.find((m) => m.id === i.id))
                      .map((ini) => (
                        <option key={ini.id} value={ini.id}>
                          {ini.name} — {ini.signature}
                        </option>
                      ))}
                  </>
                )}
              </select>
            ) : (
              <div className="no-ini-match">
                <p>No matching ECU definition found for this tune's signature.</p>
                <button className="import-ini-btn" onClick={onImportIni}>
                  Import ECU Definition...
                </button>
                <select
                  className="ini-select"
                  value={selectedIni}
                  onChange={(e) => setSelectedIni(e.target.value)}
                  style={{ marginTop: 8 }}
                >
                  <option value="">Select manually...</option>
                  {inis.map((ini) => (
                    <option key={ini.id} value={ini.id}>
                      {ini.name} — {ini.signature}
                    </option>
                  ))}
                </select>
              </div>
            )}
          </div>
        )}

        {/* Project name */}
        {msqInfo && selectedIni && (
          <div className="field-group">
            <label className="field-label">Project Name</label>
            <input
              type="text"
              className="name-input"
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
              placeholder="My Tune"
            />
          </div>
        )}

        {/* Actions */}
        <div className="dialog-actions">
          <button className="cancel-btn" onClick={onClose}>
            Cancel
          </button>
          <button
            className={`create-btn ${canCreate ? "" : "disabled"}`}
            onClick={handleCreate}
            disabled={!canCreate}
          >
            Open
          </button>
        </div>
      </div>
    </div>
  );
}
