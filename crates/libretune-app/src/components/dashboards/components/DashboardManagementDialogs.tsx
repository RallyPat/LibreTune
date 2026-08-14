
import { useEffect, useState } from 'react';
import { Dialog, Button } from '../../common';

interface Props {
  // New
  newOpen: boolean;
  onNewClose: () => void;
  onNewCreate: (name: string) => void;

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
}

/**
 * New / Rename / Delete dashboard dialogs. Owns its form fields; the shell
 * only controls open/close. Extracted from TsDashboard during Phase C4.
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
}: Props) {
  const [newName, setNewName] = useState('');
  const [renameValue, setRenameValue] = useState('');

  // Re-seed the form fields each time a dialog opens.
  useEffect(() => {
    if (newOpen) setNewName('');
  }, [newOpen]);
  useEffect(() => {
    if (renameOpen) setRenameValue(renameInitialValue);
  }, [renameOpen, renameInitialValue]);

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
            onKeyDown={(e) => e.key === 'Enter' && onNewCreate(newName)}
          />
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={onNewClose}>Cancel</Button>
          <Button variant="primary" onClick={() => onNewCreate(newName)} disabled={!newName.trim()}>
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
    </>
  );
}
