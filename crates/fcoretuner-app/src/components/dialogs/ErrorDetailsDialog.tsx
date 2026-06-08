import { useState, useCallback } from 'react';
import './ErrorDetailsDialog.css';

interface ErrorDetailsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  message: string;
  details?: string;
}

export default function ErrorDetailsDialog({
  isOpen,
  onClose,
  title,
  message,
  details,
}: ErrorDetailsDialogProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    const fullError = `FCoreTuner 错误报告
========================
标题: ${title}
消息: ${message}
${details ? `\n详细信息:\n${details}` : ''}
========================
日期: ${new Date().toISOString()}
平台: ${navigator.platform}
UserAgent: ${navigator.userAgent}
`;

    try {
      await navigator.clipboard.writeText(fullError);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy to clipboard:', err);
    }
  }, [title, message, details]);

  if (!isOpen) return null;

  return (
    <div className="error-dialog-overlay" onClick={onClose}>
      <div className="error-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="error-dialog-header">
          <span className="error-icon">⚠</span>
          <h2>{title}</h2>
          <button className="error-close-btn" onClick={onClose}>×</button>
        </div>
        
        <div className="error-dialog-content">
          <p className="error-message">{message}</p>
          
          {details && (
            <div className="error-details">
              <div className="error-details-header">
                <span>错误详情</span>
                <button 
                  className="copy-btn" 
                  onClick={handleCopy}
                  title="复制错误详情用于错误报告"
                >
                  {copied ? '✓ 已复制!' : '📋 复制用于错误报告'}
                </button>
              </div>
              <pre className="error-details-content">{details}</pre>
            </div>
          )}
        </div>
        
        <div className="error-dialog-footer">
          <p className="error-help-text">
            如果此错误持续存在，请附上上面的错误详细信息提交错误报告。
          </p>
          <button className="error-ok-btn" onClick={onClose}>OK</button>
        </div>
      </div>
    </div>
  );
}

// Hook for managing error dialog state
export function useErrorDialog() {
  const [isOpen, setIsOpen] = useState(false);
  const [errorInfo, setErrorInfo] = useState({
    title: '错误',
    message: '',
    details: '',
  });

  const showError = useCallback((title: string, message: string, details?: string) => {
    setErrorInfo({ title, message, details: details || '' });
    setIsOpen(true);
  }, []);

  const hideError = useCallback(() => {
    setIsOpen(false);
  }, []);

  return {
    isOpen,
    errorInfo,
    showError,
    hideError,
  };
}
