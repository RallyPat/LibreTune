/**
 * TuneHistoryPanel - Git-based tune version history
 * 
 * Shows commit history timeline, allows viewing diffs between versions,
 * and restoring previous tune versions.
 */

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./TuneHistoryPanel.css";

interface CommitInfo {
  sha_short: string;
  sha: string;
  message: string;
  annotation?: string | null;
  author: string;
  timestamp: string;
  is_head: boolean;
}

interface BranchInfo {
  name: string;
  is_current: boolean;
  tip_sha: string;
}

interface TuneChange {
  name: string;
  old_value: string | null;
  new_value: string | null;
  change_type: string;
}

interface CommitDiff {
  from_sha: string;
  to_sha: string;
  changes: TuneChange[];
  files_changed: string[];
}

interface TuneHistoryPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export function TuneHistoryPanel({ isOpen, onClose }: TuneHistoryPanelProps) {
  const [hasRepo, setHasRepo] = useState(false);
  const [history, setHistory] = useState<CommitInfo[]>([]);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [currentBranch, setCurrentBranch] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedCommit, setSelectedCommit] = useState<CommitInfo | null>(null);
  const [diffData, setDiffData] = useState<CommitDiff | null>(null);
  const [showDiff, setShowDiff] = useState(false);
  const [showBranchDialog, setShowBranchDialog] = useState(false);
  const [newBranchName, setNewBranchName] = useState("");

  useEffect(() => {
    if (isOpen) {
      loadData();
    }
  }, [isOpen]);

  async function loadData() {
    setLoading(true);
    try {
      const hasGit = await invoke<boolean>("git_has_repo");
      setHasRepo(hasGit);
      
      if (hasGit) {
        const [historyData, branchData, branch] = await Promise.all([
          invoke<CommitInfo[]>("git_history", { maxCount: 50 }),
          invoke<BranchInfo[]>("git_list_branches"),
          invoke<string | null>("git_current_branch"),
        ]);
        setHistory(historyData);
        setBranches(branchData);
        setCurrentBranch(branch);
      }
    } catch (e) {
      console.error("Failed to load git data:", e);
    } finally {
      setLoading(false);
    }
  }

  async function initRepo() {
    try {
      await invoke("git_init_project");
      await loadData();
    } catch (e) {
      console.error("Failed to init git:", e);
      alert(`Failed to initialize version control: ${e}`);
    }
  }

  async function handleCheckout(sha: string) {
    if (!confirm("Restore tune to this version? Unsaved changes will be lost.")) {
      return;
    }
    try {
      await invoke("git_checkout", { sha });
      await loadData();
    } catch (e) {
      console.error("Failed to checkout:", e);
      alert(`Failed to restore version: ${e}`);
    }
  }

  async function handleViewDiff(commit: CommitInfo) {
    setSelectedCommit(commit);
    // Get diff between this commit and previous
    const idx = history.findIndex(c => c.sha === commit.sha);
    if (idx < history.length - 1) {
      try {
        const diff = await invoke<CommitDiff>("git_diff", {
          fromSha: history[idx + 1].sha,
          toSha: commit.sha,
        });
        setDiffData(diff);
        setShowDiff(true);
      } catch (e) {
        console.error("Failed to get diff:", e);
      }
    }
  }

  async function handleCreateBranch() {
    if (!newBranchName.trim()) return;
    try {
      await invoke("git_create_branch", { name: newBranchName.trim() });
      setNewBranchName("");
      setShowBranchDialog(false);
      await loadData();
    } catch (e) {
      console.error("Failed to create branch:", e);
      alert(`Failed to create branch: ${e}`);
    }
  }

  async function handleSwitchBranch(name: string) {
    if (!confirm(`Switch to branch "${name}"? Unsaved changes will be lost.`)) {
      return;
    }
    try {
      await invoke("git_switch_branch", { name });
      await loadData();
    } catch (e) {
      console.error("Failed to switch branch:", e);
      alert(`Failed to switch branch: ${e}`);
    }
  }

  async function handleCommit() {
    const message = prompt("Commit message:", "Manual checkpoint");
    if (message) {
      const annotation = prompt("Annotation (optional):", "");
      try {
        await invoke("git_commit", {
          message,
          annotation: annotation?.trim() ? annotation : null,
        });
        await loadData();
      } catch (e) {
        console.error("Failed to commit:", e);
        alert(`Failed to commit: ${e}`);
      }
    }
  }

  if (!isOpen) return null;

  return (
    <div className="tune-history-overlay">
      <div className="tune-history-panel">
        <div className="tune-history-header">
          <h2>Tune History</h2>
          <button className="close-btn" onClick={onClose}>×</button>
        </div>

        {loading ? (
          <div className="tune-history-loading">Loading...</div>
        ) : !hasRepo ? (
          <div className="tune-history-init">
            <div className="init-icon">📦</div>
            <h3>Enable Version Control</h3>
            <p>Track changes to your tune over time with git-based versioning.</p>
            <button className="primary-btn" onClick={initRepo}>
              Initialize Version Control
            </button>
          </div>
        ) : (
          <>
            {/* Branch selector */}
            <div className="branch-bar">
              <select 
                value={currentBranch || ""} 
                onChange={(e) => handleSwitchBranch(e.target.value)}
                className="branch-select"
              >
                {branches.map(b => (
                  <option key={b.name} value={b.name}>
                    {b.is_current ? "● " : ""}{b.name}
                  </option>
                ))}
              </select>
              <button 
                className="icon-btn" 
                onClick={() => setShowBranchDialog(true)}
                title="New Branch"
              >
                ➕
              </button>
              <button 
                className="icon-btn" 
                onClick={handleCommit}
                title="Create Checkpoint"
              >
                💾
              </button>
            </div>

            {/* Commit timeline */}
            <div className="commit-timeline">
              {history.length === 0 ? (
                <div className="no-history">
                  No commits yet. Changes will be tracked automatically when you save.
                </div>
              ) : (
                history.map((commit, idx) => (
                  <div 
                    key={commit.sha} 
                    className={`commit-item ${commit.is_head ? "is-head" : ""}`}
                  >
                    <div className="commit-line">
                      <div className="commit-dot" />
                      {idx < history.length - 1 && <div className="commit-connector" />}
                    </div>
                    <div className="commit-content">
                      <div className="commit-header">
                        <span className="commit-sha">{commit.sha_short}</span>
                        <span className="commit-time">{commit.timestamp}</span>
                      </div>
                      <div className="commit-message">{commit.message}</div>
                      {commit.annotation && (
                        <div className="commit-annotation">Note: {commit.annotation}</div>
                      )}
                      <div className="commit-actions">
                        {idx < history.length - 1 && (
                          <button 
                            className="small-btn"
                            onClick={() => handleViewDiff(commit)}
                          >
                            View Changes
                          </button>
                        )}
                        {!commit.is_head && (
                          <button 
                            className="small-btn"
                            onClick={() => handleCheckout(commit.sha)}
                          >
                            Restore
                          </button>
                        )}
                        {commit.is_head && (
                          <span className="head-badge">Current</span>
                        )}
                      </div>
                    </div>
                  </div>
                ))
              )}
            </div>
          </>
        )}

        {/* Diff modal */}
        {showDiff && diffData && (
          <div className="diff-modal-overlay" onClick={() => setShowDiff(false)}>
            <div className="diff-modal" onClick={e => e.stopPropagation()}>
              <div className="diff-modal-header">
                <h3>Changes in {selectedCommit?.sha_short}</h3>
                <button className="close-btn" onClick={() => setShowDiff(false)}>×</button>
              </div>
              <div className="diff-content">
                {diffData.files_changed.length === 0 ? (
                  <div className="no-changes">No file changes detected</div>
                ) : (
                  <>
                    <div className="files-changed">
                      <strong>Files changed:</strong>
                      <ul>
                        {diffData.files_changed.map(f => (
                          <li key={f}>{f}</li>
                        ))}
                      </ul>
                    </div>
                    {diffData.changes.length > 0 && (
                      <div className="constant-changes">
                        <strong>Constants changed:</strong>
                        <table className="changes-table">
                          <thead>
                            <tr>
                              <th>Name</th>
                              <th>Old</th>
                              <th>New</th>
                            </tr>
                          </thead>
                          <tbody>
                            {diffData.changes.map(c => (
                              <tr key={c.name} className={`change-${c.change_type}`}>
                                <td>{c.name}</td>
                                <td>{c.old_value || "—"}</td>
                                <td>{c.new_value || "—"}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                  </>
                )}
              </div>
            </div>
          </div>
        )}

        {/* New branch dialog */}
        {showBranchDialog && (
          <div className="diff-modal-overlay" onClick={() => setShowBranchDialog(false)}>
            <div className="diff-modal small" onClick={e => e.stopPropagation()}>
              <div className="diff-modal-header">
                <h3>New Branch</h3>
                <button className="close-btn" onClick={() => setShowBranchDialog(false)}>×</button>
              </div>
              <div className="branch-dialog-content">
                <p>Create a new branch for experimenting with tune changes.</p>
                <input
                  type="text"
                  value={newBranchName}
                  onChange={e => setNewBranchName(e.target.value)}
                  placeholder="Branch name (e.g., high-boost-experiment)"
                  className="branch-input"
                />
                <div className="dialog-actions">
                  <button onClick={() => setShowBranchDialog(false)}>Cancel</button>
                  <button 
                    className="primary-btn" 
                    onClick={handleCreateBranch}
                    disabled={!newBranchName.trim()}
                  >
                    Create Branch
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default TuneHistoryPanel;
