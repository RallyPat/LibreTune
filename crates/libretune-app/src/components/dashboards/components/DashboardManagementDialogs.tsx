
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Dialog, Button } from '../../common';

/** Template info from the `get_dashboard_templates` command. */
interface DashboardTemplate {
  id: string;
  name: string;
  description: string;
}

/** Shown when no INI/backend is reachable — mirrors the backend registry. */
const FALLBACK_TEMPLATES: DashboardTemplate[] = [
  { id: 'basic', name: 'Basic Dashboard', description: 'Essential gauges: RPM, AFR, Coolant, Throttle' },
  { id: 'tuning', name: 'Tuning Dashboard', description: 'AFR, VE, Spark advance, and correction factors' },
  { id: 'telemetry_live', name: 'Telemetry Live', description: 'Dense live view with stat tiles, charts and sparklines' },
];

interface Props {
  // New
  newOpen: boolean;
  onNewClose: () => void;
  onNewCreate: (name: string, template: string) => void;

  // Rename
  renameOpen: boolean;
  renameInitialValue: string;
  onRenameClose: () => void;
  onRenameConfirm: (name: string) => void;

  // Delete
  deleteOpen: boolean;
  deleteTargetName: string;
  onDeleteClose: () => void;
  onDeleteConfirm: () => void;

  // Reset to defaults
  resetOpen: boolean;
  onResetClose: () => void;
  onResetConfirm: () => void;
}

/**
 * New / Rename / Delete / Reset dashboard dialogs. Owns its form fields; the
 * shell only controls open/close.
 */
export default function DashboardManagementDialogs({
  newOpen,
  onNewClose,
  onNewCreate,
  renameOpen,
  renameInitialValue,
  onRenameClose,
  onRenameConfirm,
  deleteOpen,
  deleteTargetName,
  onDeleteClose,
  onDeleteConfirm,
  resetOpen,
  onResetClose,
  onResetConfirm,
}: Props) {
  const [newName, setNewName] = useState('');
  const [template, setTemplate] = useState('basic');
  const [templates, setTemplates] = useState<DashboardTemplate[]>(FALLBACK_TEMPLATES);
  const [renameValue, setRenameValue] = useState('');

  // Re-seed the form fields each time a dialog opens.
  useEffect(() => {
    if (newOpen) {
      setNewName('');
      setTemplate('basic');
    }
  }, [newOpen]);
  useEffect(() => {
    if (renameOpen) setRenameValue(renameInitialValue);
  }, [renameOpen, renameInitialValue]);

  // Populate the template picker from the backend registry.
  useEffect(() => {
    if (!newOpen) return;
    invoke<DashboardTemplate[]>('get_dashboard_templates')
      .then((list) => {
        if (list && list.length > 0) setTemplates(list);
      })
      .catch(() => {
        // Keep the fallback list.
      });
  }, [newOpen]);

  return (
    <>
      {/* New Dashboard Dialog */}
      <Dialog open={newOpen} onClose={onNewClose} title="New Dashboard" size="sm">
        <Dialog.Body>
          <label>Dashboard Name:</label>
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="My Dashboard"
            autoFocus
            onKeyDown={(e) => e.key === 'Enter' && onNewCreate(newName, template)}
          />
          <label style={{ marginTop: 10 }}>Template:</label>
          <select value={template} onChange={(e) => setTemplate(e.target.value)}>
            {templates.map((t) => (
              <option key={t.id} value={t.id}>{t.name}</option>
            ))}
          </select>
          {templates.find((t) => t.id === template)?.description && (
            <p className="hint">{templates.find((t) => t.id === template)?.description}</p>
          )}
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={onNewClose}>Cancel</Button>
          <Button variant="primary" onClick={() => onNewCreate(newName, template)} disabled={!newName.trim()}>
            Create
          </Button>
        </Dialog.Footer>
      </Dialog>

      {/* Rename Dashboard Dialog */}
      <Dialog open={renameOpen} onClose={onRenameClose} title="Rename Dashboard" size="sm">
        <Dialog.Body>
          <label>New Name:</label>
          <input
            type="text"
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            placeholder="Dashboard Name"
            autoFocus
            onKeyDown={(e) => e.key === 'Enter' && onRenameConfirm(renameValue)}
          />
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={onRenameClose}>Cancel</Button>
          <Button variant="primary" onClick={() => onRenameConfirm(renameValue)} disabled={!renameValue.trim()}>
            Rename
          </Button>
        </Dialog.Footer>
      </Dialog>

      {/* Delete Confirmation Dialog */}
      <Dialog open={deleteOpen} onClose={onDeleteClose} title="Delete Dashboard?" size="sm">
        <Dialog.Body>
          <p>Are you sure you want to delete "{deleteTargetName}"?</p>
          <p className="warning">This action cannot be undone.</p>
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={onDeleteClose}>Cancel</Button>
          <Button variant="danger" onClick={onDeleteConfirm}>Delete</Button>
        </Dialog.Footer>
      </Dialog>

      {/* Reset to Defaults Confirmation Dialog */}
      <Dialog open={resetOpen} onClose={onResetClose} title="Reset All Dashboards?" size="sm">
        <Dialog.Body>
          <p>Reset every dashboard to the built-in defaults?</p>
          <p className="warning">
            This deletes ALL dashboards in your dashboard directory — including your
            custom ones — and recreates the defaults. This action cannot be undone.
          </p>
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={onResetClose}>Cancel</Button>
          <Button variant="danger" onClick={onResetConfirm}>Reset</Button>
        </Dialog.Footer>
      </Dialog>
    </>
  );
}
