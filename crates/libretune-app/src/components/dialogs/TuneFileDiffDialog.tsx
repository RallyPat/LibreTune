/**
 * Tune File Diff Dialog
 *
 * Compare two tune files from disk side-by-side, with detailed diff
 * visualization and cherry-pick merge capability.
 */
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { GitCompare, FolderOpen, GitMerge, Check, X } from 'lucide-react';
import { Dialog, Button } from '../common';
import './TuneFileDiffDialog.css';

interface TuneValue {
  Scalar?: number;
  Array?: number[];
  String?: string;
  Bool?: boolean;
}

interface TuneDifference {
  name: string;
  value_a: TuneValue | null;
  value_b: TuneValue | null;
  numeric_diff: number | null;
  percent_diff: number | null;
}

interface TuneDiff {
  differences: TuneDifference[];
  signature_match: boolean;
  signature_a: string;
  signature_b: string;
  total_constants_a: number;
  total_constants_b: number;
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
  currentTunePath?: string;
}

function formatValue(val: TuneValue | null): string {
  if (!val) return '(missing)';
  if (val.Scalar !== undefined) return val.Scalar.toFixed(2);
  if (val.Bool !== undefined) return val.Bool ? 'true' : 'false';
  if (val.String !== undefined) return `"${val.String}"`;
  if (val.Array !== undefined) {
    if (val.Array.length <= 8) return `[${val.Array.map(v => v.toFixed(1)).join(', ')}]`;
    return `[${val.Array.length} values]`;
  }
  return '?';
}

function severityClass(diff: TuneDifference): string {
  if (!diff.percent_diff) return 'diff-added';
  const pct = Math.abs(diff.percent_diff);
  if (pct >= 20) return 'diff-high';
  if (pct >= 5) return 'diff-medium';
  return 'diff-low';
}

type SortField = 'name' | 'diff' | 'pct';
type FilterMode = 'all' | 'scalar' | 'added' | 'removed';

