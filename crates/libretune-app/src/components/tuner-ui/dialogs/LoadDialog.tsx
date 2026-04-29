import { useState, useEffect, useCallback } from 'react';
import { FileText } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Dialog, Button } from '../../common';
import { DialogProps, TuneInfo } from './types';
import '../Dialogs.css';

interface LoadDialogProps extends DialogProps {
  onLoaded?: (tuneInfo: TuneInfo) => void;
}

export function LoadDialog({ isOpen, onClose, onLoaded }: LoadDialogProps) {
  const [tuneFiles, setTuneFiles] = useState<string[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      invoke<string[]>('list_tune_files')
        .then(setTuneFiles)
        .catch((e) => setError(String(e)));
    }
  }, [isOpen]);

  const handleLoad = useCallback(async (path: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const info = await invoke<TuneInfo>('load_tune', { path });
      onLoaded?.(info);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, [onClose, onLoaded]);

  const handleBrowse = useCallback(async () => {
    try {
      const selected = await open({
        title: 'Open Tune File',
        multiple: false,
        filters: [
          { name: 'Tune Files', extensions: ['msq', 'json'] },
          { name: 'MSQ Tune File', extensions: ['msq'] },
          { name: 'JSON Tune File', extensions: ['json'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      
      if (selected && typeof selected === 'string') {
        await handleLoad(selected);
      }
    } catch (e) {
      setError(String(e));
    }
  }, [handleLoad]);

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="Load Tune"
      size="lg"
      closeOnBackdrop={!isLoading}
    >
      <Dialog.Body>
        {error && <div className="dialog-error">{error}</div>}

        <div className="dialog-file-list">
          <div className="dialog-file-header">
            <span>Recent Tune Files</span>
            <Button variant="secondary" onClick={handleBrowse}>Browse...</Button>
          </div>

          {tuneFiles.length === 0 ? (
            <div className="dialog-empty">No tune files found in projects folder</div>
          ) : (
            <div className="dialog-files">
              {tuneFiles.map((file) => (
                <div
                  key={file}
                  className={`dialog-file-item ${selectedFile === file ? 'selected' : ''}`}
                  onClick={() => setSelectedFile(file)}
                  onDoubleClick={() => handleLoad(file)}
                >
                  <span className="dialog-file-icon"><FileText size={14} /></span>
                  <div className="dialog-file-info">
                    <span className="dialog-file-name">{file.split('/').pop()}</span>
                    <span className="dialog-file-path">{file}</span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose} disabled={isLoading}>Cancel</Button>
        <Button
          variant="primary"
          onClick={() => selectedFile && handleLoad(selectedFile)}
          disabled={isLoading || !selectedFile}
        >
          {isLoading ? 'Loading...' : 'Load'}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
