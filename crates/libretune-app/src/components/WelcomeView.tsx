import { useState } from "react";
import "./WelcomeView.css";

interface ProjectInfo {
  name: string;
  path: string;
  signature: string;
  modified: string;
}

interface WelcomeViewProps {
  projects: ProjectInfo[];
  onOpenProject: (path: string) => void;
  onNewProject: () => void;
  onConnect: () => void;
  onImportTsProject: () => void;
  onDeleteProject: (name: string) => void;
}

export default function WelcomeView({
  projects,
  onOpenProject,
  onNewProject,
  onConnect,
  onImportTsProject,
  onDeleteProject,
}: WelcomeViewProps) {
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  function handleDelete(e: React.MouseEvent, projectName: string) {
    e.stopPropagation();
    if (confirmDelete === projectName) {
      onDeleteProject(projectName);
      setConfirmDelete(null);
    } else {
      setConfirmDelete(projectName);
    }
  }

  return (
    <div className="welcome-view">
      <div className="welcome-header">
        <h1 className="welcome-title">LibreTune</h1>
        <p className="welcome-subtitle">Open-source ECU tuning software</p>
      </div>

      <div className="welcome-actions">
        <button className="welcome-action-btn primary" onClick={onNewProject}>
          <span className="action-icon">📁</span>
          <span className="action-label">New Project</span>
          <span className="action-desc">Create a new tuning project</span>
        </button>
        <button className="welcome-action-btn" onClick={onConnect}>
          <span className="action-icon">🔌</span>
          <span className="action-label">Connect to ECU</span>
          <span className="action-desc">Connect via serial port</span>
        </button>
        <button className="welcome-action-btn" onClick={onImportTsProject}>
          <span className="action-icon">📥</span>
          <span className="action-label">Import TS Project</span>
          <span className="action-desc">Import from TunerStudio</span>
        </button>
      </div>

      {projects.length > 0 && (
        <div className="welcome-recent">
          <h3 className="recent-title">Recent Projects</h3>
          <div className="recent-list">
            {projects.slice(0, 8).map((project) => (
              <div
                key={project.path}
                className="recent-item"
                onClick={() => onOpenProject(project.path)}
              >
                <div className="recent-info">
                  <div className="recent-name">{project.name}</div>
                  <div className="recent-sig">{project.signature}</div>
                  <div className="recent-date">
                    Last modified: {new Date(project.modified).toLocaleDateString()}
                  </div>
                </div>
                <button
                  className={`recent-delete ${confirmDelete === project.name ? "confirm" : ""}`}
                  onClick={(e) => handleDelete(e, project.name)}
                  title={confirmDelete === project.name ? "Click again to confirm" : "Delete project"}
                >
                  {confirmDelete === project.name ? "✓ Confirm" : "✕"}
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
