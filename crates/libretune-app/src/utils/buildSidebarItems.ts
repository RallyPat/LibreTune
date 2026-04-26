import type { SidebarNode } from "../components/tuner-ui";
import type { BackendMenuItem } from "../types/app";

export type SidebarNodeWithType = SidebarNode & { itemType?: string };

/**
 * Recursively converts backend menu items into sidebar tree nodes.
 * Handles SubMenu, Table, Dialog, Std, Help types and propagates
 * visibility/enabled state from condition evaluation.
 */
export function buildSidebarItems(items: BackendMenuItem[], prefix: string): SidebarNodeWithType[] {
  return items
    .filter((item) => item.type !== "Separator")
    .map((item, idx) => {
      // Determine if item is disabled (visible=false from visibility condition evaluation)
      const isDisabled = item.visible === false;
      const disabledReason = isDisabled && item.visibility_condition
        ? `Condition not met: ${item.visibility_condition}`
        : undefined;

      if (item.type === "SubMenu" && item.items && item.items.length > 0) {
        // Recursively build children for SubMenu
        return {
          id: `${prefix}-submenu-${idx}`,
          label: item.label || "",
          type: "folder" as const,
          children: buildSidebarItems(item.items, `${prefix}-${idx}`),
          disabled: isDisabled,
          disabledReason,
        };
      }
      // Leaf item - Table, Dialog, Std, or Help
      let nodeType: string = "dialog";
      if (item.type === "Table") {
        nodeType = "table";
      } else if (item.type === "Help") {
        nodeType = "help";
      }
      return {
        id: item.target || `${prefix}-${idx}`,
        label: item.label || "",
        type: nodeType as "table" | "dialog" | "help",
        itemType: item.type, // Store original type for click handling
        disabled: isDisabled,
        disabledReason,
      };
    });
}
