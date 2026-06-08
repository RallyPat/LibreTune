import { useState, useRef, useEffect, useCallback, KeyboardEvent } from 'react';
import { MenuItem } from './TunerLayout';
import './MenuBar.css';

interface MenuBarProps {
  items: MenuItem[];
}

export function MenuBar({ items }: MenuBarProps) {
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const menuBarRef = useRef<HTMLDivElement>(null);

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
  }, [items, openMenuId]);

  const handleMenuClick = (item: MenuItem, index: number) => {
    if (openMenuId === item.id) {
      setOpenMenuId(null);
    } else {
      setOpenMenuId(item.id);
    }
    setFocusedIndex(index);
  };

  const handleMenuHover = (item: MenuItem, index: number) => {
    if (openMenuId !== null) {
      setOpenMenuId(item.id);
      setFocusedIndex(index);
    }
  };

  const handleMenuKeyDown = (e: KeyboardEvent, index: number) => {
    switch (e.key) {
      case 'ArrowRight':
        e.preventDefault();
        const nextIndex = (index + 1) % items.length;
        setFocusedIndex(nextIndex);
        if (openMenuId) {
          setOpenMenuId(items[nextIndex].id);
        }
        break;
      case 'ArrowLeft':
        e.preventDefault();
        const prevIndex = (index - 1 + items.length) % items.length;
        setFocusedIndex(prevIndex);
        if (openMenuId) {
          setOpenMenuId(items[prevIndex].id);
        }
        break;
      case 'ArrowDown':
      case 'Enter':
      case ' ':
        e.preventDefault();
        setOpenMenuId(items[index].id);
        break;
    }
  };

  const closeMenu = useCallback(() => {
    setOpenMenuId(null);
    setFocusedIndex(-1);
  }, []);

  return (
    <div className="menubar" ref={menuBarRef} role="menubar">
      {items.map((item, index) => {
        const parsed = parseLabel(item.label);
        const isOpen = openMenuId === item.id;
        
        return (
          <div key={item.id} className="menubar-item-wrapper">
            <button
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
            
            {isOpen && item.items && (
              <MenuDropdown
                items={item.items}
                onClose={closeMenu}
                parentLabel={item.label}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

interface MenuDropdownProps {
  items: MenuItem[];
  onClose: () => void;
  parentLabel: string;
  level?: number;
}

function MenuDropdown({ items, onClose, level = 0 }: MenuDropdownProps) {
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
    } else if (item.onClick) {
      item.onClick();
      onClose();
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
        if (level > 0) {
          e.preventDefault();
          onClose();
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
        onClose();
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
                {item.checked && '✓'}
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
                onClose={() => setOpenSubmenuId(null)}
                parentLabel={item.label}
                level={level + 1}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}
