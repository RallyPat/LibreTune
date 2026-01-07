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
    const fullError = `LibreTune Error Report
========================
Title: ${title}
Message: ${message}
${details ? `\nDetails:\n${details}` : ''}
========================
Date: ${new Date().toISOString()}
Platform: ${navigator.platform}
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
                <span>Error Details</span>
                <button 
                  className="copy-btn" 
                  onClick={handleCopy}
                  title="Copy error details for bug report"
                >
                  {copied ? '✓ Copied!' : '📋 Copy for Bug Report'}
                </button>
              </div>
              <pre className="error-details-content">{details}</pre>
            </div>
          )}
        </div>
        
        <div className="error-dialog-footer">
          <p className="error-help-text">
            If this error persists, please file a bug report with the error details above.
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
    title: 'Error',
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
