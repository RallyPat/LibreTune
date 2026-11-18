import { useEffect, useRef, useState } from "react";
import { Menu, type MenuOptions, type PredefinedMenuItemOptions } from "@tauri-apps/api/menu";
import type { MenuItem } from "../components/tuner-ui";

type NativeItem = NonNullable<MenuOptions["items"]>[number];

function menuText(label: string): string {
  return label.split("\t", 1)[0].replace(/&(.)/g, "$1");
}

const editRoles: Record<string, PredefinedMenuItemOptions["item"]> = {
  undo: "Undo",
  redo: "Redo",
  cut: "Cut",
  copy: "Copy",
  paste: "Paste",
};

function convertItem(item: MenuItem, useEditRoles = false): NativeItem {
  const text = menuText(item.label);
  if (item.separator) return { item: "Separator" };
  if (useEditRoles && editRoles[item.id]) return { text, item: editRoles[item.id] };
  const enabled = !item.disabled;
  if (item.items) {
    return { text, enabled, items: item.items.map(child => convertItem(child)) };
  }
  // Let Tauri allocate a fresh ID. Dropping the old menu unregisters its
  // handlers by ID, which would also unregister replacements with reused IDs.
  const action = { text, enabled, action: item.onClick };
  return item.checked === undefined ? action : { ...action, checked: item.checked };
}

function createMacMenu(items: MenuItem[]): Promise<Menu> {
  const menus: NativeItem[] = [{
    text: "LibreTune",
    items: [
      { item: "Services" }, { item: "Separator" }, { item: "Hide" },
      { item: "HideOthers" }, { item: "ShowAll" }, { item: "Separator" }, { item: "Quit" },
    ],
  }];

  if (!items.some(item => item.id === "edit")) {
    menus.push({
      text: "Edit",
      items: [{ item: "Undo" }, { item: "Redo" }, { item: "Separator" },
        { item: "Cut" }, { item: "Copy" }, { item: "Paste" }, { item: "SelectAll" }],
    });
  }

  for (const item of items) {
    if (item.separator) continue;
    if (item.id === "help") {
      menus.push({
        text: "Window",
        items: [{ item: "Minimize" }, { item: "Maximize" }, { item: "CloseWindow" },
          { item: "Separator" }, { item: "BringAllToFront" }],
      });
    }
    const children = (item.items ?? []).map(child => convertItem(child, item.id === "edit"));
    if (item.id === "view") children.push({ item: "Separator" }, { item: "Fullscreen" });
    if (item.id === "edit") children.push({ item: "Separator" }, { item: "SelectAll" });
    menus.push({ text: menuText(item.label), enabled: !item.disabled, items: children });
  }
  return Menu.new({ items: menus });
}

export function useNativeMacMenu(
  enabled: boolean,
  items: MenuItem[],
  showToast: (message: string, kind: "error") => void,
): boolean {
  const [active, setActive] = useState(false);
  const queue = useRef(Promise.resolve());
  const original = useRef<Menu | null>(null);
  const installed = useRef(false);

  useEffect(() => {
    let cancelled = false;
    queue.current = queue.current.then(async () => {
      if (cancelled || (!enabled && !installed.current)) return;
      const menu = enabled ? await createMacMenu(items) : original.current ?? await Menu.default();
      try {
        if (cancelled) return;
        const previous = await menu.setAsAppMenu();
        if (enabled && !installed.current) {
          original.current = previous;
        } else {
          await previous?.close().catch(console.error);
        }
        if (!enabled) original.current = null;
        installed.current = enabled;
        setActive(enabled);
      } finally {
        if (menu !== original.current) await menu.close().catch(console.error);
      }
    }).catch(error => {
      if (!cancelled) showToast(`Could not switch the macOS menu: ${error}`, "error");
    });
    return () => { cancelled = true; };
  }, [enabled, items, showToast]);

  return active;
}
