/**
 * Every built-in tool the menu can ask for must actually open.
 *
 * `openTargetImpl` dispatches on an explicit `if (name === ...)` per tool, and
 * anything it does not recognise falls through to `get_table_data` and then
 * `get_curve_data` — so a tool with a menu entry but no branch does not fail
 * loudly, it goes looking for a *table* of that name and reports it missing.
 *
 * Log Analyze shipped exactly that way: menu entry, tab type, router case and
 * component all present and typechecking, with no branch here, so the menu item
 * raised "table not found". Types cannot catch it because the name is a string
 * on both sides. This test is the thing that does.
 */
import { vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { openTargetImpl, type OpenTargetDeps } from '../services/openTarget';
import type { TabContent } from '../types/app';

/** Tools reachable from the Tools menu that render a built-in view. */
const BUILT_IN_TOOLS: { name: string; type: TabContent['type'] }[] = [
  { name: 'autotune', type: 'autotune' },
  { name: 'datalog', type: 'datalog' },
  { name: 'log-analyze', type: 'log-analyze' },
  { name: 'tooth-logger', type: 'tooth-logger' },
  { name: 'composite-logger', type: 'composite-logger' },
  { name: 'och-status', type: 'och-status' },
];

function deps() {
  const state = {
    tabs: [] as OpenTargetDeps['tabs'],
    contents: {} as Record<string, TabContent>,
    active: '',
    toasts: [] as string[],
  };
  const d: OpenTargetDeps = {
    tabs: state.tabs,
    tabContents: state.contents,
    // Datalog refuses to open without a loggable INI, so grant it.
    iniCapabilities: {
      has_datalog_entries: true,
      has_output_channels: true,
    } as OpenTargetDeps['iniCapabilities'],
    setTabs: (t) => { state.tabs = t; },
    setTabContents: (c) => { state.contents = c; },
    setActiveTabId: (id) => { state.active = id; },
    setPortEditorAssignments: vi.fn(),
    showToast: (m) => { state.toasts.push(m); },
  };
  return { d, state };
}

beforeEach(() => {
  vi.clearAllMocks();
  // Any fall-through would land here. Rejecting makes that a visible failure
  // rather than a tab that quietly renders the wrong thing.
  (invoke as unknown as any).mockRejectedValue(new Error('not a table or curve'));
});

test.each(BUILT_IN_TOOLS)('opening %s gives a tab of the right type', async ({ name, type }) => {
  const { d, state } = deps();
  await openTargetImpl(d, name);

  expect(state.active).toBe(name);
  expect(state.contents[name]?.type).toBe(type);
  expect(state.tabs.map((t) => t.id)).toContain(name);
  // A built-in view must never have gone looking for a table of its own name.
  expect(invoke).not.toHaveBeenCalled();
  expect(state.toasts).toEqual([]);
});

test('an unknown target still falls through rather than opening a blank tab', async () => {
  const { d, state } = deps();
  await openTargetImpl(d, 'definitelyNotATable');
  expect(state.contents['definitelyNotATable']).toBeUndefined();
  expect(invoke).toHaveBeenCalled();
});
