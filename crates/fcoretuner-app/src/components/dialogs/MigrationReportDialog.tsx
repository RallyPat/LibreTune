//! Migration Report Dialog
//!
//! Shows when loading a tune created with a different INI version.
//! Displays what constants have changed and gives user options to proceed.

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle,
  X,
  Info,
  Plus,
  Minus,
  RefreshCw,
  Scale,
  ChevronDown,
  ChevronRight,
  Check,
} from "lucide-react";
import "./MigrationReportDialog.css";

// Match the Rust MigrationReport struct
export interface ConstantChange {
  name: string;
  old_type?: string;
  new_type?: string;
  old_scale?: number;
  new_scale?: number;
  old_offset?: number;
  new_offset?: number;
  old_translate?: number;
  new_translate?: number;
}

export interface MigrationReport {
  missing_in_tune: string[];
  missing_in_ini: string[];
  type_changed: ConstantChange[];
  scale_changed: ConstantChange[];
  can_auto_migrate: boolean;
  requires_user_review: boolean;
  severity: "none" | "low" | "medium" | "high";
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
  onProceed: () => void;
}

export default function MigrationReportDialog({
  isOpen,
  onClose,
  onProceed,
}: Props) {
  const [report, setReport] = useState<MigrationReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [expandedSections, setExpandedSections] = useState<Set<string>>(
    new Set(["critical"])
  );

  // Listen for migration events
  useEffect(() => {
    const unlisten = listen<MigrationReport>("tune:migration_needed", (event) => {
      setReport(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Load report when dialog opens
  useEffect(() => {
    if (isOpen && !report) {
      loadReport();
    }
  }, [isOpen]);

  const loadReport = async () => {
    setLoading(true);
    try {
      const r = await invoke<MigrationReport | null>("get_migration_report");
      setReport(r);
    } catch (e) {
      console.error("Failed to load migration report:", e);
    } finally {
      setLoading(false);
    }
  };

  const handleProceed = async () => {
    // Clear the report so we don't show it again
    try {
      await invoke("clear_migration_report");
    } catch (e) {
      console.error("Failed to clear migration report:", e);
    }
    setReport(null);
    onProceed();
    onClose();
  };

  const handleDismiss = async () => {
    try {
      await invoke("clear_migration_report");
    } catch (e) {
      console.error("Failed to clear migration report:", e);
    }
    setReport(null);
    onClose();
  };

  const toggleSection = (section: string) => {
    setExpandedSections((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(section)) {
        newSet.delete(section);
      } else {
        newSet.add(section);
      }
      return newSet;
    });
  };

  if (!isOpen) return null;

  const getSeverityClass = () => {
    switch (report?.severity) {
      case "high":
        return "severity-high";
      case "medium":
        return "severity-medium";
      case "low":
        return "severity-low";
      default:
        return "severity-none";
    }
  };

  const getSeverityText = () => {
    switch (report?.severity) {
      case "high":
        return "检测到重大变更";
      case "medium":
        return "检测到中等变更";
      case "low":
        return "检测到轻微变更";
      default:
        return "无变更";
    }
  };

  return (
    <div className="migration-overlay" onClick={handleDismiss}>
      <div
        className={`migration-dialog ${getSeverityClass()}`}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="migration-header">
          <div className={`header-icon ${getSeverityClass()}`}>
            {report?.severity === "high" ? (
              <AlertTriangle size={24} />
            ) : (
              <Info size={24} />
            )}
          </div>
          <h2>INI 版本迁移</h2>
          <button className="close-btn" onClick={handleDismiss} title="关闭">
            <X size={18} />
          </button>
        </div>

        {/* Content */}
        <div className="migration-content">
          {loading && (
            <div className="migration-loading">正在加载迁移报告...</div>
          )}

          {!loading && !report && (
            <div className="migration-empty">
              <Info size={48} className="empty-icon" />
              <p>没有可用的迁移报告。</p>
              <p className="hint">
                此调教可能是使用当前 INI 版本创建的，或者是
                不包含版本跟踪的 1.1 之前格式。
              </p>
            </div>
          )}

          {!loading && report && (
            <>
              {/* Summary */}
              <div className={`migration-summary ${getSeverityClass()}`}>
                <span className="severity-badge">{getSeverityText()}</span>
                <p>
                  此调教是使用不同的 INI 版本创建的。某些
                  常量可能已更改。
                </p>
              </div>

              {/* Critical: Type changes */}
              {report.type_changed.length > 0 && (
                <div className="migration-section critical">
                  <button
                    className="section-header"
                    onClick={() => toggleSection("type")}
                  >
                    {expandedSections.has("type") ? (
                      <ChevronDown size={16} />
                    ) : (
                      <ChevronRight size={16} />
                    )}
                    <RefreshCw size={16} className="section-icon" />
                    <span className="section-title">
                      类型变更 ({report.type_changed.length})
                    </span>
                    <span className="section-badge critical">需要审核</span>
                  </button>
                  {expandedSections.has("type") && (
                    <ul className="change-list">
                      {report.type_changed.map((c) => (
                        <li key={c.name}>
                          <code>{c.name}</code>
                          <span className="change-detail">
                            {c.old_type} → {c.new_type}
                          </span>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}

              {/* Scale changes */}
              {report.scale_changed.length > 0 && (
                <div className="migration-section warning">
                  <button
                    className="section-header"
                    onClick={() => toggleSection("scale")}
                  >
                    {expandedSections.has("scale") ? (
                      <ChevronDown size={16} />
                    ) : (
                      <ChevronRight size={16} />
                    )}
                    <Scale size={16} className="section-icon" />
                    <span className="section-title">
                      比例/偏移变更 ({report.scale_changed.length})
                    </span>
                    <span className="section-badge warning">可能影响数值</span>
                  </button>
                  {expandedSections.has("scale") && (
                    <ul className="change-list">
                      {report.scale_changed.map((c) => (
                        <li key={c.name}>
                          <code>{c.name}</code>
                          <span className="change-detail">
                            scale: {c.old_scale?.toFixed(4)} →{" "}
                            {c.new_scale?.toFixed(4)}
                          </span>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}

              {/* Missing in INI (removed constants) */}
              {report.missing_in_ini.length > 0 && (
                <div className="migration-section warning">
                  <button
                    className="section-header"
                    onClick={() => toggleSection("removed")}
                  >
                    {expandedSections.has("removed") ? (
                      <ChevronDown size={16} />
                    ) : (
                      <ChevronRight size={16} />
                    )}
                    <Minus size={16} className="section-icon" />
                    <span className="section-title">
                      从 INI 中移除 ({report.missing_in_ini.length})
                    </span>
                    <span className="section-badge info">值已保留</span>
                  </button>
                  {expandedSections.has("removed") && (
                    <ul className="change-list compact">
                      {report.missing_in_ini.slice(0, 20).map((name) => (
                        <li key={name}>
                          <code>{name}</code>
                        </li>
                      ))}
                      {report.missing_in_ini.length > 20 && (
                        <li className="more-items">
                          ...以及 {report.missing_in_ini.length - 20} 个
                        </li>
                      )}
                    </ul>
                  )}
                </div>
              )}

              {/* Missing in tune (new constants) */}
              {report.missing_in_tune.length > 0 && (
                <div className="migration-section info">
                  <button
                    className="section-header"
                    onClick={() => toggleSection("new")}
                  >
                    {expandedSections.has("new") ? (
                      <ChevronDown size={16} />
                    ) : (
                      <ChevronRight size={16} />
                    )}
                    <Plus size={16} className="section-icon" />
                    <span className="section-title">
                      INI 中新增 ({report.missing_in_tune.length})
                    </span>
                    <span className="section-badge info">使用默认值</span>
                  </button>
                  {expandedSections.has("new") && (
                    <ul className="change-list compact">
                      {report.missing_in_tune.slice(0, 20).map((name) => (
                        <li key={name}>
                          <code>{name}</code>
                        </li>
                      ))}
                      {report.missing_in_tune.length > 20 && (
                        <li className="more-items">
                          ...以及 {report.missing_in_tune.length - 20} 个
                        </li>
                      )}
                    </ul>
                  )}
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="migration-footer">
          {report?.requires_user_review && (
            <div className="review-warning">
              <AlertTriangle size={14} />
              <span>烧录到 ECU 前请审核类型变更</span>
            </div>
          )}
          <div className="footer-buttons">
            <button className="btn-secondary" onClick={handleDismiss}>
              关闭
            </button>
            <button className="btn-primary" onClick={handleProceed}>
              <Check size={16} />
              继续使用此调教
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
