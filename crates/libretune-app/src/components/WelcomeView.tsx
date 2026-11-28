import { useState } from "react";
import {
  Activity, ArrowRight, Battery, Check, CodeXml, Cpu, Download, FolderOpen,
  FolderPlus, Gauge, Layers, Shield, Thermometer, X, Zap,
} from "lucide-react";
import logo from "../../src-tauri/icons/icon.png";
import type { ConnectionStatus, ProjectInfo } from "../types/app";
import "./WelcomeView.css";

interface WelcomeViewProps {
  projects: ProjectInfo[];
  status: ConnectionStatus;
  onOpenProject: (path: string) => void;
  onConnectEcu: () => void;
  onNewProject: () => void;
  onBrowseProject: () => void;
  onImportTsProject: () => void;
  onDeleteProject: (name: string) => void;
}

const connectionLabels: Record<ConnectionStatus["state"], string> = {
  Connected: "Connected",
  Connecting: "Connecting",
  Disconnected: "Not connected",
  Error: "Connection error",
};

const demoGauges = [
  { label: "Engine speed", value: "0", unit: "RPM", icon: Gauge },
  { label: "Boost pressure", value: "0.00", unit: "bar", icon: Gauge },
  { label: "Coolant temperature", value: "20", unit: "°C", icon: Thermometer },
  { label: "Battery voltage", value: "12.4", unit: "V", icon: Battery },
];

export default function WelcomeView({
  projects, status, onOpenProject, onConnectEcu, onNewProject,
  onBrowseProject, onImportTsProject, onDeleteProject,
}: WelcomeViewProps) {
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const actions = [
    { label: "Connect ECU", detail: "Detect and connect to a supported ECU", icon: Cpu, run: onConnectEcu },
    { label: "New Offline Project", detail: "Create a tuning project without an ECU", icon: FolderPlus, run: onNewProject },
    { label: "Open Existing Project", detail: "Choose a LibreTune project folder", icon: FolderOpen, run: onBrowseProject },
    { label: "Import TunerStudio Project", detail: "Import an existing project folder", icon: Download, run: onImportTsProject },
  ];

  return (
    <div className="welcome-view">
      <div className="welcome-console">
        <section className="welcome-hero" aria-labelledby="welcome-title">
          <div className="welcome-brand">
            <header className="welcome-header">
              <h1 id="welcome-title"><img className="welcome-logo" src={logo} alt="" />LibreTune</h1>
              <p>Open-source ECU tuning software</p>
            </header>
            <div className="welcome-backdrop" aria-hidden="true">
              <div className="welcome-laptop">
                <div className="welcome-laptop-screen">
                  <span>VE Table</span>
                  <div className="welcome-ve-grid">
                    {Array.from({ length: 48 }, (_, index) => <span key={index} />)}
                  </div>
                </div>
              </div>
              <div className="welcome-backdrop-gauges"><span /><span /></div>
            </div>
          </div>
          <section className="welcome-status" aria-label="ECU status">
            <h2><Cpu size={18} aria-hidden="true" /> ECU status</h2>
            <p className="welcome-connection" data-state={status.state} aria-live="polite">
              {connectionLabels[status.state]}
            </p>
            <dl>
              <div><dt>ECU ID</dt><dd>{status.signature || "—"}</dd></div>
              <div><dt>Definition</dt><dd>{status.ini_name || (status.has_definition ? "Loaded" : "—")}</dd></div>
            </dl>
          </section>
          <section className="welcome-demo" aria-label="Demo gauges">
            <h2><Activity size={18} aria-hidden="true" /> Demo gauges</h2>
            <div className="welcome-dials">
              {demoGauges.map(({ label, value, unit, icon: Icon }) => (
                <div className="welcome-dial" key={label} role="img" aria-label={`${label}: ${value} ${unit}`}>
                  <Icon size={16} aria-hidden="true" />
                  <strong>{value}</strong><small>{unit}</small>
                </div>
              ))}
            </div>
          </section>
        </section>

        <nav className="welcome-actions" aria-label="Project actions">
          {actions.map(({ label, detail, icon: Icon, run }) => (
            <button type="button" className="welcome-action" onClick={run} key={label}>
              <Icon className="welcome-action-icon" size={32} aria-hidden="true" />
              <strong>{label}</strong>
              <span>{detail}</span>
              <ArrowRight className="welcome-action-arrow" size={22} aria-hidden="true" />
            </button>
          ))}
        </nav>

        <ul className="welcome-features" aria-label="About LibreTune">
          <li><CodeXml size={20} aria-hidden="true" /> Open Source</li>
          <li><Layers size={20} aria-hidden="true" /> Multi-Platform</li>
          <li><Zap size={20} aria-hidden="true" /> ECU Tuning &amp; Calibration</li>
          <li><Shield size={20} aria-hidden="true" /> Community Driven</li>
        </ul>

        {projects.length > 0 && (
          <section className="welcome-recent" aria-labelledby="welcome-recent-title">
            <h2 id="welcome-recent-title">Recent projects</h2>
            <ul>
              {projects.slice(0, 8).map((project) => (
                <li key={project.path}>
                  <button type="button" className="welcome-recent-open" onClick={() => onOpenProject(project.path)}>
                    <FolderOpen size={17} aria-hidden="true" />
                    <span><strong>{project.name}</strong><small>{project.signature}</small></span>
                    <time dateTime={project.modified}>{new Date(project.modified).toLocaleDateString()}</time>
                  </button>
                  <button
                    type="button"
                    className="welcome-delete"
                    aria-label={confirmDelete === project.path ? `Confirm deletion of ${project.name}` : `Delete ${project.name}`}
                    onClick={() => {
                      if (confirmDelete === project.path) {
                        onDeleteProject(project.name);
                        setConfirmDelete(null);
                      } else {
                        setConfirmDelete(project.path);
                      }
                    }}
                  >
                    {confirmDelete === project.path ? <><Check size={15} /> Confirm</> : <X size={15} />}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}