export default function TuneFileDiffDialog({ isOpen, onClose, currentTunePath }: Props) {
  const [pathA, setPathA] = useState(currentTunePath || '');
  const [pathB, setPathB] = useState('');
  const [diff, setDiff] = useState<TuneDiff | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sortField, setSortField] = useState<SortField>('name');
  const [sortAsc, setSortAsc] = useState(true);
  const [filter, setFilter] = useState<FilterMode>('all');
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [merging, setMerging] = useState(false);
  const [mergeResult, setMergeResult] = useState<string | null>(null);

  useEffect(() => {
    if (currentTunePath) setPathA(currentTunePath);
  }, [currentTunePath]);

  const handleCompare = useCallback(async () => {
    if (!pathA || !pathB) return;
    setLoading(true);
    setError(null);
    setDiff(null);
    setSelected(new Set());
    setMergeResult(null);

    try {
      const result = await invoke<TuneDiff>('compare_tune_files', { pathA, pathB });
      setDiff(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [pathA, pathB]);

  const handleBrowse = useCallback(async (setPath: (s: string) => void) => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const path = await open({
        filters: [
          { name: 'Tune Files', extensions: ['msq', 'json'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      if (path && typeof path === 'string') {
        setPath(path);
      }
    } catch (_e) { /* user cancelled */ }
  }, []);

  const getFilteredDiffs = useCallback((): TuneDifference[] => {
    if (!diff) return [];
    let items = diff.differences;

    switch (filter) {
      case 'scalar':
        items = items.filter(d => d.numeric_diff !== null);
        break;
      case 'added':
        items = items.filter(d => d.value_a === null);
        break;
      case 'removed':
        items = items.filter(d => d.value_b === null);
        break;
    }

    if (search) {
      const q = search.toLowerCase();
      items = items.filter(d => d.name.toLowerCase().includes(q));
    }

    items = [...items].sort((a, b) => {
      let cmp = 0;
      switch (sortField) {
        case 'name':
          cmp = a.name.localeCompare(b.name);
          break;
        case 'diff':
          cmp = (Math.abs(a.numeric_diff || 0)) - (Math.abs(b.numeric_diff || 0));
          break;
        case 'pct':
          cmp = (Math.abs(a.percent_diff || 0)) - (Math.abs(b.percent_diff || 0));
          break;
      }
      return sortAsc ? cmp : -cmp;
    });

    return items;
  }, [diff, filter, search, sortField, sortAsc]);

  const handleMerge = useCallback(async () => {
    if (selected.size === 0) return;
    setMerging(true);
    setMergeResult(null);

    try {
      const merged = await invoke<number>('merge_from_tune', {
        sourcePath: pathB,
        constantNames: Array.from(selected),
      });
      setMergeResult(`Successfully merged ${merged} constants`);
      handleCompare();
    } catch (e) {
      setMergeResult(`Merge failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setMerging(false);
    }
  }, [selected, pathB, handleCompare]);

  const toggleSelect = useCallback((name: string) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }, []);

  const filtered = getFilteredDiffs();

  const selectAll = useCallback(() => {
    if (!diff) return;
    if (selected.size === filtered.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(filtered.map(d => d.name)));
    }
  }, [diff, selected, filtered]);

  const handleSort = (field: SortField) => {
    if (sortField === field) setSortAsc(!sortAsc);
    else { setSortField(field); setSortAsc(true); }
  };

  const titleNode = (
    <>
      <GitCompare size={18} /> Compare Tune Files
    </>
  );

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title={titleNode}
      size="xl"
      closeOnBackdrop={!loading && !merging}
    >
      <Dialog.Body className="tune-diff-content">
        <div className="tune-diff-files">
          <div className="tune-diff-file-row">
            <label>Base Tune (A):</label>
            <input
              type="text"
              value={pathA}
              onChange={e => setPathA(e.target.value)}
              placeholder="Path to base tune..."
            />
            <Button
              variant="secondary"
              onClick={() => handleBrowse(setPathA)}
              leadingIcon={<FolderOpen size={14} />}
            >
              Browse...
            </Button>
          </div>
          <div className="tune-diff-file-row">
            <label>Compare Tune (B):</label>
            <input
              type="text"
              value={pathB}
              onChange={e => setPathB(e.target.value)}
              placeholder="Path to comparison tune..."
            />
            <Button
              variant="secondary"
              onClick={() => handleBrowse(setPathB)}
              leadingIcon={<FolderOpen size={14} />}
            >
              Browse...
            </Button>
          </div>
          <Button
            variant="primary"
            onClick={handleCompare}
            disabled={!pathA || !pathB || loading}
          >
            {loading ? 'Comparing...' : 'Compare'}
          </Button>
        </div>

        {error && <div className="tune-diff-error">{error}</div>}

        {diff && (
          <div className="tune-diff-results">
            <div className="tune-diff-summary">
              <div className="tune-diff-stat">
                <span className="stat-label">Signature Match</span>
                <span className={`stat-value ${diff.signature_match ? 'match' : 'mismatch'}`} style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                  {diff.signature_match ? <><Check size={14} /> Yes</> : <><X size={14} /> No</>}
                </span>
              </div>
              <div className="tune-diff-stat">
                <span className="stat-label">Differences</span>
                <span className="stat-value">{diff.differences.length}</span>
              </div>
              <div className="tune-diff-stat">
                <span className="stat-label">Constants A</span>
                <span className="stat-value">{diff.total_constants_a}</span>
              </div>
              <div className="tune-diff-stat">
                <span className="stat-label">Constants B</span>
                <span className="stat-value">{diff.total_constants_b}</span>
              </div>
            </div>

            {diff.differences.length === 0 ? (
              <div className="tune-diff-identical">Tunes are identical!</div>
            ) : (
              <>
                <div className="tune-diff-toolbar">
                  <div className="tune-diff-filters">
                    {(['all', 'scalar', 'added', 'removed'] as FilterMode[]).map(f => (
                      <button
                        key={f}
                        className={`filter-btn ${filter === f ? 'active' : ''}`}
                        onClick={() => setFilter(f)}
                      >
                        {f.charAt(0).toUpperCase() + f.slice(1)}
                      </button>
                    ))}
                  </div>
                  <input
                    type="text"
                    className="tune-diff-search"
                    placeholder="Search constants..."
                    value={search}
                    onChange={e => setSearch(e.target.value)}
                  />
                </div>

                <div className="tune-diff-table-container">
                  <table className="tune-diff-table">
                    <thead>
                      <tr>
                        <th className="col-check">
                          <input
                            type="checkbox"
                            checked={filtered.length > 0 && selected.size === filtered.length}
                            onChange={selectAll}
                          />
                        </th>
                        <th className="col-name sortable" onClick={() => handleSort('name')}>
                          Constant {sortField === 'name' && (sortAsc ? '▲' : '▼')}
                        </th>
                        <th className="col-value">Value A</th>
                        <th className="col-value">Value B</th>
                        <th className="col-diff sortable" onClick={() => handleSort('diff')}>
                          Diff {sortField === 'diff' && (sortAsc ? '▲' : '▼')}
                        </th>
                        <th className="col-pct sortable" onClick={() => handleSort('pct')}>
                          % {sortField === 'pct' && (sortAsc ? '▲' : '▼')}
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {filtered.slice(0, 200).map(d => (
                        <tr key={d.name} className={severityClass(d)}>
                          <td className="col-check">
                            <input
                              type="checkbox"
                              checked={selected.has(d.name)}
                              onChange={() => toggleSelect(d.name)}
                            />
                          </td>
                          <td className="col-name">{d.name}</td>
                          <td className="col-value">{formatValue(d.value_a)}</td>
                          <td className="col-value">{formatValue(d.value_b)}</td>
                          <td className="col-diff">
                            {d.numeric_diff !== null ? d.numeric_diff.toFixed(2) : '-'}
                          </td>
                          <td className="col-pct">
                            {d.percent_diff !== null ? `${d.percent_diff.toFixed(1)}%` : '-'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {filtered.length > 200 && (
                    <div className="tune-diff-overflow">
                      Showing 200 of {filtered.length} differences
                    </div>
                  )}
                </div>
              </>
            )}
          </div>
        )}
      </Dialog.Body>

      <Dialog.Footer className="tune-diff-footer">
        {diff && diff.differences.length > 0 && (
          <>
            <span className="merge-info">
              {selected.size} constant{selected.size !== 1 ? 's' : ''} selected
            </span>
            <Button
              variant="primary"
              onClick={handleMerge}
              disabled={selected.size === 0 || merging}
              leadingIcon={<GitMerge size={14} />}
            >
              {merging ? 'Merging...' : `Merge ${selected.size} from B → Current`}
            </Button>
            {mergeResult && (
              <span className={`merge-result ${mergeResult.includes('failed') ? 'error' : 'success'}`}>
                {mergeResult}
              </span>
            )}
          </>
        )}
        <Button variant="secondary" onClick={onClose}>Close</Button>
      </Dialog.Footer>
    </Dialog>
  );
}
