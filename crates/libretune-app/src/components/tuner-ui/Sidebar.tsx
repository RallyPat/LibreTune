import React, { useState, useCallback, useRef, useMemo, MouseEvent, useEffect } from 'react';
import { SidebarNode } from './TunerLayout';
import './Sidebar.css';

type IconElement = React.ReactElement;

interface SidebarProps {
  items: SidebarNode[];
  width: number;
  onResize: (width: number) => void;
  /** Callback when an item is selected. highlightTerm is the search query if user clicked from search results. */
  onItemSelect: (item: SidebarNode, highlightTerm?: string) => void;
  /** Index of searchable content for deep search (target -> terms) */
  searchIndex?: Record<string, string[]>;
}

/** Recursively filter tree nodes by search query, preserving parent folders when children match */
function filterTree(nodes: SidebarNode[], query: string, searchIndex?: Record<string, string[]>): SidebarNode[] {
  if (!query.trim()) return nodes;
  
  const lowerQuery = query.toLowerCase();
  
  return nodes.reduce<SidebarNode[]>((acc, node) => {
    const labelMatches = node.label.toLowerCase().includes(lowerQuery);
    const idMatches = node.id.toLowerCase().includes(lowerQuery);
    
    // Deep search: check if any indexed content for this node matches
    const indexedTerms = searchIndex?.[node.id] || [];
    const contentMatches = indexedTerms.some(term => 
      term.toLowerCase().includes(lowerQuery)
    );
    
    if (node.children && node.children.length > 0) {
      const filteredChildren = filterTree(node.children, query, searchIndex);
      // Include folder if it has matching children OR its own label/content matches
      if (filteredChildren.length > 0 || labelMatches || idMatches || contentMatches) {
        acc.push({
          ...node,
          children: filteredChildren.length > 0 ? filteredChildren : node.children,
          expanded: true, // Auto-expand folders with matches
        });
      }
    } else if (labelMatches || idMatches || contentMatches) {
      acc.push(node);
    }
    
    return acc;
  }, []);
}

/** Collect all folder IDs from a tree (for auto-expand during search) */
function collectFolderIds(nodes: SidebarNode[]): Set<string> {
  const ids = new Set<string>();
  function walk(items: SidebarNode[]) {
    for (const item of items) {
      if (item.children && item.children.length > 0) {
        ids.add(item.id);
        walk(item.children);
      }
    }
  }
  walk(nodes);
  return ids;
}

/** Count leaf nodes (non-folder items) in the tree */
function countLeafNodes(nodes: SidebarNode[]): number {
  let count = 0;
  function walk(items: SidebarNode[]) {
    for (const item of items) {
      if (item.children && item.children.length > 0) {
        walk(item.children);
      } else {
        count++;
      }
    }
  }
  walk(nodes);
  return count;
}

/** Highlight matching text in a label */
function highlightMatch(label: string, query: string): React.ReactNode {
  if (!query.trim()) return label;
  
  const lowerLabel = label.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const index = lowerLabel.indexOf(lowerQuery);
  
  if (index === -1) return label;
  
  return (
    <>
      {label.slice(0, index)}
      <mark className="search-highlight">{label.slice(index, index + query.length)}</mark>
      {label.slice(index + query.length)}
    </>
  );
}

