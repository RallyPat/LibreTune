//! Tune Comparison Dialog
//!
//! Shown when the tune on ECU differs from the tune in the project.
//! Allows user to choose which tune to use.

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, HardDrive, FileText, Loader } from "lucide-react";
import "./TuneComparisonDialog.css";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  onUseProjectTune: () => void;
  onUseEcuTune: () => void;
}

export default function TuneComparisonDialog({
  isOpen,
  onClose,
  onUseProjectTune,
  onUseEcuTune,
}: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleUseProjectTune = async () => {
    setLoading(true);
    setError(null);
    try {
      await invoke("write_project_tune_to_ecu");
      onUseProjectTune();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleUseEcuTune = async () => {
    setLoading(true);
    setError(null);
    try {
      // Save the current tune (ECU tune) to the project file
      await invoke("save_tune_to_project");
      onUseEcuTune();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="dialog-overlay tune-comparison-overlay" onClick={onClose}>
      <div className="dialog tune-comparison-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <div className="dialog-header-icon">
            <AlertTriangle size={24} />
          </div>
          <h2>检测到调教不一致</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          <p>
            ECU 上的调教与您项目中的调教不同。
            请选择要使用的调教:
          </p>
          
          {error && (
            <div className="dialog-error">
              {error}
            </div>
          )}
          
          <div className="tune-choice-buttons">
            <button
              className="tune-choice-button tune-choice-project"
              onClick={handleUseProjectTune}
              disabled={loading}
            >
              <div className="tune-choice-icon">
                <FileText size={32} />
              </div>
              <div className="tune-choice-content">
                <h3>使用项目调教</h3>
                <p>从项目文件加载调教并写入 ECU</p>
              </div>
              {loading && <Loader className="tune-choice-loader" size={20} />}
            </button>
            
            <button
              className="tune-choice-button tune-choice-ecu"
              onClick={handleUseEcuTune}
              disabled={loading}
            >
              <div className="tune-choice-icon">
                <HardDrive size={32} />
              </div>
              <div className="tune-choice-content">
                <h3>使用 ECU 调教</h3>
                <p>保留 ECU 上的当前调教并更新您的项目文件</p>
              </div>
              {loading && <Loader className="tune-choice-loader" size={20} />}
            </button>
          </div>
        </div>
        
        <div className="dialog-footer">
          <button className="dialog-button-secondary" onClick={onClose} disabled={loading}>
            取消
          </button>
        </div>
      </div>
    </div>
  );
}
