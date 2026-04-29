import { useState, useEffect } from 'react';
import { Wrench } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Dialog, Button } from '../../common';
import { DialogProps, BuildInfo } from './types';
import '../Dialogs.css';

export function AboutDialog({ isOpen, onClose }: DialogProps) {
  const [buildInfo, setBuildInfo] = useState<BuildInfo | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    invoke<BuildInfo>('get_build_info')
      .then(setBuildInfo)
      .catch(() => setBuildInfo(null));
  }, [isOpen]);

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="About LibreTune"
      size="sm"
    >
      <Dialog.Body className="dialog-about">
        <div className="dialog-about-logo"><Wrench size={48} /></div>
        <h3>LibreTune</h3>
        <p className="dialog-version">
          Version {buildInfo?.version ?? 'unknown'}
        </p>
        <p className="dialog-build">
          Build {buildInfo?.build_id ?? 'unknown'}
        </p>

        <p>Open-source ECU tuning software compatible with standard INI definition files.</p>

        <div className="dialog-about-links">
          <a
            href="#"
            onClick={(e) => { e.preventDefault(); openUrl('https://github.com/RallyPat/LibreTune'); }}
          >
            GitHub
          </a>
          <a
            href="#"
            onClick={(e) => { e.preventDefault(); openUrl('https://github.com/RallyPat/LibreTune/tree/main/docs'); }}
          >
            Documentation
          </a>
        </div>

        <p className="dialog-license">
          Licensed under GPL-2.0
        </p>
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose}>Close</Button>
      </Dialog.Footer>
    </Dialog>
  );
}
