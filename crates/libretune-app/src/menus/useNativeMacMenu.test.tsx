import { act, renderHook, waitFor } from "@testing-library/react";
import type { MenuOptions } from "@tauri-apps/api/menu";
import { expect, it, vi } from "vitest";
import { useNativeMacMenu } from "./useNativeMacMenu";

const { createMenu } = vi.hoisted(() => ({ createMenu: vi.fn() }));
vi.mock("@tauri-apps/api/menu", () => ({ Menu: { new: createMenu } }));

it("keeps Tools → Settings clickable when the previous native menu is released", async () => {
  // Tauri registers callbacks by ID and removes them when their items are dropped.
  let nextId = 0;
  const callbacks = new Map<string, () => void>();
  let installed: { close: () => Promise<void>; settingsId: string } | null = null;
  createMenu.mockImplementation(async ({ items }: MenuOptions) => {
    const tools = items?.find(item => "text" in item && item.text === "Tools");
    if (!tools || !("items" in tools) || typeof tools.items === "function") {
      throw new Error("Tools menu missing");
    }
    const settings = tools.items?.[0];
    if (!settings || !("action" in settings) || !settings.action) {
      throw new Error("Settings action missing");
    }

    const settingsId = settings.id ?? String(nextId++);
    const action = settings.action;
    callbacks.set(settingsId, () => action(settingsId));
    const menu = { settingsId, close: async () => { callbacks.delete(settingsId); } };
    return {
      // Closing the creation handle leaves the installed menu alive.
      close: async () => { if (installed !== menu) await menu.close(); },
      setAsAppMenu: async () => {
        const previous = installed;
        installed = menu;
        return previous;
      },
    };
  });

  const firstAction = vi.fn();
  const currentAction = vi.fn();
  const showToast = vi.fn();
  const { rerender } = renderHook(
    ({ action }) => useNativeMacMenu(true, [{
      id: "tools",
      label: "&Tools",
      items: [{ id: "settings", label: "Settings", onClick: action }],
    }], showToast),
    { initialProps: { action: firstAction } },
  );
  await waitFor(() => expect(installed).not.toBeNull());
  const previous = installed;

  rerender({ action: currentAction });
  await waitFor(() => expect(installed).not.toBe(previous));
  await waitFor(() => expect(callbacks.size).toBe(1));
  act(() => { if (installed) callbacks.get(installed.settingsId)?.(); });
  expect(currentAction).toHaveBeenCalledOnce();
  expect(firstAction).not.toHaveBeenCalled();
});
