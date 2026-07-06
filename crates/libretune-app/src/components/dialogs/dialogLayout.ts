import type { DialogComponent } from './types';

/** Panel names that reference a 2D/3D table editor (Tbl/Map suffix). */
export function isEmbeddedTablePanelName(name: string | undefined): boolean {
  if (!name) return false;
  return /(?:Tbl|Map)$/i.test(name);
}

export function isTableComponent(comp: DialogComponent): boolean {
  if (comp.type === 'Table' && comp.name) return true;
  if (comp.type === 'Panel' && isEmbeddedTablePanelName(comp.name)) return true;
  return false;
}

function isConfigComponent(comp: DialogComponent): boolean {
  if (isTableComponent(comp)) return false;
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
 * Generic PWM and other table dialogs keep the default stacked layout.
 */
export function isUserTableDialog(dialogName: string, components: DialogComponent[]): boolean {
  if (/^gppwm/i.test(dialogName)) return false;

  if (/^userTable\d+TblSettings$/i.test(dialogName)) return true;
  if (/^usertables\d+TblSettings$/i.test(dialogName)) return true;

  // Fallback when dialog name differs but table/settings components match
  const hasUserTable = components.some((c) => {
    const name = c.name ?? '';
    return (
      (c.type === 'Table' || c.type === 'Panel') &&
      (/^userTable\d+Tbl$/i.test(name) || /^usertables\d+Tbl$/i.test(name))
    );
  });
  const hasSettingsPanel = components.some(
    (c) => c.type === 'Panel' && /settings/i.test(c.name ?? ''),
  );

  return hasUserTable && (hasSettingsPanel || components.some((c) => c.type === 'Field'));
}

/**
 * Side-by-side layout for User Tables: config West (left), table East (right).
 * Does not apply to Generic PWM (gppwm*) or other dialogs.
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

  const hasExplicitPosition = components.some((c) => {
    const pos = c.position?.toLowerCase();
    return pos === 'west' || pos === 'east';
  });
  if (hasExplicitPosition) {
    return { components, hasTableSplit: false };
  }

  const hasTable = components.some(isTableComponent);
  const hasConfig = components.some(isConfigComponent);
  if (!hasTable || !hasConfig) {
    return { components, hasTableSplit: false };
  }

  const updated = components.map((comp) => {
    if (comp.position) return comp;
    if (isTableComponent(comp)) {
      return { ...comp, position: 'East' };
    }
    if (isConfigComponent(comp)) {
      return { ...comp, position: 'West' };
    }
    return comp;
  });

  return { components: updated, hasTableSplit: true };
}

export function rowHasTableSplitLayout(
  row: { west: DialogComponent[]; east: DialogComponent[] },
): boolean {
  const hasTableEast = row.east.some(isTableComponent);
  const hasConfigWest = row.west.some(isConfigComponent);
  return hasTableEast && hasConfigWest;
}
