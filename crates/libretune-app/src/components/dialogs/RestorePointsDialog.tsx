//! Restore Points Dialog
//!
//! Allows user to view, load, and delete restore points for the current project.
//! Shows warning when loading if there are unsaved changes.

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Clock, Trash2, RotateCcw, X, AlertTriangle, FileArchive } from "lucide-react";
import "./RestorePointsDialog.css";

export interface RestorePointInfo {
  filename: string;
  path: string;
  created: string;
  size_bytes: number;
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
  tuneModified: boolean;
  onRestorePointLoaded: () => void;
}

export default function RestorePointsDialog({
  isOpen,
  onClose,
  tuneModified,
  onRestorePointLoaded,
}: Props) {
  const [restorePoints, setRestorePoints] = useState<RestorePointInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  
  // Confirmation dialogs
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [confirmLoad, setConfirmLoad] = useState<string | null>(null);

  // Load restore points when dialog opens
  useEffect(() => {
    if (isOpen) {
      loadRestorePoints();
    }
  }, [isOpen]);

  const loadRestorePoints = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const points = await invoke<RestorePointInfo[]>("list_restore_points");
      setRestorePoints(points);
    } catch (e) {
      console.error("Failed to load restore points:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleCreateRestorePoint = async () => {
    setActionInProgress("creating");
    setError(null);
    
    try {
      await invoke("create_restore_point");
      await loadRestorePoints();
    } catch (e) {
      console.error("Failed to create restore point:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setActionInProgress(null);
    }
  };

  const handleLoad = async (filename: string) => {
    // If tune is modified, show confirmation first
    if (tuneModified && !confirmLoad) {
      setConfirmLoad(filename);
      return;
    }
    
    setConfirmLoad(null);
    setActionInProgress(filename);
    setError(null);
    
    try {
      await invoke("load_restore_point", { filename });
      onRestorePointLoaded();
      onClose();
    } catch (e) {
      console.error("Failed to load restore point:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setActionInProgress(null);
    }
  };

  const handleDelete = async (filename: string) => {
    // Show confirmation first
    if (!confirmDelete || confirmDelete !== filename) {
      setConfirmDelete(filename);
      return;
    }
    
    setConfirmDelete(null);
    setActionInProgress(filename);
    setError(null);
    
    try {
      await invoke("delete_restore_point", { filename });
      await loadRestorePoints();
    } catch (e) {
      console.error("Failed to delete restore point:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setActionInProgress(null);
    }
  };

  const formatDate = (isoString: string): string => {
    try {
      const date = new Date(isoString);
      return date.toLocaleDateString(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return isoString;
    }
  };

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  if (!isOpen) return null;

  return (
    <div className="restore-points-overlay" onClick={onClose}>
      <div className="restore-points-dialog" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="restore-points-header">
          <FileArchive className="header-icon" size={20} />
          <h2>Restore Points</h2>
          <button className="close-btn" onClick={onClose}>
            <X size={18} />
          </button>
        </div>

        {/* Content */}
        <div className="restore-points-content">
          {error && (
            <div className="restore-points-error">
              <AlertTriangle size={16} />
              {error}
            </div>
          )}

          {/* Load confirmation dialog */}
          {confirmLoad && (
            <div className="restore-points-confirm">
              <AlertTriangle size={16} className="warning-icon" />
              <div className="confirm-message">
                <strong>Unsaved changes will be lost</strong>
                <p>Loading this restore point will discard your current unsaved changes. Continue?</p>
              </div>
              <div className="confirm-actions">
                <button className="btn-cancel" onClick={() => setConfirmLoad(null)}>
                  Cancel
                </button>
                <button className="btn-danger" onClick={() => handleLoad(confirmLoad)}>
                  Load Anyway
                </button>
              </div>
            </div>
          )}

          {/* Delete confirmation dialog */}
          {confirmDelete && (
            <div className="restore-points-confirm">
              <AlertTriangle size={16} className="warning-icon" />
              <div className="confirm-message">
                <strong>Delete restore point?</strong>
                <p>This action cannot be undone.</p>
              </div>
              <div className="confirm-actions">
                <button className="btn-cancel" onClick={() => setConfirmDelete(null)}>
                  Cancel
                </button>
                <button className="btn-danger" onClick={() => handleDelete(confirmDelete)}>
                  Delete
                </button>
              </div>
            </div>
          )}

          {loading ? (
            <div className="restore-points-loading">Loading restore points...</div>
          ) : restorePoints.length === 0 ? (
            <div className="restore-points-empty">
              <FileArchive size={48} />
              <p>No restore points yet</p>
              <span>Create a restore point to save a backup of your current tune.</span>
            </div>
          ) : (
            <div className="restore-points-list">
              {restorePoints.map((point) => (
                <div
                  key={point.filename}
                  className={`restore-point-item ${actionInProgress === point.filename ? "loading" : ""}`}
                >
                  <div className="restore-point-info">
                    <div className="restore-point-name">{point.filename}</div>
                    <div className="restore-point-meta">
                      <span className="restore-point-date">
                        <Clock size={12} />
                        {formatDate(point.created)}
                      </span>
                      <span className="restore-point-size">{formatSize(point.size_bytes)}</span>
                    </div>
                  </div>
                  <div className="restore-point-actions">
                    <button
                      className="btn-load"
                      onClick={() => handleLoad(point.filename)}
                      disabled={actionInProgress !== null}
                      title="Load this restore point"
                    >
                      <RotateCcw size={14} />
                      Load
                    </button>
                    <button
                      className="btn-delete"
                      onClick={() => handleDelete(point.filename)}
                      disabled={actionInProgress !== null}
                      title="Delete this restore point"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="restore-points-footer">
          <span className="restore-points-count">
            {restorePoints.length} restore point{restorePoints.length !== 1 ? "s" : ""}
          </span>
          <div className="footer-actions">
            <button
              className="btn-create"
              onClick={handleCreateRestorePoint}
              disabled={actionInProgress !== null}
            >
              {actionInProgress === "creating" ? "Creating..." : "Create Restore Point"}
            </button>
            <button className="btn-close" onClick={onClose}>
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
