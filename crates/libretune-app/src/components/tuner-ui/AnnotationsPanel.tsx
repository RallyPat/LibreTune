/**
 * Tune Annotations Panel
 *
 * Display and manage annotations/notes on tune constants and table cells.
 */
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './AnnotationsPanel.css';

interface TuneAnnotation {
  text: string;
  author: string | null;
  created: string;
  modified: string | null;
  tag: string | null;
}

interface Props {
  /** If provided, filter to annotations for this table */
  tableName?: string;
  /** Currently selected cell (for quick-add) */
  selectedCell?: { row: number; col: number };
}

const TAG_COLORS: Record<string, string> = {
  Info: '#4fc3f7',
  Warning: '#ffa726',
  Critical: '#ef5350',
  Success: '#66bb6a',
  Todo: '#ab47bc',
};

export function AnnotationsPanel({ tableName, selectedCell }: Props) {
  const [annotations, setAnnotations] = useState<Record<string, TuneAnnotation>>({});
  const [showAdd, setShowAdd] = useState(false);
  const [newKey, setNewKey] = useState('');
  const [newText, setNewText] = useState('');
  const [newTag, setNewTag] = useState('info');
  const [loading, setLoading] = useState(false);

  const fetchAnnotations = useCallback(async () => {
    try {
      if (tableName) {
        const result = await invoke<[string, TuneAnnotation][]>('get_table_annotations', {
          tableName,
        });
        const map: Record<string, TuneAnnotation> = {};
        for (const [k, a] of result) {
          map[k] = a;
        }
        setAnnotations(map);
      } else {
        const result = await invoke<Record<string, TuneAnnotation>>('get_all_annotations');
        setAnnotations(result);
      }
    } catch (_e) {
      // Silently fail if no tune loaded
    }
  }, [tableName]);

  useEffect(() => {
    fetchAnnotations();
  }, [fetchAnnotations]);

  // Set initial key from selected cell
  useEffect(() => {
    if (selectedCell && tableName) {
      setNewKey(`${tableName}:${selectedCell.row}:${selectedCell.col}`);
    } else if (tableName) {
      setNewKey(tableName);
    }
  }, [selectedCell, tableName]);

  const handleAdd = useCallback(async () => {
    if (!newKey.trim() || !newText.trim()) return;
    setLoading(true);
    try {
      await invoke('set_annotation', {
        key: newKey.trim(),
        text: newText.trim(),
        tag: newTag,
      });
      setShowAdd(false);
      setNewText('');
      fetchAnnotations();
    } catch (_e) {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [newKey, newText, newTag, fetchAnnotations]);

  const handleDelete = useCallback(async (key: string) => {
    try {
      await invoke('delete_annotation', { key });
      fetchAnnotations();
    } catch (_e) {
      // ignore
    }
  }, [fetchAnnotations]);

  const entries = Object.entries(annotations);
  const sortedEntries = entries.sort((a, b) => {
    // Sort by tag priority: Critical > Warning > Todo > Info > Success
    const tagOrder = ['Critical', 'Warning', 'Todo', 'Info', 'Success'];
    const aIdx = a[1].tag ? tagOrder.indexOf(a[1].tag) : 99;
    const bIdx = b[1].tag ? tagOrder.indexOf(b[1].tag) : 99;
    if (aIdx !== bIdx) return aIdx - bIdx;
    return a[0].localeCompare(b[0]);
  });

  return (
    <div className="annotations-panel">
      <div className="annotations-header">
        <span className="annotations-title">
          Notes {entries.length > 0 && `(${entries.length})`}
        </span>
        <button
          className="annotations-add-btn"
          onClick={() => setShowAdd(!showAdd)}
        >
          {showAdd ? '−' : '+'}
        </button>
      </div>

      {showAdd && (
        <div className="annotation-add-form">
          <input
            type="text"
            className="annotation-key-input"
            placeholder="Key (e.g. veTable1:3:5)"
            value={newKey}
            onChange={e => setNewKey(e.target.value)}
          />
          <textarea
            className="annotation-text-input"
            placeholder="Your note..."
            value={newText}
            onChange={e => setNewText(e.target.value)}
            rows={2}
          />
          <div className="annotation-add-actions">
            <select value={newTag} onChange={e => setNewTag(e.target.value)}>
              <option value="info">Info</option>
              <option value="warning">Warning</option>
              <option value="critical">Critical</option>
              <option value="success">Success</option>
              <option value="todo">Todo</option>
            </select>
            <button
              className="annotation-save-btn"
              onClick={handleAdd}
              disabled={!newKey.trim() || !newText.trim() || loading}
            >
              Save
            </button>
          </div>
        </div>
      )}

      <div className="annotations-list">
        {sortedEntries.length === 0 && !showAdd && (
          <div className="annotations-empty">No annotations yet</div>
        )}
        {sortedEntries.map(([key, ann]) => (
          <div key={key} className="annotation-item">
            <div className="annotation-item-header">
              {ann.tag && (
                <span
                  className="annotation-tag"
                  style={{ background: TAG_COLORS[ann.tag] || '#666' }}
                >
                  {ann.tag}
                </span>
              )}
              <span className="annotation-key">{key}</span>
              <button
                className="annotation-delete-btn"
                onClick={() => handleDelete(key)}
              >
                ×
              </button>
            </div>
            <div className="annotation-text">{ann.text}</div>
            <div className="annotation-meta">
              {new Date(ann.created).toLocaleDateString()}
              {ann.author && ` · ${ann.author}`}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
