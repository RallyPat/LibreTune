/**
 * Table Comparison Dialog
 * 
 * Allows users to select two tables and visualize the differences between them.
 * Useful for comparing tunes, before/after changes, or different calibrations.
 */

import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, GitCompare, ArrowRight, ArrowLeft } from "lucide-react";
import "./TableComparisonDialog.css";

interface TableInfo {
  name: string;
  title: string;
}

interface TableCellDiff {
  x: number;
  y: number;
  value_a: number;
  value_b: number;
  difference: number;
  percent_diff: number;
}

interface TableComparisonResult {
  differences: TableCellDiff[];
  max_diff: number;
  avg_diff: number;
  cells_changed: number;
  total_cells: number;
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

export default function TableComparisonDialog({ isOpen, onClose }: Props) {
  const [tables, setTables] = useState<TableInfo[]>([]);
  const [tableA, setTableA] = useState<string>("");
  const [tableB, setTableB] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [comparing, setComparing] = useState(false);
  const [result, setResult] = useState<TableComparisonResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Load available tables on open
  useEffect(() => {
    if (isOpen) {
      loadTables();
      // Reset state
      setResult(null);
      setError(null);
    }
  }, [isOpen]);

  const loadTables = async () => {
    setLoading(true);
    try {
      const tableList = await invoke<TableInfo[]>("get_tables");
      setTables(tableList);
      if (tableList.length >= 2) {
        setTableA(tableList[0].name);
        setTableB(tableList[1].name);
      }
    } catch (e) {
      setError(`Failed to load tables: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const handleCompare = async () => {
    if (!tableA || !tableB) {
      setError("Please select two tables to compare");
      return;
    }
    if (tableA === tableB) {
      setError("Please select two different tables");
      return;
    }

    setComparing(true);
    setError(null);
    setResult(null);
    
    try {
      const comparisonResult = await invoke<TableComparisonResult>("compare_tables", {
        tableAName: tableA,
        tableBName: tableB,
      });
      setResult(comparisonResult);
    } catch (e) {
      setError(`Comparison failed: ${e}`);
    } finally {
      setComparing(false);
    }
  };

  const swapTables = () => {
    const temp = tableA;
    setTableA(tableB);
    setTableB(temp);
    setResult(null);
  };

  // Group differences by severity
  const diffGroups = useMemo(() => {
    if (!result) return { high: [], medium: [], low: [] };
    
    const high = result.differences.filter(d => Math.abs(d.percent_diff) >= 10);
    const medium = result.differences.filter(d => Math.abs(d.percent_diff) >= 5 && Math.abs(d.percent_diff) < 10);
    const low = result.differences.filter(d => Math.abs(d.percent_diff) > 0 && Math.abs(d.percent_diff) < 5);
    
    return { high, medium, low };
  }, [result]);

  if (!isOpen) return null;

  return (
    <div className="table-comparison-overlay" onClick={onClose}>
      <div className="table-comparison-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="table-comparison-header">
          <h2>
            <GitCompare size={20} />
            Table Comparison
          </h2>
          <button className="close-btn" onClick={onClose}>
            <X size={18} />
          </button>
        </div>

        <div className="table-comparison-content">
          {/* Table Selection */}
          <div className="table-selection">
            <div className="table-select-group">
              <label>Table A (Base)</label>
              <select 
                value={tableA} 
                onChange={(e) => { setTableA(e.target.value); setResult(null); }}
                disabled={loading}
              >
                <option value="">Select a table...</option>
                {tables.map((t) => (
                  <option key={t.name} value={t.name}>{t.title || t.name}</option>
                ))}
              </select>
            </div>

            <button className="swap-btn" onClick={swapTables} title="Swap tables">
              <ArrowLeft size={14} />
              <ArrowRight size={14} />
            </button>

            <div className="table-select-group">
              <label>Table B (Compare)</label>
              <select 
                value={tableB} 
                onChange={(e) => { setTableB(e.target.value); setResult(null); }}
                disabled={loading}
              >
                <option value="">Select a table...</option>
                {tables.map((t) => (
                  <option key={t.name} value={t.name}>{t.title || t.name}</option>
                ))}
              </select>
            </div>
          </div>

          {error && (
            <div className="comparison-error">{error}</div>
          )}

          <button 
            className="compare-btn" 
            onClick={handleCompare}
            disabled={!tableA || !tableB || comparing}
          >
            {comparing ? "Comparing..." : "Compare Tables"}
          </button>

          {/* Results */}
          {result && (
            <div className="comparison-results">
              <div className="results-summary">
                <div className="summary-stat">
                  <span className="stat-value">{result.cells_changed}</span>
                  <span className="stat-label">Cells Changed</span>
                </div>
                <div className="summary-stat">
                  <span className="stat-value">{result.total_cells}</span>
                  <span className="stat-label">Total Cells</span>
                </div>
                <div className="summary-stat">
                  <span className="stat-value">{result.max_diff.toFixed(2)}</span>
                  <span className="stat-label">Max Difference</span>
                </div>
                <div className="summary-stat">
                  <span className="stat-value">{result.avg_diff.toFixed(2)}</span>
                  <span className="stat-label">Avg Difference</span>
                </div>
              </div>

              {result.cells_changed === 0 ? (
                <div className="no-differences">
                  ✓ Tables are identical
                </div>
              ) : (
                <div className="diff-groups">
                  {diffGroups.high.length > 0 && (
                    <div className="diff-group high">
                      <h4>High Variance (&ge;10%)</h4>
                      <div className="diff-list">
                        {diffGroups.high.slice(0, 10).map((d, i) => (
                          <div key={i} className="diff-item">
                            <span className="diff-coord">[{d.x}, {d.y}]</span>
                            <span className="diff-values">
                              {d.value_a.toFixed(2)} → {d.value_b.toFixed(2)}
                            </span>
                            <span className={`diff-percent ${d.difference > 0 ? 'positive' : 'negative'}`}>
                              {d.difference > 0 ? '+' : ''}{d.percent_diff.toFixed(1)}%
                            </span>
                          </div>
                        ))}
                        {diffGroups.high.length > 10 && (
                          <div className="diff-more">...and {diffGroups.high.length - 10} more</div>
                        )}
                      </div>
                    </div>
                  )}

                  {diffGroups.medium.length > 0 && (
                    <div className="diff-group medium">
                      <h4>Medium Variance (5-10%)</h4>
                      <div className="diff-list">
                        {diffGroups.medium.slice(0, 10).map((d, i) => (
                          <div key={i} className="diff-item">
                            <span className="diff-coord">[{d.x}, {d.y}]</span>
                            <span className="diff-values">
                              {d.value_a.toFixed(2)} → {d.value_b.toFixed(2)}
                            </span>
                            <span className={`diff-percent ${d.difference > 0 ? 'positive' : 'negative'}`}>
                              {d.difference > 0 ? '+' : ''}{d.percent_diff.toFixed(1)}%
                            </span>
                          </div>
                        ))}
                        {diffGroups.medium.length > 10 && (
                          <div className="diff-more">...and {diffGroups.medium.length - 10} more</div>
                        )}
                      </div>
                    </div>
                  )}

                  {diffGroups.low.length > 0 && (
                    <div className="diff-group low">
                      <h4>Low Variance (&lt;5%)</h4>
                      <div className="diff-list collapsed">
                        <span>{diffGroups.low.length} cells with minor differences</span>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        <div className="table-comparison-footer">
          <button className="cancel-btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}
