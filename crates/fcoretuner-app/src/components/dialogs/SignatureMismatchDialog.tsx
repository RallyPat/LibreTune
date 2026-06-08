//! Signature Mismatch Dialog
//!
//! Shown when the ECU signature doesn't match the loaded INI file.
//! Allows user to select a matching INI, continue anyway, or cancel.

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Check, X, Download, RefreshCw, Globe, Wifi, WifiOff } from "lucide-react";
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
          <h2>INI 签名不匹配</h2>
          <button className="close-btn" onClick={onClose}>
            <X size={18} />
          </button>
        </div>

        <div className="signature-mismatch-content">
          <div className="signature-comparison">
            <div className="signature-row">
              <span className="label">ECU 签名:</span>
              <code className="signature ecu">{mismatchInfo.ecu_signature}</code>
            </div>
            <div className="signature-row">
              <span className="label">INI 签名:</span>
              <code className="signature ini">{mismatchInfo.ini_signature}</code>
            </div>
          </div>

          <p className="warning-text">
            {mismatchInfo.match_type === "partial"
              ? "ECU 签名与加载的 INI 文件部分匹配。这可能表示版本差异。"
              : "ECU 签名与加载的 INI 文件不匹配。使用错误的 INI 可能导致读写错误或数据显示不正确。"}
          </p>

          {!showOnlineSearch ? (
            <>
              {mismatchInfo.matching_inis.length > 0 ? (
                <div className="matching-inis-section">
                  <h3>
                    {hasExactMatch
                      ? "找到匹配的 INI 文件:"
                      : "找到相似的 INI 文件:"}
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
                              <Check size={12} /> 完全匹配
                            </span>
                          ) : (
                            <span className="badge partial">部分匹配</span>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="no-matches">
                  <p>本地仓库中未找到匹配的 INI 文件。</p>
                </div>
              )}
              
              <button 
                className="search-online-btn" 
                onClick={() => setShowOnlineSearch(true)}
              >
                <Globe size={16} />
                在线搜索...
              </button>
            </>
          ) : (
            <div className="online-search-section">
              <div className="online-search-header">
                <h3>
                  <Globe size={16} />
                  在线 INI 搜索
                </h3>
                <button 
                  className="back-btn"
                  onClick={() => setShowOnlineSearch(false)}
                >
                  ← 返回本地
                </button>
              </div>
              
              {hasInternet === null ? (
                  <div className="checking-connectivity">
                  <RefreshCw size={16} className="spinning" />
                  检查网络连接...
                </div>
              ) : hasInternet === false ? (
                <div className="no-internet">
                  <WifiOff size={24} />
                  <p>无网络连接</p>
                  <p className="subtitle">无法访问 GitHub。请检查您的网络连接后重试。</p>
                  <button className="retry-btn" onClick={checkConnectivity}>
                    <RefreshCw size={14} />
                    重试
                  </button>
                </div>
              ) : searching ? (
                <div className="searching">
                  <RefreshCw size={20} className="spinning" />
                  <p>正在搜索 Speeduino 和 rusEFI 仓库...</p>
                </div>
              ) : searchError ? (
                <div className="search-error">
                  <p>错误: {searchError}</p>
                  <button className="retry-btn" onClick={searchOnline}>
                    <RefreshCw size={14} />
                    重试
                  </button>
                </div>
              ) : onlineResults.length > 0 ? (
                <div className="online-results">
                  <p className="result-count">找到 {onlineResults.length} 个 INI 文件</p>
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
                          {downloading === entry.download_url ? "下载中..." : "下载"}
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="no-online-results">
                  <Wifi size={24} />
                  <p>在线未找到匹配的 INI 文件。</p>
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
                  切换中...
                </>
              ) : (
                <>
                  <Check size={16} />
                  使用选中的 INI
                </>
              )}
            </button>
          )}
          <button className="action-btn secondary" onClick={onContinue}>
            继续使用
          </button>
          <button className="action-btn cancel" onClick={onClose}>
            取消
          </button>
        </div>
      </div>
    </div>
  );
}
