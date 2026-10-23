import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import OnlineIniDialog from '../OnlineIniDialog';
import ConnectEcuWizard from '../ConnectEcuWizard';
import type { IniEntry } from '../../../types/app';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invoke(cmd, args),
}));

// Both dialogs open a native file picker for the local .ini path.
const openDialog = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: (options?: unknown) => openDialog(options),
}));

const localEntry: IniEntry = {
  id: 'ini-42',
  name: 'myCustomFirmware.ini',
  signature: 'speeduino v4.4 custom',
  path: 'C:/definitions/myCustomFirmware.ini',
};

beforeEach(() => {
  invoke.mockReset();
  openDialog.mockReset();
});

describe('OnlineIniDialog local file import', () => {
  function renderDialog(onIniImported?: (e: IniEntry) => void) {
    return render(
      <OnlineIniDialog isOpen onClose={() => {}} onIniImported={onIniImported} />,
    );
  }

  it('imports a user-provided .ini, makes it the project INI and notifies the parent', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'search_online_inis') {
        return { entries: [], last_updated: null, refreshed: false };
      }
      if (cmd === 'import_ini') return localEntry;
      if (cmd === 'update_project_ini') return 'ok';
      return undefined;
    });
    openDialog.mockResolvedValue('C:/definitions/myCustomFirmware.ini');
    const onIniImported = vi.fn();

    renderDialog(onIniImported);
    await userEvent.click(await screen.findByRole('button', { name: /use local file/i }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('import_ini', {
      sourcePath: 'C:/definitions/myCustomFirmware.ini',
    }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('update_project_ini', {
      iniPath: localEntry.path,
      forceResync: true,
    }));
    expect(onIniImported).toHaveBeenCalledWith(localEntry);
    expect(await screen.findByText(/Loaded myCustomFirmware\.ini/)).toBeInTheDocument();
  });

  it('does nothing when the file picker is cancelled', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'search_online_inis') {
        return { entries: [], last_updated: null, refreshed: false };
      }
      return undefined;
    });
    openDialog.mockResolvedValue(null);

    renderDialog();
    await userEvent.click(await screen.findByRole('button', { name: /use local file/i }));

    // Give any erroneous invocation a chance to run.
    await new Promise((r) => setTimeout(r, 10));
    expect(invoke).not.toHaveBeenCalledWith('import_ini', expect.anything());
  });

  it('surfaces an import failure', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'search_online_inis') {
        return { entries: [], last_updated: null, refreshed: false };
      }
      if (cmd === 'import_ini') throw new Error('not a valid INI');
      return undefined;
    });
    openDialog.mockResolvedValue('C:/bad/file.ini');

    renderDialog();
    await userEvent.click(await screen.findByRole('button', { name: /use local file/i }));

    expect(await screen.findByText(/not a valid INI/)).toBeInTheDocument();
  });
});

describe('ConnectEcuWizard offline local .ini browse', () => {
  function renderWizard(onIniImported?: (e: IniEntry) => void) {
    return render(
      <ConnectEcuWizard
        isOpen
        onClose={() => {}}
        inis={[]}
        onIniImported={onIniImported}
        onCreateProject={vi.fn(async () => true)}
        onConnect={vi.fn(async () => {})}
      />,
    );
  }

  /** Walk transport → (offline skips params/connect/resolve) → name step. */
  async function goToNameStep() {
    await userEvent.click(await screen.findByRole('radio', { name: /no connection right now/i }));
    await userEvent.click(await screen.findByRole('button', { name: 'Next' }));
    await screen.findByText('Project name');
  }

  it('offers a browse button on the offline name step and uses the imported INI', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'import_ini') return localEntry;
      return undefined;
    });
    openDialog.mockResolvedValue('C:/definitions/myCustomFirmware.ini');
    const onIniImported = vi.fn();

    renderWizard(onIniImported);
    await goToNameStep();

    await userEvent.click(await screen.findByRole('button', { name: /browse for \.ini file/i }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('import_ini', {
      sourcePath: 'C:/definitions/myCustomFirmware.ini',
    }));
    expect(onIniImported).toHaveBeenCalledWith(localEntry);
    // The resolved definition is shown on the name step (the name is the
    // <b> element inside the "Using … (manual)." sentence, which spans
    // multiple nodes so it can't be matched as one text run).
    expect(await screen.findByText('myCustomFirmware.ini', { selector: 'b' })).toBeInTheDocument();
  });

  it('surfaces an import failure on the offline name step', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'import_ini') throw new Error('unreadable definition');
      return undefined;
    });
    openDialog.mockResolvedValue('C:/bad/file.ini');

    renderWizard();
    await goToNameStep();

    await userEvent.click(await screen.findByRole('button', { name: /browse for \.ini file/i }));

    expect(await screen.findByText(/unreadable definition/)).toBeInTheDocument();
  });
});
