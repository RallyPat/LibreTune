import type { DialogComponent } from './types';

/** Panel names that reference a 2D/3D table editor (Tbl/Map suffix). */
export function isEmbeddedTablePanelName(name: string | undefined): boolean {
  if (!name) return false;
  return /(?:Tbl|Map)$/i.test(name);
}

export function isUserTableName(name: string | undefined): boolean {
  if (!name) return false;
  return /userTable\s*\d+/i.test(name) || /usertables?\d+/i.test(name);
}

export function isUserTableLiveChannel(name: string | undefined): boolean {
  if (!name) return false;
  return /^userTable(?:[XY]Axis|Output)\d+$/i.test(name);
}

/** GPPWM live channels rendered beside the table via TableLiveReadout — skip INI runtimeValue duplicates. */
export function isGppwmLiveChannel(name: string | undefined): boolean {
  if (!name) return false;
  return /^gppwm(?:Switch|XAxis|YAxis|Output)\d+$/i.test(name);
}

export function isEmbeddedTableLiveChannel(name: string | undefined): boolean {
  return isUserTableLiveChannel(name) || isGppwmLiveChannel(name);
}

export function isTableComponent(comp: DialogComponent): boolean {
  if (comp.type === 'Table' && comp.name) return true;
  if (comp.type === 'Panel' && isEmbeddedTablePanelName(comp.name)) return true;
  return false;
}

function isConfigComponent(comp: DialogComponent): boolean {
  if (isTableComponent(comp)) return false;
  if (comp.type === 'RuntimeValue') return false;
  return (
    comp.type === 'Field' ||
    comp.type === 'Label' ||
    comp.type === 'CommandButton' ||
    comp.type === 'Indicator' ||
    (comp.type === 'Panel' && !!comp.name)
  );
}

/**
 * User Tables only (Advanced → User tables 1–8).
 */
export function isUserTableDialog(dialogName: string, components: DialogComponent[]): boolean {
  if (/^gppwm/i.test(dialogName)) return false;
  if (/^scriptTable/i.test(dialogName)) return false;

  if (/userTable\s*\d+TblSettings$/i.test(dialogName)) return true;
  if (/usertables?\d+TblSettings$/i.test(dialogName)) return true;

  const hasUserTable = components.some(
    (c) => isTableComponent(c) && isUserTableName(c.name),
  );
  if (!hasUserTable) return false;

  return components.some(isConfigComponent);
}

/**
 * Stacked layout for User Tables (same as Generic PWM): config on top, table below.
 * Strips INI West/East/Center positions and puts settings before the table.
 */
export function applyTableSettingsLayout(
  dialogName: string,
  components: DialogComponent[],
): {
  components: DialogComponent[];
  hasTableSplit: boolean;
} {
  if (!isUserTableDialog(dialogName, components)) {
    return { components, hasTableSplit: false };
  }

  const hasTable = components.some(isTableComponent);
  const hasConfig = components.some(isConfigComponent);
  if (!hasTable || !hasConfig) {
    return { components, hasTableSplit: false };
  }

  const config = components.filter(isConfigComponent).map((comp) => {
    const { position: _position, ...rest } = comp;
    return rest as DialogComponent;
  });
  const tables = components.filter(isTableComponent).map((comp) => {
    const { position: _position, ...rest } = comp;
    return rest as DialogComponent;
  });
  const other = components
    .filter((c) => !isConfigComponent(c) && !isTableComponent(c))
    .map((comp) => {
      const { position: _position, ...rest } = comp;
      return rest as DialogComponent;
    });

  return {
    components: [...config, ...other, ...tables],
    hasTableSplit: false,
  };
}

export type DialogComponentRow = {
  west: DialogComponent[];
  east: DialogComponent[];
  unpositioned: DialogComponent[];
};

/** Group dialog components into West/East rows for grid layout. */
export function organizeComponents(components: DialogComponent[]): DialogComponentRow[] {
  const rows: DialogComponentRow[] = [];
  let currentRow: DialogComponentRow | null = null;

  for (const comp of components) {
    const position = comp.position?.toLowerCase();

    if (position === 'west' || position === 'east') {
      if (!currentRow || currentRow.unpositioned.length > 0) {
        currentRow = { west: [], east: [], unpositioned: [] };
        rows.push(currentRow);
      }

      if (position === 'west') {
        currentRow.west.push(comp);
      } else {
        currentRow.east.push(comp);
      }
    } else {
      if (!currentRow) {
        currentRow = { west: [], east: [], unpositioned: [] };
        rows.push(currentRow);
      }
      currentRow.unpositioned.push(comp);
    }
  }

  return rows;
}

export function rowHasTableSplitLayout(row: DialogComponentRow): boolean {
  const hasTableEast = row.east.some(isTableComponent);
  const hasConfigWest = row.west.some(isConfigComponent);
  return hasTableEast && hasConfigWest;
}

/** Bench / hardware test dialogs (Spark, Injector, etc.) */
export function isHardwareTestDialog(dialogName: string): boolean {
  return /^ioTest$/i.test(dialogName) || /^injTest$/i.test(dialogName);
}

