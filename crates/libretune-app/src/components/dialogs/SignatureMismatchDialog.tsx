//! Signature Mismatch Dialog
//!
//! Shown when the ECU signature doesn't match the loaded INI file.
//! Allows user to select a matching INI, continue anyway, or cancel.

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Check, X, Download, RefreshCw, Globe, Wifi, WifiOff, Save, HelpCircle } from "lucide-react";
import "./SignatureMismatchDialog.css";

export interface MatchingIniInfo {
  path: string;
  name: string;
  signature: string;
  match_type: "exact" | "partial" | "mismatch";
}

export interface SignatureMismatchInfo {
  ecu_signature: string;
  ini_signature: string;
  match_type: "exact" | "partial" | "mismatch";
  current_ini_path: string | null;
  matching_inis: MatchingIniInfo[];
}

interface OnlineIniEntry {
  source: string;
  name: string;
  signature: string | null;
  download_url: string;
  repo_path: string;
  size: number | null;
}

interface Props {
  isOpen: boolean;
  mismatchInfo: SignatureMismatchInfo | null;
  onClose: () => void;
  onSelectIni: (path: string) => void;
  onContinue: () => void;
}

export default function SignatureMismatchDialog({
  isOpen,
  mismatchInfo,
  onClose,
  onSelectIni,
  onContinue,
}: Props) {
  const [selectedIni, setSelectedIni] = useState<string | null>(null);
  const [switching, setSwitching] = useState(false);
  const [savingTune, setSavingTune] = useState(false);
  
  // Online search state
  const [showOnlineSearch, setShowOnlineSearch] = useState(false);
  const [onlineResults, setOnlineResults] = useState<OnlineIniEntry[]>([]);
  const [searching, setSearching] = useState(false);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [hasInternet, setHasInternet] = useState<boolean | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);

  // Check internet connectivity when online search is opened
  useEffect(() => {
    if (showOnlineSearch && hasInternet === null) {
      checkConnectivity();
    }
  }, [showOnlineSearch]);

  const checkConnectivity = async () => {
    try {
      const connected = await invoke<boolean>("check_internet_connectivity");
      setHasInternet(connected);
      if (connected && mismatchInfo) {
        searchOnline();
      }
    } catch (e) {
      console.error("Failed to check connectivity:", e);
      setHasInternet(false);
    }
  };

  const searchOnline = async () => {
    if (!mismatchInfo) return;
    
    setSearching(true);
    setSearchError(null);
    
    try {
      const results = await invoke<OnlineIniEntry[]>("search_online_inis", {
        signature: mismatchInfo.ecu_signature,
      });
      setOnlineResults(results);
    } catch (e) {
      console.error("Failed to search online:", e);
      setSearchError(e instanceof Error ? e.message : String(e));
    } finally {
      setSearching(false);
    }
  };

  const handleDownload = async (entry: OnlineIniEntry) => {
    setDownloading(entry.download_url);
    
    try {
      const downloadedPath = await invoke<string>("download_ini", {
        downloadUrl: entry.download_url,
        name: entry.name,
        source: entry.source,
      });
      
      // Switch to the downloaded INI
      await invoke("update_project_ini", {
        iniPath: downloadedPath,
        forceResync: true,
      });
      
      onSelectIni(downloadedPath);
    } catch (e) {
      console.error("Failed to download INI:", e);
      setSearchError(e instanceof Error ? e.message : String(e));
    } finally {
      setDownloading(null);
    }
  };

  const handleSaveEcuTune = async () => {
    setSavingTune(true);
    
    try {
      // Sync ECU data first
      await invoke("sync_ecu_data");
      
      // Save the current tune with a timestamp
      const timestamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, -5);
      const filename = `ecu_backup_${timestamp}.msq`;
      
      await invoke("save_tune", {
        filename,
        includeMetadata: true,
      });
      
      alert(`ECU tune saved successfully as ${filename}`);
    } catch (e) {
      console.error("Failed to save ECU tune:", e);
      alert(`Failed to save ECU tune: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setSavingTune(false);
    }
  };

  if (!isOpen || !mismatchInfo) return null;

  const handleSelectIni = async () => {
    if (!selectedIni) return;
    
    setSwitching(true);
    try {
      await invoke("update_project_ini", {
        iniPath: selectedIni,
        forceResync: true,
      });
      onSelectIni(selectedIni);
    } catch (e) {
      console.error("Failed to switch INI:", e);
    } finally {
      setSwitching(false);
    }
  };

  const hasExactMatch = mismatchInfo.matching_inis.some(
    (ini) => ini.match_type === "exact"
  );

  return (
    <div className="signature-mismatch-overlay" onClick={onClose}>
      <div
        className="signature-mismatch-dialog"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="signature-mismatch-header">
          <AlertTriangle className="warning-icon" size={24} />
          <h2>INI Signature Mismatch</h2>
          <button className="close-btn" onClick={onClose}>
            <X size={18} />
          </button>
        </div>

        <div className="signature-mismatch-content">
          <div className="signature-comparison">
            <div className="signature-row">
              <span className="label">ECU Signature:</span>
              <code className="signature ecu">{mismatchInfo.ecu_signature}</code>
            </div>
            <div className="signature-row">
              <span className="label">INI Signature:</span>
              <code className="signature ini">{mismatchInfo.ini_signature}</code>
            </div>
          </div>

          <p className="warning-text">
            {mismatchInfo.match_type === "partial"
              ? "The ECU signature partially matches the loaded INI file. This usually means the ECU firmware and INI file are from different versions of the same platform (e.g., different release dates). Using a partial match may work, but some features might not display correctly."
              : "The ECU signature does not match the loaded INI file. Using the wrong INI may cause read/write errors or incorrect data display."}
          </p>

          {/* Save ECU Tune First Option */}
          <div className="save-ecu-option">
            <button 
              className="save-ecu-btn" 
              onClick={handleSaveEcuTune}
              disabled={savingTune}
            >
              {savingTune ? (
                <>
                  <RefreshCw size={16} className="spinning" />
                  Saving ECU Tune...
                </>
              ) : (
                <>
                  <Save size={16} />
                  Download Current ECU Tune First
                </>
              )}
            </button>
            <p className="save-ecu-hint">
              <HelpCircle size={14} />
              Save your current ECU tune to your PC before switching INI files. This protects your existing tune.
            </p>
          </div>

          {!showOnlineSearch ? (
            <>
              {mismatchInfo.matching_inis.length > 0 ? (
                <div className="matching-inis-section">
                  <h3>
                    {hasExactMatch
                      ? "Matching INI files found:"
                      : "Similar INI files found:"}
                  </h3>
                  <div className="ini-list">
                    {mismatchInfo.matching_inis.map((ini) => (
                      <div
                        key={ini.path}
                        className={`ini-item ${selectedIni === ini.path ? "selected" : ""} ${ini.match_type}`}
                        onClick={() => setSelectedIni(ini.path)}
                      >
                        <div className="ini-info">
                          <span className="ini-name">{ini.name}</span>
                          <code className="ini-signature">{ini.signature}</code>
                        </div>
                        <div className="match-badge">
                          {ini.match_type === "exact" ? (
                            <span className="badge exact">
                              <Check size={12} /> Exact
                            </span>
                          ) : (
                            <span 
                              className="badge partial" 
                              title="Partial match: The signature prefix matches but version differs. May work with limited compatibility."
                            >
                              Partial
                              <HelpCircle size={12} className="help-icon" />
                            </span>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="no-matches">
                  <p>No matching INI files found in the local repository.</p>
                </div>
              )}
              
              <button 
                className="search-online-btn" 
                onClick={() => setShowOnlineSearch(true)}
              >
                <Globe size={16} />
                Search Online...
              </button>
            </>
          ) : (
            <div className="online-search-section">
              <div className="online-search-header">
                <h3>
                  <Globe size={16} />
                  Online INI Search
                </h3>
                <button 
                  className="back-btn"
                  onClick={() => setShowOnlineSearch(false)}
                >
                  ← Back to Local
                </button>
              </div>
              
              {hasInternet === null ? (
                <div className="checking-connectivity">
                  <RefreshCw size={16} className="spinning" />
                  Checking internet connectivity...
                </div>
              ) : hasInternet === false ? (
                <div className="no-internet">
                  <WifiOff size={24} />
                  <p>No Internet</p>
                  <p className="subtitle">Unable to reach GitHub. Check your connection and try again.</p>
                  <button className="retry-btn" onClick={checkConnectivity}>
                    <RefreshCw size={14} />
                    Retry
                  </button>
                </div>
              ) : searching ? (
                <div className="searching">
                  <RefreshCw size={20} className="spinning" />
                  <p>Searching Speeduino and rusEFI repositories...</p>
                </div>
              ) : searchError ? (
                <div className="search-error">
                  <p>Error: {searchError}</p>
                  <button className="retry-btn" onClick={searchOnline}>
                    <RefreshCw size={14} />
                    Retry
                  </button>
                </div>
              ) : onlineResults.length > 0 ? (
                <div className="online-results">
                  <p className="result-count">{onlineResults.length} INI files found</p>
                  <div className="ini-list">
                    {onlineResults.map((entry) => (
                      <div key={entry.download_url} className="ini-item online">
                        <div className="ini-info">
                          <span className="ini-name">{entry.name}</span>
                          <span className="ini-source">{entry.source}</span>
                          {entry.size && (
                            <span className="ini-size">
                              {(entry.size / 1024).toFixed(1)} KB
                            </span>
                          )}
                        </div>
                        <button
                          className="download-btn"
                          onClick={() => handleDownload(entry)}
                          disabled={downloading !== null}
                        >
                          {downloading === entry.download_url ? (
                            <RefreshCw size={14} className="spinning" />
                          ) : (
                            <Download size={14} />
                          )}
                          {downloading === entry.download_url ? "Downloading..." : "Download"}
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="no-online-results">
                  <Wifi size={24} />
                  <p>No matching INI files found online.</p>
                </div>
              )}
            </div>
          )}
        </div>

        <div className="signature-mismatch-actions">
          {selectedIni && !showOnlineSearch && (
            <button
              className="action-btn primary"
              onClick={handleSelectIni}
              disabled={switching}
            >
              {switching ? (
                <>
                  <RefreshCw size={16} className="spinning" />
                  Switching...
                </>
              ) : (
                <>
                  <Check size={16} />
                  Use Selected INI
                </>
              )}
            </button>
          )}
          <button 
            className="action-btn secondary" 
            onClick={onContinue}
            title="Continue with current INI despite mismatch (not recommended)"
          >
            Continue Anyway
          </button>
          <button className="action-btn cancel" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
