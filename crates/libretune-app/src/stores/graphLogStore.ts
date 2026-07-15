/**
 * Zustand store for the Graph Log view (TunerStudio-style stacked strip charts).
 *
 * Layout model: the user has multiple renameable tabs; each tab holds a fixed
 * number of stacked panes sharing one time axis; each pane plots up to two
 * channels — one on the left axis, one on the right — with independent scales.
 *
 * Persisted to localStorage so tab layouts and scale overrides survive restarts.
 */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export const PANES_PER_TAB = 4;

export type AxisSide = 'left' | 'right';

export interface ChannelSlot {
  /** Output channel name, or null when the slot is unassigned */
  channel: string | null;
  /** Fixed scale bounds (used when auto is false) */
  min: number;
  max: number;
  /** Auto-scale to the visible data instead of the fixed bounds */
  auto: boolean;
  color: string;
}

export interface GraphPane {
  left: ChannelSlot;
  right: ChannelSlot;
  hidden: boolean;
}

export interface GraphTab {
  id: string;
  name: string;
  panes: GraphPane[];
}

/** Left/right line colors per pane, loosely matching the TunerStudio palette */
export const PANE_COLORS: Array<{ left: string; right: string }> = [
  { left: '#e05252', right: '#4f8fe8' },
  { left: '#3dba6f', right: '#c069d8' },
  { left: '#e0a030', right: '#7f7ff0' },
  { left: '#d8d8d8', right: '#2fc4c4' },
];

const emptySlot = (side: AxisSide, paneIndex: number): ChannelSlot => ({
  channel: null,
  min: 0,
  max: 100,
  auto: true,
  color: PANE_COLORS[paneIndex % PANE_COLORS.length][side],
});

const makeSlot = (
  side: AxisSide,
  paneIndex: number,
  channel: string,
  min?: number,
  max?: number,
): ChannelSlot => ({
  channel,
  min: min ?? 0,
  max: max ?? 100,
  auto: min === undefined || max === undefined,
  color: PANE_COLORS[paneIndex % PANE_COLORS.length][side],
});

const makePane = (paneIndex: number, left?: ChannelSlot, right?: ChannelSlot): GraphPane => ({
  left: left ?? emptySlot('left', paneIndex),
  right: right ?? emptySlot('right', paneIndex),
  hidden: false,
});

let nextId = Date.now();
const newTabId = () => `graphtab-${nextId++}`;

/** Default pages mirroring the TunerStudio Fuel/Ignition presets.
 *  Channel names follow the rusEFI INI conventions; slots for channels a
 *  given INI doesn't define simply show no data until reassigned.
 */
const defaultTabs = (): GraphTab[] => [
  {
    id: newTabId(),
    name: 'Fuel',
    panes: [
      makePane(0, makeSlot('left', 0, 'RPM', 0, 9000), makeSlot('right', 0, 'MAP', 0, 400)),
      makePane(1, makeSlot('left', 1, 'lambda', 0.7, 1.3), makeSlot('right', 1, 'pulseWidth', 0, 25)),
      makePane(2, makeSlot('left', 2, 've', 0, 200), makeSlot('right', 2, 'tps', 0, 100)),
      makePane(3, makeSlot('left', 3, 'coolant', -40, 140), makeSlot('right', 3, 'iat', -40, 120)),
    ],
  },
  {
    id: newTabId(),
    name: 'Ignition',
    panes: [
      makePane(0, makeSlot('left', 0, 'RPM', 0, 9000), makeSlot('right', 0, 'timing', -10, 60)),
      makePane(1, makeSlot('left', 1, 'MAP', 0, 400), makeSlot('right', 1, 'tps', 0, 100)),
      makePane(2),
      makePane(3),
    ],
  },
];

interface GraphLogState {
  tabs: GraphTab[];
  activeTabId: string;
  /** Visible time window in seconds for live scrolling */
  timeWindowSec: number;

  addTab: () => void;
  removeTab: (tabId: string) => void;
  renameTab: (tabId: string, name: string) => void;
  setActiveTab: (tabId: string) => void;
  setTimeWindow: (seconds: number) => void;
  updateSlot: (
    tabId: string,
    paneIndex: number,
    side: AxisSide,
    patch: Partial<ChannelSlot>,
  ) => void;
  setPaneHidden: (tabId: string, paneIndex: number, hidden: boolean) => void;
}

export const useGraphLogStore = create<GraphLogState>()(
  persist(
    (set) => ({
      tabs: defaultTabs(),
      activeTabId: '',
      timeWindowSec: 30,

      addTab: () =>
        set((state) => {
          const tab: GraphTab = {
            id: newTabId(),
            name: `Graph ${state.tabs.length + 1}`,
            panes: [makePane(0), makePane(1), makePane(2), makePane(3)],
          };
          return { tabs: [...state.tabs, tab], activeTabId: tab.id };
        }),

      removeTab: (tabId) =>
        set((state) => {
          if (state.tabs.length <= 1) return state;
          const tabs = state.tabs.filter((t) => t.id !== tabId);
          const activeTabId =
            state.activeTabId === tabId ? tabs[0].id : state.activeTabId;
          return { tabs, activeTabId };
        }),

      renameTab: (tabId, name) =>
        set((state) => ({
          tabs: state.tabs.map((t) =>
            t.id === tabId ? { ...t, name: name.trim() || t.name } : t,
          ),
        })),

      setActiveTab: (tabId) => set({ activeTabId: tabId }),

      setTimeWindow: (seconds) =>
        set({ timeWindowSec: Math.min(300, Math.max(5, seconds)) }),

      updateSlot: (tabId, paneIndex, side, patch) =>
        set((state) => ({
          tabs: state.tabs.map((t) => {
            if (t.id !== tabId) return t;
            const panes = t.panes.map((pane, i) =>
              i === paneIndex ? { ...pane, [side]: { ...pane[side], ...patch } } : pane,
            );
            return { ...t, panes };
          }),
        })),

      setPaneHidden: (tabId, paneIndex, hidden) =>
        set((state) => ({
          tabs: state.tabs.map((t) => {
            if (t.id !== tabId) return t;
            const panes = t.panes.map((pane, i) =>
              i === paneIndex ? { ...pane, hidden } : pane,
            );
            return { ...t, panes };
          }),
        })),
    }),
    {
      name: 'libretune-graph-log',
      version: 1,
    },
  ),
);

/** Resolve the active tab, falling back to the first tab when the persisted
 *  activeTabId no longer exists. */
export const selectActiveTab = (state: GraphLogState): GraphTab =>
  state.tabs.find((t) => t.id === state.activeTabId) ?? state.tabs[0];
