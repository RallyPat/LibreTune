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
