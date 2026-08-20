import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';

import HotkeyEditor from '../HotkeyEditor';

/**
 * Regression tests for the settings-dialog hotkey tab lock-up.
 *
 * HotkeyEditor used to sync child -> parent with an effect watching `hotkeys`
 * and parent -> child with an effect applying the `bindings` prop. When the
 * loaded bindings differed from the built-in defaults, each effect undid the
 * other's work with fresh object references every pass, and React aborted
 * with "Maximum update depth exceeded" (the Settings dialog then churned
 * forever). The fix reports changes to the parent only from user-action
 * handlers, so merely rendering with non-default bindings must be stable.
 */

/** Mirrors how SettingsDialog hosts the editor: state in, state out. */
function Host({ initial }: { initial: Record<string, string> }) {
  const [bindings, setBindings] = useState(initial);
  return <HotkeyEditor bindings={bindings} onChange={setBindings} />;
}

describe('HotkeyEditor', () => {
  it('renders without an update loop when saved bindings differ from defaults', () => {
    // A loaded binding that differs from the built-in default for the same
    // action is what used to start the ping-pong between the two effects.
    // If the loop regresses, render() itself throws before these assertions.
    render(<Host initial={{ 'table.setEqual': 'Ctrl+Equals' }} />);

    // The customized binding must actually be applied...
    expect(screen.getByText('Ctrl+Equals')).toBeTruthy();
    // ...and some default binding must still be present.
    expect(screen.getByText('ArrowUp')).toBeTruthy();
  });

  it('applies empty bindings and still renders the default table', () => {
    render(<Host initial={{}} />);
    expect(screen.getByText('ArrowUp')).toBeTruthy();
  });

  it('does not notify the parent spontaneously on mount', () => {
    const onChange = vi.fn();
    render(<HotkeyEditor bindings={{ 'table.setEqual': '=' }} onChange={onChange} />);
    expect(onChange).not.toHaveBeenCalled();
  });

  it('notifies the parent when the user edits a binding', () => {
    const onChange = vi.fn();
    render(<HotkeyEditor bindings={{ 'table.setEqual': '=' }} onChange={onChange} />);

    // Click the binding chip to enter edit mode, then type.
    fireEvent.click(screen.getByText('='));
    const input = screen.getByPlaceholderText('Press keys or type binding');
    fireEvent.change(input, { target: { value: 'Ctrl+Equals' } });

    expect(onChange).toHaveBeenCalledTimes(1);
    const reported = onChange.mock.calls[0][0] as Record<string, string>;
    expect(reported['table.setEqual']).toBe('Ctrl+Equals');
    // Every action is included in the report, not just the edited one.
    expect(Object.keys(reported).length).toBeGreaterThan(1);
  });

  it('notifies the parent when the user resets to defaults', () => {
    const onChange = vi.fn();
    render(<HotkeyEditor bindings={{ 'table.setEqual': 'Ctrl+Equals' }} onChange={onChange} />);

    fireEvent.click(screen.getByText('Reset to Defaults'));

    expect(onChange).toHaveBeenCalledTimes(1);
    const reported = onChange.mock.calls[0][0] as Record<string, string>;
    expect(reported['table.setEqual']).toBe('=');
  });
});
