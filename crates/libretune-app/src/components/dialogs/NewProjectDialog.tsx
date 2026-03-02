import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import "./NewProjectDialog.css";

interface IniEntry {
  id: string;
  name: string;
  signature: string;
  path: string;
}

interface NewProjectDialogProps {
  isOpen: boolean;
  onClose: () => void;
  inis: IniEntry[];
  onImportIni: () => void;
  /** Creates a project with the given INI. Returns true on success. */
  onCreateProject: (projectName: string, iniId: string) => Promise<boolean>;
  /** Called when user chooses to import an existing tune file */
  onImportTune: (tunePath: string) => void;
  /** Called when user chooses to generate a base map */
  onGenerateBaseMap: () => void;
}

type Step = "select-ini" | "choose-tune";

export default function NewProjectDialog({
  isOpen,
  onClose,
  inis,
  onImportIni,
  onCreateProject,
  onImportTune,
  onGenerateBaseMap,
}: NewProjectDialogProps) {
  const [step, setStep] = useState<Step>("select-ini");
  const [projectName, setProjectName] = useState("");
  const [selectedIni, setSelectedIni] = useState("");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState("");

  // Reset on open
  useEffect(() => {
    if (isOpen) {
      setStep("select-ini");
      setProjectName("");
      setSelectedIni("");
      setCreating(false);
      setError("");
    }
  }, [isOpen]);

  async function handleCreate() {
    if (!projectName.trim() || !selectedIni) return;
    setCreating(true);
    setError("");
    try {
      const success = await onCreateProject(projectName.trim(), selectedIni);
      if (success) {
        setStep("choose-tune");
      }
    } catch (e) {
      setError(`Failed to create project: ${e}`);
    } finally {
      setCreating(false);
    }
  }

  async function handleImportTune() {
    try {
      const path = await open({
        multiple: false,
        filters: [
          { name: "Tune Files", extensions: ["msq", "xml"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });
      if (path && typeof path === "string") {
        onImportTune(path);
        onClose();
      }
    } catch (e) {
      console.error("Error browsing for tune:", e);
    }
  }

  function handleGenerateBaseMap() {
    onGenerateBaseMap();
    onClose();
  }

  function handleSkip() {
    // Close dialog — project is already created with empty/default tune
    onClose();
  }

  if (!isOpen) return null;

  const canCreate = projectName.trim() && selectedIni;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="new-project-dialog" onClick={(e) => e.stopPropagation()}>

        {step === "select-ini" && (
          <>
            <h2 className="dialog-title">New Project</h2>
            <p className="dialog-subtitle">
              Select an ECU definition (INI) and name your project.
            </p>

            {/* INI selector */}
            <div className="field-group">
              <label className="field-label">ECU Definition (INI)</label>
              {inis.length > 0 ? (
                <select
                  className="ini-select"
                  value={selectedIni}
                  onChange={(e) => setSelectedIni(e.target.value)}
                >
                  <option value="">Select ECU definition...</option>
                  {inis.map((ini) => (
                    <option key={ini.id} value={ini.id}>
                      {ini.name} — {ini.signature}
                    </option>
                  ))}
                </select>
              ) : (
                <div className="no-ini-available">
                  <p>No ECU definitions available. Import one to get started.</p>
                </div>
              )}
              <button className="import-ini-link" onClick={onImportIni}>
                + Import ECU Definition...
              </button>
            </div>

            {/* Project name */}
            <div className="field-group">
              <label className="field-label">Project Name</label>
              <input
                type="text"
                className="name-input"
                value={projectName}
                onChange={(e) => setProjectName(e.target.value)}
                placeholder="e.g. My SR20 Build"
                onKeyDown={(e) => { if (e.key === "Enter" && canCreate) handleCreate(); }}
              />
            </div>

            {error && <div className="error-msg">{error}</div>}

            <div className="dialog-actions">
              <button className="cancel-btn" onClick={onClose}>Cancel</button>
              <button
                className={`create-btn ${canCreate && !creating ? "" : "disabled"}`}
                onClick={handleCreate}
                disabled={!canCreate || creating}
              >
                {creating ? "Creating..." : "Create Project"}
              </button>
            </div>
          </>
        )}

        {step === "choose-tune" && (
          <>
            <h2 className="dialog-title">Project Created</h2>
            <p className="dialog-subtitle">
              How would you like to start your tune?
            </p>

            <div className="tune-choice-cards">
              <button className="tune-choice-card" onClick={handleImportTune}>
                <span className="choice-icon">📂</span>
                <span className="choice-label">Import Existing Tune</span>
                <span className="choice-desc">
                  Load an existing .msq or .xml tune file into this project
                </span>
              </button>

              <button className="tune-choice-card" onClick={handleGenerateBaseMap}>
                <span className="choice-icon">🔧</span>
                <span className="choice-label">Generate Base Map</span>
                <span className="choice-desc">
                  Create a safe starting tune from your engine specifications
                </span>
              </button>
            </div>

            <div className="dialog-actions">
              <button className="skip-btn" onClick={handleSkip}>
                Skip — start with default values
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
