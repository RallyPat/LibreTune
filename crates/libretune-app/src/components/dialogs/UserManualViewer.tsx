/**
 * UserManualViewer - Displays the bundled mdBook user manual.
 * 
 * Loads HTML content from the bundled documentation and displays it
 * in an iframe or inline viewer. Supports navigation between sections
 * and falls back to online docs if bundled content is unavailable.
 * 
 * @example
 * ```tsx
 * <UserManualViewer
 *   section="getting-started/connecting"
 *   onClose={() => setShowManual(false)}
 * />
 * ```
 */

import { useState } from 'react';
import { X, ExternalLink, ChevronLeft, ChevronRight, Home, Book } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';
import './UserManualViewer.css';

/** Props for UserManualViewer component */
interface UserManualViewerProps {
  /** Section path to display (e.g., 'getting-started/connecting') */
  section?: string;
  /** Callback when viewer is closed */
  onClose: () => void;
}

/** Table of contents entry */
interface TocEntry {
  title: string;
  path: string;
  children?: TocEntry[];
}

/** Fallback online docs URL */
const ONLINE_DOCS_URL = 'https://github.com/RallyPat/LibreTune/tree/main/docs';

/** Table of contents for navigation */
const TABLE_OF_CONTENTS: TocEntry[] = [
  { title: 'Introduction', path: 'introduction' },
  {
    title: 'Getting Started',
    path: 'getting-started',
    children: [
      { title: 'Installation', path: 'getting-started/installation' },
      { title: 'Creating Your First Project', path: 'getting-started/first-project' },
      { title: 'Connecting to Your ECU', path: 'getting-started/connecting' },
    ],
  },
  {
    title: 'Core Features',
    path: 'features',
    children: [
      { title: 'Table Editing', path: 'features/table-editing' },
      { title: 'AutoTune Live', path: 'features/autotune' },
      { title: 'Dashboards', path: 'features/dashboards' },
      { title: 'Data Logging', path: 'features/datalog' },
    ],
  },
  {
    title: 'Project Management',
    path: 'projects',
    children: [
      { title: 'Managing Tunes', path: 'projects/tunes' },
      { title: 'Version Control', path: 'projects/version-control' },
      { title: 'Restore Points', path: 'projects/restore-points' },
      { title: 'Importing Projects', path: 'projects/importing' },
    ],
  },
  {
    title: 'Reference',
    path: 'reference',
    children: [
      { title: 'Supported ECUs', path: 'reference/supported-ecus' },
      { title: 'INI File Format', path: 'reference/ini-format' },
      { title: 'Keyboard Shortcuts', path: 'reference/shortcuts' },
      { title: 'Troubleshooting', path: 'reference/troubleshooting' },
    ],
  },
  { title: 'FAQ', path: 'faq' },
  { title: 'Contributing', path: 'contributing' },
];

export default function UserManualViewer({ section = 'introduction', onClose }: UserManualViewerProps) {
  const [currentSection, setCurrentSection] = useState(section);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [loading] = useState(false);

  // Get the current section title
  const getCurrentTitle = (): string => {
    const findTitle = (entries: TocEntry[], path: string): string | null => {
      for (const entry of entries) {
        if (entry.path === path) return entry.title;
        if (entry.children) {
          const found = findTitle(entry.children, path);
          if (found) return found;
        }
      }
      return null;
    };
    return findTitle(TABLE_OF_CONTENTS, currentSection) || 'User Manual';
  };

  // Get flat list for prev/next navigation
  const getFlatList = (): TocEntry[] => {
    const flat: TocEntry[] = [];
    const flatten = (entries: TocEntry[]) => {
      for (const entry of entries) {
        flat.push(entry);
        if (entry.children) flatten(entry.children);
      }
    };
    flatten(TABLE_OF_CONTENTS);
    return flat;
  };

  const flatList = getFlatList();
  const currentIndex = flatList.findIndex(e => e.path === currentSection);
  const prevSection = currentIndex > 0 ? flatList[currentIndex - 1] : null;
  const nextSection = currentIndex < flatList.length - 1 ? flatList[currentIndex + 1] : null;

  const handleOpenOnline = async () => {
    try {
      await openUrl(`${ONLINE_DOCS_URL}/src/${currentSection}.md`);
    } catch (err) {
      console.error('Failed to open URL:', err);
    }
  };

  const renderTocEntry = (entry: TocEntry, depth = 0) => {
    const isActive = currentSection === entry.path || currentSection.startsWith(entry.path + '/');
    
    return (
      <div key={entry.path}>
        <button
          className={`toc-entry ${isActive ? 'active' : ''}`}
          style={{ paddingLeft: `${12 + depth * 16}px` }}
          onClick={() => setCurrentSection(entry.path)}
        >
          {entry.title}
        </button>
        {entry.children && isActive && (
          <div className="toc-children">
            {entry.children.map(child => renderTocEntry(child, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  // Map section to content (simplified - in production, load from bundled HTML)
  const getContent = (): string => {
    // This would load from bundled mdBook output in production
    // For now, return a placeholder pointing to docs
    return `
      <div class="manual-placeholder">
        <h1>${getCurrentTitle()}</h1>
        <p>The full user manual is available in the <code>docs/</code> folder.</p>
        <p>To build and view the manual:</p>
        <pre><code>cd docs
mdbook serve --open</code></pre>
        <p>Or click "Open Online" to view on GitHub.</p>
      </div>
    `;
  };

  return (
    <div className="manual-viewer-overlay" onClick={onClose}>
      <div className="manual-viewer-modal" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="manual-viewer-header">
          <div className="manual-header-left">
            <Book size={20} />
            <span>LibreTune User Manual</span>
          </div>
          <div className="manual-header-right">
            <button className="manual-icon-btn" onClick={handleOpenOnline} title="Open Online">
              <ExternalLink size={18} />
            </button>
            <button className="manual-icon-btn" onClick={onClose} title="Close">
              <X size={20} />
            </button>
          </div>
        </div>

        <div className="manual-viewer-body">
          {/* Sidebar */}
          {sidebarOpen && (
            <div className="manual-sidebar">
              <div className="manual-sidebar-header">
                <button
                  className="toc-home-btn"
                  onClick={() => setCurrentSection('introduction')}
                >
                  <Home size={16} />
                  Home
                </button>
              </div>
              <div className="manual-toc">
                {TABLE_OF_CONTENTS.map(entry => renderTocEntry(entry))}
              </div>
            </div>
          )}

          {/* Content */}
          <div className="manual-content">
            <button
              className="sidebar-toggle"
              onClick={() => setSidebarOpen(!sidebarOpen)}
              title={sidebarOpen ? 'Hide sidebar' : 'Show sidebar'}
            >
              {sidebarOpen ? <ChevronLeft size={20} /> : <ChevronRight size={20} />}
            </button>

            <div className="manual-content-inner">
              {loading ? (
                <div className="manual-loading">Loading...</div>
              ) : (
                <div
                  className="manual-text"
                  dangerouslySetInnerHTML={{ __html: getContent() }}
                />
              )}
            </div>

            {/* Navigation footer */}
            <div className="manual-nav-footer">
              {prevSection ? (
                <button
                  className="manual-nav-btn prev"
                  onClick={() => setCurrentSection(prevSection.path)}
                >
                  <ChevronLeft size={16} />
                  {prevSection.title}
                </button>
              ) : (
                <div />
              )}
              {nextSection && (
                <button
                  className="manual-nav-btn next"
                  onClick={() => setCurrentSection(nextSection.path)}
                >
                  {nextSection.title}
                  <ChevronRight size={16} />
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
