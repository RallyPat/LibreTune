import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import './ImportDashboardDialog.css';

interface DashConflictInfo {
  file_name: string;
  has_conflict: boolean;
  suggested_name: string | null;
}

interface DashFileInfo {
  name: string;
  path: string;
  category: string;
}

interface DashImportResult {
  source_path: string;
  success: boolean;
  error: string | null;
  file_info: DashFileInfo | null;
}

interface FileEntry {
  sourcePath: string;
  fileName: string;
  status: 'pending' | 'checking' | 'conflict' | 'ready' | 'importing' | 'success' | 'error';
  conflict?: DashConflictInfo;
  renameTo?: string;
  error?: string;
  result?: DashFileInfo;
}

interface ImportDashboardDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onImportComplete: (imported: DashFileInfo[]) => void;
}

export const ImportDashboardDialog: React.FC<ImportDashboardDialogProps> = ({
  isOpen,
  onClose,
  onImportComplete,
}) => {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [isImporting, setIsImporting] = useState(false);
  const [importedFiles, setImportedFiles] = useState<DashFileInfo[]>([]);

  // Check conflicts for all files when they're added
  const checkConflicts = useCallback(async (entries: FileEntry[]) => {
    const updatedEntries = await Promise.all(
      entries.map(async (entry) => {
        if (entry.status !== 'pending') return entry;
        
        const updated = { ...entry, status: 'checking' as const };
        
        try {
          const conflict = await invoke<DashConflictInfo>('check_dash_conflict', {
            fileName: entry.fileName,
          });
          
          if (conflict.has_conflict) {
            return {
              ...updated,
              status: 'conflict' as const,
              conflict,
              renameTo: conflict.suggested_name || entry.fileName,
            };
          } else {
            return { ...updated, status: 'ready' as const };
          }
        } catch (e) {
          return {
            ...updated,
            status: 'error' as const,
            error: `Failed to check conflict: ${e}`,
          };
        }
      })
    );
    
    setFiles(updatedEntries);
  }, []);

  // Handle file selection
  const handleSelectFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: 'Dashboard Files',
            extensions: ['dash', 'xml'],
          },
        ],
      });
      
      if (!selected || (Array.isArray(selected) && selected.length === 0)) {
        return;
      }
      
      const paths = Array.isArray(selected) ? selected : [selected];
      
      const newEntries: FileEntry[] = paths.map((path) => ({
        sourcePath: path,
        fileName: path.split(/[/\\]/).pop() || 'unknown',
        status: 'pending' as const,
      }));
      
      // Check for duplicates already in the list
      const existingPaths = new Set(files.map(f => f.sourcePath));
      const uniqueEntries = newEntries.filter(e => !existingPaths.has(e.sourcePath));
      
      if (uniqueEntries.length === 0) {
        return;
      }
      
      const allEntries = [...files, ...uniqueEntries];
      setFiles(allEntries);
      
      // Check conflicts for new entries
      await checkConflicts(allEntries);
    } catch (e) {
      console.error('Error selecting files:', e);
    }
  };

  // Remove a file from the list
  const handleRemoveFile = (sourcePath: string) => {
    setFiles(prev => prev.filter(f => f.sourcePath !== sourcePath));
  };

  // Update rename value for a file
  const handleRenameChange = (sourcePath: string, newName: string) => {
    setFiles(prev => prev.map(f => 
      f.sourcePath === sourcePath ? { ...f, renameTo: newName } : f
    ));
  };

  // Re-check conflict after rename
  const handleRecheck = async (sourcePath: string) => {
    const file = files.find(f => f.sourcePath === sourcePath);
    if (!file || !file.renameTo) return;
    
    setFiles(prev => prev.map(f => 
      f.sourcePath === sourcePath ? { ...f, status: 'checking' } : f
    ));
    
    try {
      const conflict = await invoke<DashConflictInfo>('check_dash_conflict', {
        fileName: file.renameTo,
      });
      
      setFiles(prev => prev.map(f => 
        f.sourcePath === sourcePath
          ? {
              ...f,
              status: conflict.has_conflict ? 'conflict' : 'ready',
              conflict: conflict.has_conflict ? conflict : undefined,
            }
          : f
      ));
    } catch (e) {
      setFiles(prev => prev.map(f => 
        f.sourcePath === sourcePath
          ? { ...f, status: 'error', error: `Failed to check: ${e}` }
          : f
      ));
    }
  };

  // Mark a file to be overwritten
  const handleOverwrite = (sourcePath: string) => {
    setFiles(prev => prev.map(f => 
      f.sourcePath === sourcePath
        ? { ...f, status: 'ready', renameTo: undefined }
        : f
    ));
  };

  // Overwrite all conflicts
  const handleOverwriteAll = () => {
    setFiles(prev => prev.map(f => 
      f.status === 'conflict'
        ? { ...f, status: 'ready', renameTo: undefined }
        : f
    ));
  };

  // Skip all conflicts (remove from list)
  const handleSkipAll = () => {
    setFiles(prev => prev.filter(f => f.status !== 'conflict'));
  };

  // Import all ready files
  const handleImport = async () => {
    const readyFiles = files.filter(f => f.status === 'ready');
    if (readyFiles.length === 0) return;
    
    setIsImporting(true);
    const results: DashFileInfo[] = [];
    
    for (const file of readyFiles) {
      setFiles(prev => prev.map(f => 
        f.sourcePath === file.sourcePath ? { ...f, status: 'importing' } : f
      ));
      
      try {
        const result = await invoke<DashImportResult>('import_dash_file', {
          sourcePath: file.sourcePath,
          renameTo: file.renameTo || null,
          overwrite: !file.renameTo, // If we're using original name on conflict, overwrite
        });
        
        if (result.success && result.file_info) {
          results.push(result.file_info);
          setFiles(prev => prev.map(f => 
            f.sourcePath === file.sourcePath
              ? { ...f, status: 'success', result: result.file_info! }
              : f
          ));
        } else {
          setFiles(prev => prev.map(f => 
            f.sourcePath === file.sourcePath
              ? { ...f, status: 'error', error: result.error || 'Unknown error' }
              : f
          ));
        }
      } catch (e) {
        setFiles(prev => prev.map(f => 
          f.sourcePath === file.sourcePath
            ? { ...f, status: 'error', error: `${e}` }
            : f
        ));
      }
    }
    
    setImportedFiles(prev => [...prev, ...results]);
    setIsImporting(false);
  };

  // Close and report results
  const handleClose = () => {
    if (importedFiles.length > 0) {
      onImportComplete(importedFiles);
    }
    setFiles([]);
    setImportedFiles([]);
    onClose();
  };

  // Reset state when dialog opens
  useEffect(() => {
    if (isOpen) {
      setFiles([]);
      setImportedFiles([]);
      setIsImporting(false);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const readyCount = files.filter(f => f.status === 'ready').length;
  const conflictCount = files.filter(f => f.status === 'conflict').length;
  const successCount = files.filter(f => f.status === 'success').length;

  return (
    <div className="import-dashboard-overlay" onClick={handleClose}>
      <div className="import-dashboard-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="import-dashboard-header">
          <h2>Import Dashboards</h2>
          <button className="close-btn" onClick={handleClose}>×</button>
        </div>

        <div className="import-dashboard-content">
          {/* File selection area */}
          <div className="file-selection-area">
            <button className="select-files-btn" onClick={handleSelectFiles}>
              <span className="icon">📁</span>
              Select Dashboard Files
            </button>
            <span className="hint">.dash and .xml files supported</span>
          </div>

          {/* File list */}
          {files.length > 0 && (
            <div className="file-list">
              <div className="file-list-header">
                <span>Files to Import ({files.length})</span>
                {conflictCount > 0 && (
                  <div className="bulk-actions">
                    <button className="bulk-btn" onClick={handleOverwriteAll}>
                      Overwrite All Conflicts
                    </button>
                    <button className="bulk-btn skip" onClick={handleSkipAll}>
                      Skip All Conflicts
                    </button>
                  </div>
                )}
              </div>

              <div className="file-entries">
                {files.map((file) => (
                  <div key={file.sourcePath} className={`file-entry status-${file.status}`}>
                    <div className="file-info">
                      <span className="status-icon">
                        {file.status === 'checking' && '⏳'}
                        {file.status === 'pending' && '⏳'}
                        {file.status === 'conflict' && '⚠️'}
                        {file.status === 'ready' && '✓'}
                        {file.status === 'importing' && '⏳'}
                        {file.status === 'success' && '✅'}
                        {file.status === 'error' && '❌'}
                      </span>
                      <span className="file-name" title={file.sourcePath}>
                        {file.fileName}
                      </span>
                      {file.status !== 'success' && file.status !== 'importing' && (
                        <button
                          className="remove-btn"
                          onClick={() => handleRemoveFile(file.sourcePath)}
                          title="Remove from list"
                        >
                          ×
                        </button>
                      )}
                    </div>

                    {/* Conflict resolution */}
                    {file.status === 'conflict' && (
                      <div className="conflict-resolution">
                        <span className="conflict-msg">
                          File already exists
                        </span>
                        <div className="rename-row">
                          <input
                            type="text"
                            value={file.renameTo || ''}
                            onChange={(e) => handleRenameChange(file.sourcePath, e.target.value)}
                            placeholder="New filename"
                          />
                          <button onClick={() => handleRecheck(file.sourcePath)}>
                            Rename
                          </button>
                          <button onClick={() => handleOverwrite(file.sourcePath)}>
                            Overwrite
                          </button>
                        </div>
                      </div>
                    )}

                    {/* Error message */}
                    {file.status === 'error' && file.error && (
                      <div className="error-msg">{file.error}</div>
                    )}

                    {/* Success message */}
                    {file.status === 'success' && file.result && (
                      <div className="success-msg">
                        Imported as {file.result.name}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Empty state */}
          {files.length === 0 && (
            <div className="empty-state">
              <p>Select dashboard files to import them into LibreTune.</p>
              <p className="hint">They will be copied to your dashboards folder and available for selection.</p>
            </div>
          )}
        </div>

        <div className="import-dashboard-footer">
          <div className="status-summary">
            {successCount > 0 && (
              <span className="success-count">✅ {successCount} imported</span>
            )}
            {readyCount > 0 && (
              <span className="ready-count">✓ {readyCount} ready</span>
            )}
            {conflictCount > 0 && (
              <span className="conflict-count">⚠️ {conflictCount} conflicts</span>
            )}
          </div>
          <div className="footer-buttons">
            <button className="cancel-btn" onClick={handleClose}>
              {successCount > 0 ? 'Done' : 'Cancel'}
            </button>
            {readyCount > 0 && (
              <button
                className="import-btn"
                onClick={handleImport}
                disabled={isImporting}
              >
                {isImporting ? 'Importing...' : `Import ${readyCount} File${readyCount !== 1 ? 's' : ''}`}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default ImportDashboardDialog;
