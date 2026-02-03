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

import { MouseEvent, useEffect, useState } from 'react';
import { X, ExternalLink, ChevronLeft, ChevronRight, Home, Book } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { marked } from 'marked';
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
const ONLINE_DOCS_URL = 'https://github.com/RallyPat/LibreTune/tree/main/docs/src';

/** Table of contents for navigation */
const FALLBACK_TOC: TocEntry[] = [
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
      { title: 'AutoTune', path: 'features/autotune' },
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
  const [loading, setLoading] = useState(false);
  const [tocEntries, setTocEntries] = useState<TocEntry[]>(FALLBACK_TOC);
  const [contentHtml, setContentHtml] = useState('');
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    setCurrentSection(section);
  }, [section]);

  useEffect(() => {
    let cancelled = false;
    fetch('/manual/toc.json')
      .then((res) => (res.ok ? res.json() : Promise.reject(new Error('toc load failed'))))
      .then((data) => {
        if (!cancelled && Array.isArray(data)) {
          setTocEntries(data);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setTocEntries(FALLBACK_TOC);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

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
    return findTitle(tocEntries, currentSection) || 'User Manual';
  };

  // Get flat list for prev/next navigation
  const getFlatList = (): TocEntry[] => {
    const flat: TocEntry[] = [];
    const flatten = (entries: TocEntry[]) => {
      for (const entry of entries) {
        if (entry.path) {
          flat.push(entry);
        }
        if (entry.children) flatten(entry.children);
      }
    };
    flatten(tocEntries);
    return flat;
  };

  const flatList = getFlatList();
  const currentIndex = flatList.findIndex(e => e.path === currentSection);
  const prevSection = currentIndex > 0 ? flatList[currentIndex - 1] : null;
  const nextSection = currentIndex < flatList.length - 1 ? flatList[currentIndex + 1] : null;

  const handleOpenOnline = async () => {
    try {
      await openUrl(`${ONLINE_DOCS_URL}/${currentSection}.md`);
    } catch (err) {
      console.error('Failed to open URL:', err);
    }
  };

  const renderTocEntry = (entry: TocEntry, depth = 0, parentActive = false) => {
    if (!entry.path) {
      return null;
    }

    const isActive = currentSection === entry.path || currentSection.startsWith(entry.path + '/');
    const shouldShowChildren = entry.children && entry.children.length > 0;
    
    return (
      <div key={entry.path}>
        <button
          className={`toc-entry ${isActive ? 'active' : ''}`}
          style={{ paddingLeft: `${12 + depth * 16}px` }}
          onClick={() => setCurrentSection(entry.path)}
        >
          {entry.title}
        </button>
        {shouldShowChildren && (
          <div className="toc-children">
            {entry.children!.map(child => renderTocEntry(child, depth + 1, isActive || parentActive))}
          </div>
        )}
      </div>
    );
  };

  const normalizeSectionPath = (path: string) => path.replace(/^\//, '').replace(/\.md$/i, '');

  const resolveRelativePath = (baseSection: string, href: string) => {
    const baseParts = baseSection.split('/').slice(0, -1);
    const relParts = href.split('/');
    for (const part of relParts) {
      if (part === '.' || part === '') continue;
      if (part === '..') {
        baseParts.pop();
      } else {
        baseParts.push(part);
      }
    }
    return baseParts.join('/');
  };

  const normalizeImageSrc = (href: string, baseSection: string) => {
    if (!href || href.startsWith('http') || href.startsWith('data:')) return href;
    if (href.startsWith('/manual/')) return href;

    const cleaned = href.replace(/^\.\//, '');
    const screenshotPrefix = '../screenshots/';
    if (cleaned.startsWith(screenshotPrefix)) {
      return `/manual/screenshots/${cleaned.slice(screenshotPrefix.length)}`;
    }
    if (cleaned.startsWith('screenshots/')) {
      return `/manual/${cleaned}`;
    }

    const resolved = resolveRelativePath(baseSection, cleaned);
    if (resolved.startsWith('screenshots/')) {
      return `/manual/${resolved}`;
    }
    return `/manual/${resolved}`;
  };

  const createRenderer = (baseSection: string) => {
    const renderer = new marked.Renderer();

    renderer.image = (href, title, text) => {
      const src = normalizeImageSrc(href ?? '', baseSection);
      const safeTitle = title ? ` title="${title}"` : '';
      const alt = text ?? '';
      return `<img src="${src}" alt="${alt}"${safeTitle} />`;
    };

    renderer.link = (href, title, text) => {
      if (!href) return text ?? '';
      if (href.startsWith('http') || href.startsWith('mailto:')) {
        const safeTitle = title ? ` title="${title}"` : '';
        return `<a href="${href}" target="_blank" rel="noreferrer"${safeTitle}>${text ?? href}</a>`;
      }

      const resolved = resolveRelativePath(baseSection, href.replace(/^\.\//, ''));
      const sectionPath = normalizeSectionPath(resolved);
      const safeTitle = title ? ` title="${title}"` : '';
      return `<a href="#" data-manual-link="${sectionPath}"${safeTitle}>${text ?? href}</a>`;
    };

    return renderer;
  };

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError(null);

    const sectionPath = normalizeSectionPath(currentSection);

    fetch(`/manual/${sectionPath}.md`)
      .then((res) => (res.ok ? res.text() : Promise.reject(new Error('manual load failed'))))
      .then((markdown) => {
        if (cancelled) return;
        const renderer = createRenderer(sectionPath);
        const html = marked.parse(markdown, { renderer }) as string;
        setContentHtml(html);
        setLoading(false);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error('Failed to load manual section:', err);
        setLoadError('Unable to load this manual section.');
        setContentHtml('');
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [currentSection]);

  const handleContentClick = (event: MouseEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement | null;
    if (!target) return;
    const link = target.closest('a[data-manual-link]') as HTMLAnchorElement | null;
    if (!link) return;
    event.preventDefault();
    const next = link.getAttribute('data-manual-link');
    if (next) {
      setCurrentSection(next);
    }
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
                {tocEntries.map(entry => renderTocEntry(entry))}
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

            <div className="manual-content-inner" onClick={handleContentClick}>
              {loading && <div className="manual-loading">Loading...</div>}
              {!loading && loadError && (
                <div className="manual-placeholder">
                  <h1>{getCurrentTitle()}</h1>
                  <p>{loadError}</p>
                  <button className="toc-home-btn" onClick={handleOpenOnline}>Open Online</button>
                </div>
              )}
              {!loading && !loadError && (
                <div className="manual-text" dangerouslySetInnerHTML={{ __html: contentHtml }} />
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
