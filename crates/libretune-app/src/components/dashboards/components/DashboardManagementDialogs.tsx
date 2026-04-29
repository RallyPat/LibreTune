
import { Dialog, Button } from '../../common';

interface Props {
  // New
  newOpen: boolean;
  newName: string;
  onNewNameChange: (v: string) => void;
  onNewClose: () => void;
  onNewCreate: () => void;

  // Rename
  renameOpen: boolean;
  renameValue: string;
  onRenameValueChange: (v: string) => void;
  onRenameClose: () => void;
  onRenameConfirm: () => void;

  // Delete
  deleteOpen: boolean;
  deleteTargetName: string;
  onDeleteClose: () => void;
  onDeleteConfirm: () => void;
}

/**
 * New / Rename / Delete dashboard dialogs.
 * Extracted from TsDashboard during Phase C4.
 */
export default function DashboardManagementDialogs({
  newOpen,
  newName,
  onNewNameChange,
  onNewClose,
  onNewCreate,
  renameOpen,
  renameValue,
  onRenameValueChange,
  onRenameClose,
  onRenameConfirm,
  deleteOpen,
  deleteTargetName,
  onDeleteClose,
  onDeleteConfirm,
}: Props) {
  return (
    <>
      {/* New Dashboard Dialog */}
      <Dialog open={newOpen} onClose={onNewClose} title="New Dashboard" size="sm">
        <Dialog.Body>
          <label>Dashboard Name:</label>
          <input
            type="text"
            value={newName}
            onChange={(e) => onNewNameChange(e.target.value)}
            placeholder="My Dashboard"
            autoFocus
            onKeyDown={(e) => e.key === 'Enter' && onNewCreate()}
          />
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={onNewClose}>Cancel</Button>
          <Button variant="primary" onClick={onNewCreate} disabled={!newName.trim()}>
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
            onChange={(e) => onRenameValueChange(e.target.value)}
            placeholder="Dashboard Name"
            autoFocus
            onKeyDown={(e) => e.key === 'Enter' && onRenameConfirm()}
          />
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={onRenameClose}>Cancel</Button>
          <Button variant="primary" onClick={onRenameConfirm} disabled={!renameValue.trim()}>
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
