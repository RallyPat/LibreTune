import { useState, useCallback } from 'react';
import { AlertTriangle, Check, Clipboard } from 'lucide-react';
import { Dialog, Button } from '../common';
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

  const titleNode = (
    <>
      <span className="error-icon" aria-hidden="true"><AlertTriangle size={18} /></span>
      {title}
    </>
  );

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title={titleNode}
      size="md"
      className="error-dialog"
    >
      <Dialog.Body className="error-dialog-body">
        <p className="error-message">{message}</p>

        {details && (
          <div className="error-details">
            <div className="error-details-header">
              <span>Error Details</span>
              <button
                type="button"
                className="error-copy-btn"
                onClick={handleCopy}
                title="Copy error details for bug report"
              >
                {copied ? <><Check size={14} /> Copied!</> : <><Clipboard size={14} /> Copy for Bug Report</>}
              </button>
            </div>
            <pre className="error-details-content">{details}</pre>
          </div>
        )}

        <p className="error-help-text">
          If this error persists, please file a bug report with the error details above.
        </p>
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose}>Close</Button>
      </Dialog.Footer>
    </Dialog>
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
