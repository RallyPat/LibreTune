/**
 * Shared Dialog primitive
 *
 * Provides consistent overlay, glass-card, header, footer, and close button
 * styling/behavior for modal dialogs across the app. Replaces the bespoke
 * overlay/header markup that each dialog used to define.
 *
 * Usage:
 *   <Dialog open={open} onClose={...} title="Edit Cell" size="sm">
 *     <Dialog.Body>...</Dialog.Body>
 *     <Dialog.Footer>
 *       <Button variant="secondary" onClick={...}>Cancel</Button>
 *       <Button variant="primary" onClick={...}>Apply</Button>
 *     </Dialog.Footer>
 *   </Dialog>
 */

import React, { useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import './Dialog.css';

export type DialogSize = 'sm' | 'md' | 'lg' | 'xl' | 'full';

export interface DialogProps {
  open: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  /** Title-bar adornment (e.g. badge, icon). Rendered next to title. */
  titleAdornment?: React.ReactNode;
  /** Hide the close button in the header. */
  hideClose?: boolean;
  /** Click on the backdrop closes the dialog. Default true. */
  closeOnBackdrop?: boolean;
  /** ESC closes the dialog. Default true. */
  closeOnEscape?: boolean;
  /** Width preset. */
  size?: DialogSize;
  /** Optional className appended to the dialog card. */
  className?: string;
  children: React.ReactNode;
  /** Hide title-bar entirely (caller renders custom header). */
  noHeader?: boolean;
  /** Optional aria label when no visible title is set. */
  ariaLabel?: string;
}

export function Dialog({
  open,
  onClose,
  title,
  titleAdornment,
  hideClose = false,
  closeOnBackdrop = true,
  closeOnEscape = true,
  size = 'md',
  className,
  children,
  noHeader = false,
  ariaLabel,
}: DialogProps) {
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open || !closeOnEscape) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, closeOnEscape, onClose]);

  if (!open) return null;

  const handleBackdrop = (e: React.MouseEvent) => {
    if (!closeOnBackdrop) return;
    if (e.target === e.currentTarget) onClose();
  };

  return (
    <div className="lt-dialog-overlay" onClick={handleBackdrop}>
      <div
        ref={cardRef}
        className={`lt-dialog lt-dialog--${size}${className ? ` ${className}` : ''}`}
        role="dialog"
        aria-modal="true"
        aria-label={typeof title === 'string' ? title : ariaLabel}
      >
        {!noHeader && (title || !hideClose) && (
          <header className="lt-dialog__header">
            <div className="lt-dialog__title">
              {title}
              {titleAdornment}
            </div>
            {!hideClose && (
              <button
                type="button"
                className="lt-dialog__close"
                onClick={onClose}
                aria-label="Close dialog"
              >
                <X size={18} />
              </button>
            )}
          </header>
        )}
        {children}
      </div>
    </div>
  );
}

function Body({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={`lt-dialog__body${className ? ` ${className}` : ''}`}>{children}</div>
  );
}

/**
 * Dialog footer for action buttons.
 *
 * Convention (Western/macOS): place buttons in DOM order such that the
 * primary/affirmative action is LAST. The footer is `flex` with
 * `justify-content: flex-end`, so buttons render with the primary on the
 * RIGHT and Cancel on the LEFT.
 *
 * Recommended:
 *   <Dialog.Footer>
 *     <Button variant="secondary" onClick={onClose}>Cancel</Button>
 *     <Button variant="primary" onClick={onSave}>Save</Button>
 *   </Dialog.Footer>
 *
 * Use `Close` (variant="secondary") for read-only/info dialogs.
 * Avoid generic `OK` — use the verb that matches the action
 * (Save / Apply / Create / Import / Delete / Continue).
 */
function Footer({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <footer className={`lt-dialog__footer${className ? ` ${className}` : ''}`}>{children}</footer>
  );
}

Dialog.Body = Body;
Dialog.Footer = Footer;

export default Dialog;
