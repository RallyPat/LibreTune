import { useState, useRef, useEffect, useCallback, CSSProperties, KeyboardEvent } from 'react';
import { Check } from 'lucide-react';
import { MenuItem } from './TunerLayout';
import './MenuBar.css';

interface MenuBarProps {
  items: MenuItem[];
}

export function MenuBar({ items }: MenuBarProps) {
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [moreOpen, setMoreOpen] = useState(false);
  const [dropdownLeft, setDropdownLeft] = useState(0);
  const menuBarRef = useRef<HTMLDivElement>(null);
  const moreWrapRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  // The dropdown for the open top-level menu renders outside the scrollable
  // `.menubar-items` row (see below) so horizontal scrolling can't clip it
  // vertically; this measures where to anchor it under the clicked button.
  const updateDropdownPosition = useCallback((itemId: string) => {
    const btn = itemRefs.current[itemId];
    const bar = menuBarRef.current;
    if (btn && bar) {
      setDropdownLeft(btn.getBoundingClientRect().left - bar.getBoundingClientRect().left);
    }
  }, []);

  // Parse accelerator from label (e.g., "&File" -> { label: "File", accelerator: "F" })
  const parseLabel = (label: string) => {
    const match = label.match(/&(.)/);
    if (match) {
      const char = match[1];
      const parts = label.split('&' + char);
      return {
        before: parts[0],
        accelerator: char,
        after: parts.slice(1).join(char),
      };
    }
    return { before: label, accelerator: null, after: '' };
  };

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuBarRef.current && !menuBarRef.current.contains(e.target as Node)) {
        setOpenMenuId(null);
        setFocusedIndex(-1);
        setMoreOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Handle Alt+key accelerators
  useEffect(() => {
    const handleKeyDown = (e: globalThis.KeyboardEvent) => {
      if (e.altKey && !e.ctrlKey && !e.metaKey) {
        const key = e.key.toLowerCase();
        const index = items.findIndex((item) => {
          const parsed = parseLabel(item.label);
          return parsed.accelerator?.toLowerCase() === key;
        });
        if (index !== -1) {
          e.preventDefault();
          updateDropdownPosition(items[index].id);
          setOpenMenuId(items[index].id);
          setFocusedIndex(index);
        }
      }
      // Escape closes menu
      if (e.key === 'Escape' && openMenuId) {
        setOpenMenuId(null);
        setFocusedIndex(-1);
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [items, openMenuId, updateDropdownPosition]);

  const handleMenuClick = (item: MenuItem, index: number) => {
    if (openMenuId === item.id) {
      setOpenMenuId(null);
    } else {
      updateDropdownPosition(item.id);
      setOpenMenuId(item.id);
    }
    setFocusedIndex(index);
  };

  const handleMenuHover = (item: MenuItem, index: number) => {
    if (openMenuId !== null) {
      updateDropdownPosition(item.id);
      setOpenMenuId(item.id);
      setFocusedIndex(index);
    }
  };

  // Indexes of top-level items that can receive focus (skip separators).
  // Separators are non-interactive dividers; ArrowRight/ArrowLeft must hop
  // over them rather than landing on a non-focusable element.
  const focusableIndexes = items
    .map((item, i) => ({ item, i }))
    .filter(({ item }) => !item.separator)
    .map(({ i }) => i);

  const handleMenuKeyDown = (e: KeyboardEvent, index: number) => {
    switch (e.key) {
      case 'ArrowRight':
        e.preventDefault();
        {
          const pos = focusableIndexes.indexOf(index);
          const nextIndex = focusableIndexes[(pos + 1) % focusableIndexes.length];
          setFocusedIndex(nextIndex);
          if (openMenuId) {
            updateDropdownPosition(items[nextIndex].id);
            setOpenMenuId(items[nextIndex].id);
          }
        }
        break;
      case 'ArrowLeft':
        e.preventDefault();
        {
          const pos = focusableIndexes.indexOf(index);
          const prevIndex =
            focusableIndexes[(pos - 1 + focusableIndexes.length) % focusableIndexes.length];
          setFocusedIndex(prevIndex);
          if (openMenuId) {
            updateDropdownPosition(items[prevIndex].id);
            setOpenMenuId(items[prevIndex].id);
          }
        }
        break;
      case 'ArrowDown':
      case 'Enter':
      case ' ':
        e.preventDefault();
        updateDropdownPosition(items[index].id);
        setOpenMenuId(items[index].id);
        break;
    }
  };

  const closeMenu = useCallback(() => {
    setOpenMenuId(null);
    setFocusedIndex(-1);
  }, []);

  // Closing on scroll avoids the dropdown drifting away from its button
  // (its position is measured once, on open, not tracked continuously).
  const handleItemsScroll = useCallback(() => {
    setOpenMenuId(null);
    setFocusedIndex(-1);
  }, []);

  const openItem = openMenuId ? items.find((item) => item.id === openMenuId) : undefined;

  return (
    <div className="menubar" ref={menuBarRef}>
      <div className="menubar-items" onScroll={handleItemsScroll} role="menubar">
        {items.map((item, index) => {
          // Top-level separators render as a non-focusable vertical rule.
          if (item.separator) {
            return (
              <div
                key={item.id || `sep-${index}`}
                className="menubar-separator"
                role="separator"
                aria-orientation="vertical"
              />
            );
          }
          const parsed = parseLabel(item.label);
          const isOpen = openMenuId === item.id;

          return (
            <div key={item.id} className="menubar-item-wrapper" role="none">
              <button
                ref={(el) => {
                  itemRefs.current[item.id] = el;
                }}
                className={`menubar-item ${isOpen ? 'menubar-item-open' : ''} ${
                  focusedIndex === index ? 'menubar-item-focused' : ''
                }`}
                onClick={() => handleMenuClick(item, index)}
                onMouseEnter={() => handleMenuHover(item, index)}
                onKeyDown={(e) => handleMenuKeyDown(e, index)}
                role="menuitem"
                aria-haspopup="true"
                aria-expanded={isOpen}
              >
                {parsed.before}
                {parsed.accelerator && (
                  <span className="menubar-accelerator">{parsed.accelerator}</span>
                )}
                {parsed.after}
              </button>
            </div>
          );
        })}
      </div>

      {/* Rendered outside `.menubar-items` (whose horizontal scroll clips
          any absolutely-positioned child vertically) and anchored via a
          measured left offset instead. */}
      {openItem && openItem.items && (
        <MenuDropdown
          items={openItem.items}
          onDismissAll={closeMenu}
          level={0}
          style={{ left: dropdownLeft }}
        />
      )}

      {/* Reachability fallback for narrow windows: the row above scrolls
          horizontally (menubar-items { overflow-x: auto }), and this button
          lists every menu regardless of scroll position — same pattern as
          the tab bar's "▾ More tabs" menu. */}
      <div className="menubar-more-wrap" ref={moreWrapRef}>
        <button
          className={`menubar-more ${moreOpen ? 'menubar-item-open' : ''}`}
          onClick={() => setMoreOpen((v) => !v)}
          aria-haspopup="true"
          aria-expanded={moreOpen}
          aria-label="More menus"
          title="More menus"
        >
          ⋯
        </button>
        {moreOpen && (
          <MenuDropdown
            items={items.filter((item) => !item.separator)}
            onDismissAll={() => {
              closeMenu();
              setMoreOpen(false);
            }}
            level={0}
          />
        )}
      </div>
    </div>
  );
}

interface MenuDropdownProps {
  items: MenuItem[];
  onDismissAll: () => void;
  onCloseSubmenu?: () => void;
  level?: number;
  style?: CSSProperties;
}

function MenuDropdown({ items, onDismissAll, onCloseSubmenu, level = 0, style }: MenuDropdownProps) {
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [openSubmenuId, setOpenSubmenuId] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Focus first item on mount
  useEffect(() => {
    const firstFocusable = items.findIndex(
      (item) => !item.separator && !item.disabled
    );
    setFocusedIndex(firstFocusable >= 0 ? firstFocusable : 0);
  }, [items]);

  const handleItemClick = (item: MenuItem) => {
    if (item.disabled || item.separator) return;

    if (item.items && item.items.length > 0) {
      setOpenSubmenuId(openSubmenuId === item.id ? null : item.id);
    } else {
      item.onClick?.();
      onDismissAll();
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    const focusableItems = items
      .map((item, i) => ({ item, index: i }))
      .filter(({ item }) => !item.separator && !item.disabled);

    const currentFocusableIndex = focusableItems.findIndex(
      ({ index }) => index === focusedIndex
    );

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        e.stopPropagation();
        const nextIndex =
          (currentFocusableIndex + 1) % focusableItems.length;
        setFocusedIndex(focusableItems[nextIndex].index);
        break;
      case 'ArrowUp':
        e.preventDefault();
        e.stopPropagation();
        const prevIndex =
          (currentFocusableIndex - 1 + focusableItems.length) %
          focusableItems.length;
        setFocusedIndex(focusableItems[prevIndex].index);
        break;
      case 'ArrowRight':
        e.stopPropagation();
        const currentItem = items[focusedIndex];
        if (currentItem?.items && currentItem.items.length > 0) {
          e.preventDefault();
          setOpenSubmenuId(currentItem.id);
        }
        break;
      case 'ArrowLeft':
        e.stopPropagation();
        if (level > 0 && onCloseSubmenu) {
          e.preventDefault();
          onCloseSubmenu();
        }
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        e.stopPropagation();
        handleItemClick(items[focusedIndex]);
        break;
      case 'Escape':
        e.preventDefault();
        e.stopPropagation();
        if (level > 0 && onCloseSubmenu) {
          onCloseSubmenu();
        } else {
          onDismissAll();
        }
        break;
    }
  };

  // Parse accelerator and shortcut from menu item
  const parseMenuItem = (label: string) => {
    // Check for shortcut (e.g., "Save\tCtrl+S")
    const [text, shortcut] = label.split('\t');
    const parsed = parseAccelerator(text);
    return { ...parsed, shortcut };
  };

  const parseAccelerator = (label: string) => {
    const match = label.match(/&(.)/);
    if (match) {
      const char = match[1];
      const parts = label.split('&' + char);
      return {
        before: parts[0],
        accelerator: char,
        after: parts.slice(1).join(char),
      };
    }
    return { before: label, accelerator: null, after: '' };
  };

  return (
    <div
      className={`menu-dropdown menu-dropdown-level-${level}`}
      style={style}
      ref={dropdownRef}
      role="menu"
      onKeyDown={handleKeyDown}
      tabIndex={-1}
    >
      {items.map((item, index) => {
        if (item.separator) {
          return <div key={`sep-${index}`} className="menu-separator" role="separator" />;
        }

        const parsed = parseMenuItem(item.label);
        const hasSubmenu = item.items && item.items.length > 0;
        const isOpen = openSubmenuId === item.id;

        return (
          <div key={item.id} className="menu-item-wrapper">
            <button
              className={`menu-item ${
                focusedIndex === index ? 'menu-item-focused' : ''
              } ${item.disabled ? 'menu-item-disabled' : ''} ${
                item.checked ? 'menu-item-checked' : ''
              }`}
              onClick={() => handleItemClick(item)}
              onMouseEnter={() => {
                setFocusedIndex(index);
                if (hasSubmenu) {
                  setOpenSubmenuId(item.id);
                } else {
                  setOpenSubmenuId(null);
                }
              }}
              disabled={item.disabled}
              role="menuitem"
              aria-haspopup={hasSubmenu}
              aria-expanded={isOpen}
            >
              <span className="menu-item-check">
                {item.checked && <Check size={12} />}
              </span>
              <span className="menu-item-label">
                {parsed.before}
                {parsed.accelerator && (
                  <span className="menu-item-accelerator">{parsed.accelerator}</span>
                )}
                {parsed.after}
              </span>
              {parsed.shortcut && (
                <span className="menu-item-shortcut">{parsed.shortcut}</span>
              )}
              {hasSubmenu && (
                <span className="menu-item-arrow">▶</span>
              )}
            </button>
            
            {isOpen && hasSubmenu && (
              <MenuDropdown
                items={item.items!}
                onDismissAll={onDismissAll}
                onCloseSubmenu={() => setOpenSubmenuId(null)}
                level={level + 1}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}
