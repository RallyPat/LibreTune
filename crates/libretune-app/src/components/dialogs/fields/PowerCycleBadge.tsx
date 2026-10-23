import { Power } from 'lucide-react';

/**
 * Badge marking a constant listed in the INI's [ConstantsExtensions]
 * `requiresPowerCycle` — changing it only takes effect after the ECU is
 * power-cycled. Rendered next to the field label.
 */
export function PowerCycleBadge() {
  return (
    <span
      className="power-cycle-badge"
      title="Requires power cycle: restart the ECU for this change to take effect"
    >
      <Power size={11} />
      <span className="power-cycle-badge-text">power cycle</span>
    </span>
  );
}