/** Reference gauge panel embedded in hardware test (injTest_r). */
export function isReferenceGaugePanel(panelName: string | undefined): boolean {
  if (!panelName) return false;
  return /^injTest_r$/i.test(panelName);
}

/** Shell dialogs: settings West, live state East (WMI, launch control, …). */
const CONFIG_LIVE_STATE_DIALOGS = new Set([
  'wmicontroldialog',
  'smlaunchcontrol',
]);

export function isConfigLiveStateDialog(dialogName: string): boolean {
  return CONFIG_LIVE_STATE_DIALOGS.has(dialogName.toLowerCase());
}

/** Gate expression for live-state side panels when the INI omits a panel condition. */
export function inferLiveStateGateExpression(panelName: string): string | null {
  const lower = panelName.toLowerCase();
  if (lower === 'wmilivestatedialog') return 'isWmiEnabled';
  if (lower === 'launch_control_statedialog') return 'launchControlEnabled';
  if (lower.endsWith('livestatedialog')) {
    if (lower.includes('wmi')) return 'isWmiEnabled';
  }
  if (lower.endsWith('_statedialog') || lower.endsWith('statedialog')) {
    const base = panelName.replace(/Dialog$/i, '');
    if (/launch/i.test(base)) return 'launchControlEnabled';
  }
  return null;
}

export function rowHasConfigLiveStateSplit(row: DialogComponentRow): boolean {
  const west = row.west.length > 0;
  const east = row.east.length > 0;
  if (!west || !east) return false;
  const eastPanels = row.east.filter((c) => c.type === 'Panel' && c.name);
  return eastPanels.some((c) => inferLiveStateGateExpression(c.name!) !== null);
}

/**
 * Preserve West/East layout for config + live-state shell dialogs.
 * Strips Center-only noise so WMI / launch control render as a two-column split.
 */
export function applyConfigLiveStateLayout(
  dialogName: string,
  components: DialogComponent[],
): { components: DialogComponent[]; useConfigLiveSplit: boolean } {
  if (!isConfigLiveStateDialog(dialogName)) {
    return { components, useConfigLiveSplit: false };
  }

  const west = components.filter((c) => c.position?.toLowerCase() === 'west');
  const east = components.filter((c) => c.position?.toLowerCase() === 'east');
  const rest = components.filter((c) => {
    const pos = c.position?.toLowerCase();
    return pos !== 'west' && pos !== 'east';
  });

  if (west.length === 0 || east.length === 0) {
    return { components, useConfigLiveSplit: false };
  }

  return {
    components: [...west, ...east, ...rest],
    useConfigLiveSplit: true,
  };
}

/** Command-only panels grouped on the first row (similar height). */
const HARDWARE_TEST_COMPACT_PANELS = new Set([
  'testspark',
  'testinjectors',
  'testluaout',
  'testmisc',
]);

/** Settings / gauge panels on the second row (often taller). */
const HARDWARE_TEST_AUX_PANELS = new Set(['testother', 'injtest_r']);

export function partitionHardwareTestComponents(components: DialogComponent[]): {
  compact: DialogComponent[];
  auxiliary: DialogComponent[];
} {
  const compact: DialogComponent[] = [];
  const auxiliary: DialogComponent[] = [];
  const overflow: DialogComponent[] = [];

  for (const comp of components) {
    const panelName =
      comp.type === 'Panel' && comp.name ? comp.name.toLowerCase() : '';
    if (HARDWARE_TEST_COMPACT_PANELS.has(panelName)) {
      compact.push(comp);
    } else if (HARDWARE_TEST_AUX_PANELS.has(panelName)) {
      auxiliary.push(comp);
    } else {
      overflow.push(comp);
    }
  }

  const orderIndex = (name: string, order: string[]) => {
    const idx = order.indexOf(name);
    return idx === -1 ? order.length : idx;
  };

  const compactOrder = ['testspark', 'testinjectors', 'testluaout', 'testmisc'];
  compact.sort(
    (a, b) =>
      orderIndex((a.type === 'Panel' && a.name ? a.name : '').toLowerCase(), compactOrder) -
      orderIndex((b.type === 'Panel' && b.name ? b.name : '').toLowerCase(), compactOrder),
  );

  const auxOrder = ['testother', 'injtest_r'];
  auxiliary.sort(
    (a, b) =>
      orderIndex((a.type === 'Panel' && a.name ? a.name : '').toLowerCase(), auxOrder) -
      orderIndex((b.type === 'Panel' && b.name ? b.name : '').toLowerCase(), auxOrder),
  );

  return {
    compact,
    auxiliary: [...auxiliary, ...overflow],
  };
}

/** Nested panel whose only interactive content is controller command buttons. */
export function isCommandButtonPanel(components: DialogComponent[]): boolean {
  const interactive = components.filter(
    (c) =>
      c.type === 'CommandButton' ||
      (c.type === 'Field' && !!c.name) ||
      c.type === 'Table' ||
      c.type === 'Panel',
  );
  if (interactive.length === 0) return false;
  return interactive.every((c) => c.type === 'CommandButton');
}
