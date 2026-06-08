import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './TuneMismatchDialog.css';

export interface TuneMismatchInfo {
  ecu_pages: number[];
  project_pages: number[];
  diff_pages: number[];
}

interface TuneMismatchDialogProps {
  isOpen: boolean;
  mismatchInfo: TuneMismatchInfo | null;
  onClose: () => void;
  onUseProject: () => void;
  onUseECU: () => void;
}

export default function TuneMismatchDialog({
  isOpen,
  mismatchInfo,
  onClose,
  onUseProject,
  onUseECU,
}: TuneMismatchDialogProps) {
  const [isLoading, setIsLoading] = useState(false);

  if (!isOpen || !mismatchInfo) return null;

  const handleUseProject = async () => {
    setIsLoading(true);
    try {
      await invoke('use_project_tune');
      onUseProject();
      onClose();
    } catch (err) {
      console.error('Failed to load project tune:', err);
      alert(`Failed to load project tune: ${err}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleUseECU = async () => {
    setIsLoading(true);
    try {
      await invoke('use_ecu_tune');
      onUseECU();
      onClose();
    } catch (err) {
      console.error('Failed to use ECU tune:', err);
      alert(`Failed to use ECU tune: ${err}`);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog tune-mismatch-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>检测到调教不一致</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>

        <div className="dialog-content">
          <div className="tune-mismatch-warning">
            <p>
              <strong>ECU 上的调教与您项目中的调教不同。</strong>
            </p>
            <p>
              ECU 已加载 {mismatchInfo.ecu_pages.length} 页，而您的项目有 {mismatchInfo.project_pages.length} 页。
              {mismatchInfo.diff_pages.length > 0 && (
                <> 其中 {mismatchInfo.diff_pages.length} 页存在差异。</>
              )}
            </p>
          </div>

          <div className="tune-mismatch-options">
            <div className="tune-option">
              <h3>使用项目调教</h3>
              <p>从项目文件加载调教。这将用您保存的项目数据覆盖 ECU 调教。</p>
              <button
                onClick={handleUseProject}
                disabled={isLoading}
                className="dialog-primary"
              >
                {isLoading ? '加载中...' : '使用项目调教'}
              </button>
            </div>

            <div className="tune-option">
              <h3>使用 ECU 调教</h3>
              <p>保留 ECU 上的当前调教。您的项目将更新以匹配 ECU。</p>
              <button
                onClick={handleUseECU}
                disabled={isLoading}
              >
                {isLoading ? '加载中...' : '使用 ECU 调教'}
              </button>
            </div>
          </div>
        </div>

        <div className="dialog-footer">
          <button onClick={onClose} disabled={isLoading}>
            取消
          </button>
        </div>
      </div>
    </div>
  );
}