export function Sidebar({ items, width, onResize, onItemSelect, searchIndex }: SidebarProps) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState('');
  const [savedExpandedIds, setSavedExpandedIds] = useState<Set<string> | null>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const resizeRef = useRef<HTMLDivElement>(null);
  const isResizing = useRef(false);

  // Filter tree based on search query (with deep search via searchIndex)
  const filteredItems = useMemo(() => {
    return filterTree(items, searchQuery, searchIndex);
  }, [items, searchQuery, searchIndex]);

  // Auto-expand all folders when searching
  useEffect(() => {
    if (searchQuery.trim()) {
      // Save current expansion state before searching (only once)
      if (savedExpandedIds === null) {
        setSavedExpandedIds(new Set(expandedIds));
      }
      // Expand all folders in filtered results
      const allFolderIds = collectFolderIds(filteredItems);
      setExpandedIds(allFolderIds);
    } else if (savedExpandedIds !== null) {
      // Restore previous expansion state when search is cleared
      setExpandedIds(savedExpandedIds);
      setSavedExpandedIds(null);
    }
  }, [searchQuery, filteredItems, savedExpandedIds]);

  // Clean up stale expanded IDs when menu tree changes
  // Remove any expanded IDs that no longer exist or belong to empty folders
  useEffect(() => {
    if (searchQuery.trim()) {
      // Don't clean up during search (we want to keep expanded state for search results)
      return;
    }
    
    // Helper to check if a folder has any visible children
    const hasVisibleChildren = (node: SidebarNode): boolean => {
      if (!node.children || node.children.length === 0) {
        return false;
      }
      // Check if any child is visible (not filtered out)
      return node.children.some(child => {
        if (child.children && child.children.length > 0) {
          return hasVisibleChildren(child);
        }
        return true; // Leaf node is visible
      });
    };
    
    // Collect all valid folder IDs from current tree
    const validFolderIds = collectFolderIds(items);
    
    // Remove any expanded IDs that don't exist anymore or belong to empty folders
    setExpandedIds((prev) => {
      const cleaned = new Set<string>();
      
      // Helper to find a node by ID
      const findNode = (nodes: SidebarNode[], id: string): SidebarNode | null => {
        for (const node of nodes) {
          if (node.id === id) return node;
          if (node.children) {
            const found = findNode(node.children, id);
            if (found) return found;
          }
        }
        return null;
      };
      
      for (const id of prev) {
        if (!validFolderIds.has(id)) {
          // ID doesn't exist anymore, skip it
          continue;
        }
        
        // Check if the folder has visible children
        const node = findNode(items, id);
        if (node && hasVisibleChildren(node)) {
          cleaned.add(id);
        }
        // If folder is empty, don't add it (auto-collapse)
      }
      
      // If we removed IDs, return cleaned set; otherwise return previous to avoid unnecessary updates
      return cleaned.size !== prev.size ? cleaned : prev;
    });
  }, [items, searchQuery]);

  // Keyboard shortcut: Ctrl+K or / to focus search
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+K or Cmd+K to focus search
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
      // / to focus search (only if not already in an input)
      if (e.key === '/' && document.activeElement?.tagName !== 'INPUT' && document.activeElement?.tagName !== 'TEXTAREA') {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
      // Escape to clear search and blur
      if (e.key === 'Escape' && document.activeElement === searchInputRef.current) {
        setSearchQuery('');
        searchInputRef.current?.blur();
      }
    };
    
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  const handleSearchChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(e.target.value);
  }, []);

  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
    searchInputRef.current?.focus();
  }, []);

  const handleResizeStart = useCallback((e: MouseEvent) => {
    e.preventDefault();
    isResizing.current = true;
    const startX = e.clientX;
    const startWidth = width;

    const handleMouseMove = (moveEvent: globalThis.MouseEvent) => {
      if (!isResizing.current) return;
      const delta = moveEvent.clientX - startX;
      onResize(startWidth + delta);
    };

    const handleMouseUp = () => {
      isResizing.current = false;
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [width, onResize]);

  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const handleItemClick = useCallback((item: SidebarNode) => {
    console.log('[Sidebar] handleItemClick called', { id: item.id, label: item.label, type: item.type, hasChildren: !!(item.children && item.children.length > 0) });
    if (item.children && item.children.length > 0) {
      toggleExpand(item.id);
    } else {
      console.log('[Sidebar] Calling onItemSelect for leaf item', item);
      // Pass searchQuery as highlightTerm so the dialog can highlight matching fields
      onItemSelect(item, searchQuery.trim() || undefined);
    }
  }, [toggleExpand, onItemSelect, searchQuery]);

  const handleDoubleClick = useCallback((item: SidebarNode) => {
    if (item.children && item.children.length > 0) {
      // Expand/collapse all children
      toggleExpand(item.id);
    } else {
      onItemSelect(item, searchQuery.trim() || undefined);
    }
  }, [toggleExpand, onItemSelect, searchQuery]);

  return (
    <div className="sidebar" style={{ width }}>
      <div className="sidebar-header">
        <span className="sidebar-title">Project</span>
      </div>
      <div className="sidebar-search">
        <svg className="search-icon" viewBox="0 0 16 16" fill="currentColor">
          <path d="M11.742 10.344a6.5 6.5 0 1 0-1.397 1.398l3.85 3.85a1 1 0 0 0 1.415-1.414l-3.868-3.834zm-5.242.156a4.5 4.5 0 1 1 0-9 4.5 4.5 0 0 1 0 9z"/>
        </svg>
        <input
          ref={searchInputRef}
          type="text"
          className="search-input"
          placeholder="Search... (Ctrl+K)"
          value={searchQuery}
          onChange={handleSearchChange}
        />
        {searchQuery && (
          <button className="search-clear" onClick={handleClearSearch} title="Clear search">
            ×
          </button>
        )}
        <button 
          className="expand-collapse-btn" 
          onClick={() => {
            const allFolderIds = collectFolderIds(items);
            const isExpanded = expandedIds.size >= allFolderIds.size * 0.5;
            setExpandedIds(isExpanded ? new Set() : allFolderIds);
          }} 
          title={expandedIds.size >= collectFolderIds(items).size * 0.5 ? "Collapse All" : "Expand All"}
        >
          {expandedIds.size >= collectFolderIds(items).size * 0.5 ? '⊟' : '⊞'}
        </button>
      </div>
      <div className="sidebar-content">
        {filteredItems.length === 0 && searchQuery ? (
          <div className="search-no-results">
            No results for "{searchQuery}"
          </div>
        ) : (
          <>
            {searchQuery && (
              <div className="search-results-count">
                {countLeafNodes(filteredItems)} result{countLeafNodes(filteredItems) !== 1 ? 's' : ''}
              </div>
            )}
            <TreeView
              items={filteredItems}
              expandedIds={expandedIds}
              onItemClick={handleItemClick}
              onItemDoubleClick={handleDoubleClick}
              level={0}
              searchQuery={searchQuery}
            />
          </>
        )}
      </div>
      <div
        className="sidebar-resize"
        ref={resizeRef}
        onMouseDown={handleResizeStart}
      />
    </div>
  );
}

interface TreeViewProps {
  items: SidebarNode[];
  expandedIds: Set<string>;
  onItemClick: (item: SidebarNode) => void;
  onItemDoubleClick: (item: SidebarNode) => void;
  level: number;
  searchQuery?: string;
}

function TreeView({
  items,
  expandedIds,
  onItemClick,
  onItemDoubleClick,
  level,
  searchQuery = '',
}: TreeViewProps) {
  return (
    <ul className="tree-list" role="tree">
      {items.map((item) => {
        const hasChildren = item.children && item.children.length > 0;
        const isExpanded = expandedIds.has(item.id);
        const isDisabled = item.disabled === true;

        return (
          <li key={item.id} className="tree-item" role="treeitem">
            <div
              className={`tree-item-row ${isDisabled ? 'tree-item-disabled' : ''}`}
              style={{ paddingLeft: level * 16 + 8 }}
              draggable={!hasChildren}
              onDragStart={(e) => {
                if (!hasChildren) {
                  e.dataTransfer.effectAllowed = 'copy';
                  e.dataTransfer.setData('application/json', JSON.stringify({
                    type: 'channel',
                    id: item.id,
                    label: item.label,
                  }));
                }
              }}
              onClick={() => {
                // Always allow folder expand/collapse even if disabled
                if (hasChildren || !isDisabled) {
                  onItemClick(item);
                }
              }}
              onDoubleClick={() => {
                if (hasChildren || !isDisabled) {
                  onItemDoubleClick(item);
                }
              }}
              title={isDisabled ? item.disabledReason || 'Not available' : ((!hasChildren) ? 'Drag to dashboard to create gauge' : undefined)}
            >
              <span className="tree-item-expander">
                {hasChildren && (isExpanded ? '▼' : '▶')}
              </span>
              <NodeIcon type={item.type} />
              <span className="tree-item-label">
                {highlightMatch(item.label, searchQuery)}
              </span>
            </div>
            {hasChildren && isExpanded && (
              <TreeView
                items={item.children!}
                expandedIds={expandedIds}
                onItemClick={onItemClick}
                onItemDoubleClick={onItemDoubleClick}
                level={level + 1}
                searchQuery={searchQuery}
              />
            )}
          </li>
        );
      })}
    </ul>
  );
}

function NodeIcon({ type }: { type?: string }) {
  const icons: Record<string, IconElement> = {
    folder: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="node-icon node-icon-folder">
        <path d="M1 3h5l1 1h7v9H1V3zm1 1v8h11V5H6.5l-1-1H2z"/>
      </svg>
    ),
    table: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="node-icon node-icon-table">
        <path d="M1 2h14v12H1V2zm1 1v3h5V3H2zm6 0v3h6V3H8zM2 7v3h5V7H2zm6 0v3h6V7H8zM2 11v2h5v-2H2zm6 0v2h6v-2H8z"/>
      </svg>
    ),
    dialog: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="node-icon node-icon-dialog">
        <path d="M2 2h12v12H2V2zm1 1v10h10V3H3z"/>
        <path d="M4 5h8v1H4zm0 2h6v1H4zm0 2h7v1H4z"/>
      </svg>
    ),
    dashboard: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="node-icon node-icon-dashboard">
        <path d="M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2zm0 1a5 5 0 1 1 0 10A5 5 0 0 1 8 3z"/>
        <path d="M8 4v4l3 2" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round"/>
      </svg>
    ),
    log: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="node-icon node-icon-log">
        <path d="M2 1h12v14H2V1zm1 1v12h10V2H3z"/>
        <path d="M4 4h8M4 7h8M4 10h5" stroke="currentColor" strokeWidth="1"/>
      </svg>
    ),
    help: (
      <svg viewBox="0 0 16 16" fill="currentColor" className="node-icon node-icon-help">
        <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zm0 1a6 6 0 1 1 0 12A6 6 0 0 1 8 2z"/>
        <path d="M8 11.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5z"/>
        <path d="M8 4.5c-1.1 0-2 .7-2 1.5h1c0-.3.4-.5 1-.5s1 .2 1 .5c0 .4-.5.7-1 1-.5.3-1 .8-1 1.5v.5h1v-.5c0-.3.3-.5.7-.7.7-.4 1.3-.9 1.3-1.8 0-.8-.9-1.5-2-1.5z"/>
      </svg>
    ),
  };

  return icons[type || 'folder'] || icons.folder;
}
