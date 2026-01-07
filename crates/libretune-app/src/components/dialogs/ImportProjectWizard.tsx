import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { FolderOpen, FileArchive, Check, AlertTriangle, X, ArrowRight, Loader } from 'lucide-react';
import './ImportProjectWizard.css';

interface ImportProjectWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onImportComplete: (projectPath: string) => void;
}

interface ImportPreview {
  project_name: string;
  ini_file: Option<string>;
  has_tune: boolean;
  restore_point_count: number;
  has_pc_variables: boolean;
  connection_port: Option<string>;
  connection_baud: Option<number>;
}

type Option<T> = T | null;

export default function ImportProjectWizard({
  isOpen,
  onClose,
  onImportComplete,
}: ImportProjectWizardProps) {
  const [step, setStep] = useState<'select' | 'confirm'>('select');
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = () => {
    setStep('select');
    setSelectedPath(null);
    setPreview(null);
    setLoading(false);
    setImporting(false);
    setError(null);
  };

  const handleClose = () => {
    reset();
    onClose();
  };

  const handleSelectFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select TS Project Folder',
      });

      if (selected && typeof selected === 'string') {
        setSelectedPath(selected);
        setLoading(true);
        setError(null);

        // Preview the import
        try {
          const previewData = await invoke<ImportPreview>('preview_tunerstudio_import', {
            path: selected,
          });
          setPreview(previewData);
          setStep('confirm');
        } catch (e) {
          setError(`Not a valid TS project: ${e}`);
          setSelectedPath(null);
        } finally {
          setLoading(false);
        }
      }
    } catch (e) {
      setError(`Failed to open folder picker: ${e}`);
    }
  };

  const handleImport = async () => {
    if (!selectedPath) return;

    setImporting(true);
    setError(null);

    try {
      const projectPath = await invoke<string>('import_tunerstudio_project', {
        sourcePath: selectedPath,
      });

      // Import successful - call the callback which will open the project
      onImportComplete(projectPath);
      handleClose();
    } catch (e) {
      setError(`Import failed: ${e}`);
      setImporting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="import-wizard-overlay" onClick={handleClose}>
      <div className="import-wizard-dialog" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="import-wizard-header">
          <FileArchive size={22} className="header-icon" />
          <h2>Import TS Project</h2>
          <button className="close-btn" onClick={handleClose}>
            <X size={18} />
          </button>
        </div>

        {/* Content */}
        <div className="import-wizard-content">
          {/* Step indicators */}
          <div className="step-indicators">
            <div className={`step-indicator ${step === 'select' ? 'active' : 'completed'}`}>
              <span className="step-number">1</span>
              <span className="step-label">Select Folder</span>
            </div>
            <ArrowRight size={16} className="step-arrow" />
            <div className={`step-indicator ${step === 'confirm' ? 'active' : ''}`}>
              <span className="step-number">2</span>
              <span className="step-label">Confirm Import</span>
            </div>
          </div>

          {/* Error display */}
          {error && (
            <div className="import-error">
              <AlertTriangle size={16} />
              <span>{error}</span>
            </div>
          )}

          {/* Step 1: Select folder */}
          {step === 'select' && (
            <div className="import-step select-step">
              <div className="select-folder-area" onClick={handleSelectFolder}>
                {loading ? (
                  <>
                    <Loader size={32} className="spinner" />
                    <p>Analyzing project...</p>
                  </>
                ) : (
                  <>
                    <FolderOpen size={48} />
                    <p>Click to select a TS project folder</p>
                    <span className="hint">
                      Look for a folder containing <code>project.properties</code>
                    </span>
                  </>
                )}
              </div>
              <div className="import-info">
                <h4>What gets imported:</h4>
                <ul>
                  <li>Current tune (CurrentTune.msq)</li>
                  <li>PC variables (pcVariableValues.msq)</li>
                  <li>Restore points / backups</li>
                  <li>Connection settings</li>
                </ul>
              </div>
            </div>
          )}

          {/* Step 2: Confirm */}
          {step === 'confirm' && preview && (
            <div className="import-step confirm-step">
              <div className="preview-card">
                <h3>{preview.project_name}</h3>
                <div className="preview-details">
                  <div className="preview-row">
                    <span className="label">INI File:</span>
                    <span className="value">{preview.ini_file || 'None specified'}</span>
                  </div>
                  <div className="preview-row">
                    <span className="label">Has Tune:</span>
                    <span className={`value ${preview.has_tune ? 'success' : 'warning'}`}>
                      {preview.has_tune ? <><Check size={14} /> Yes</> : 'No'}
                    </span>
                  </div>
                  <div className="preview-row">
                    <span className="label">Restore Points:</span>
                    <span className="value">{preview.restore_point_count}</span>
                  </div>
                  <div className="preview-row">
                    <span className="label">PC Variables:</span>
                    <span className={`value ${preview.has_pc_variables ? 'success' : ''}`}>
                      {preview.has_pc_variables ? <><Check size={14} /> Yes</> : 'No'}
                    </span>
                  </div>
                  {preview.connection_port && (
                    <div className="preview-row">
                      <span className="label">Serial Port:</span>
                      <span className="value">
                        {preview.connection_port}
                        {preview.connection_baud && ` @ ${preview.connection_baud}`}
                      </span>
                    </div>
                  )}
                </div>
              </div>

              <div className="import-note">
                <p>
                  The project will be imported to your LibreTune projects folder and opened
                  automatically.
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="import-wizard-footer">
          {step === 'select' && (
            <button className="btn-cancel" onClick={handleClose}>
              Cancel
            </button>
          )}

          {step === 'confirm' && (
            <>
              <button className="btn-back" onClick={() => setStep('select')} disabled={importing}>
                Back
              </button>
              <button className="btn-import" onClick={handleImport} disabled={importing}>
                {importing ? (
                  <>
                    <Loader size={14} className="spinner" />
                    Importing...
                  </>
                ) : (
                  <>
                    <FileArchive size={14} />
                    Import Project
                  </>
                )}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
