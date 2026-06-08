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
const ONLINE_DOCS_URL = 'https://github.com/RallyPat/FCoreTuner/tree/main/docs';

/** Table of contents for navigation */
const TABLE_OF_CONTENTS: TocEntry[] = [
  { title: '简介', path: 'introduction' },
  {
    title: '快速入门',
    path: 'getting-started',
    children: [
      { title: '安装', path: 'getting-started/installation' },
      { title: '创建第一个项目', path: 'getting-started/first-project' },
      { title: '连接您的 ECU', path: 'getting-started/connecting' },
    ],
  },
  {
    title: '核心功能',
    path: 'features',
    children: [
      { title: '表格编辑', path: 'features/table-editing' },
      { title: '实时自动调校', path: 'features/autotune' },
      { title: '仪表盘', path: 'features/dashboards' },
      { title: '数据记录', path: 'features/datalog' },
    ],
  },
  {
    title: '项目管理',
    path: 'projects',
    children: [
      { title: '管理调教', path: 'projects/tunes' },
      { title: '版本控制', path: 'projects/version-control' },
      { title: '还原点', path: 'projects/restore-points' },
      { title: '导入项目', path: 'projects/importing' },
    ],
  },
  {
    title: '参考',
    path: 'reference',
    children: [
      { title: '支持的 ECU', path: 'reference/supported-ecus' },
      { title: 'INI 文件格式', path: 'reference/ini-format' },
      { title: '键盘快捷键', path: 'reference/shortcuts' },
      { title: '故障排除', path: 'reference/troubleshooting' },
    ],
  },
  { title: '常见问题', path: 'faq' },
  { title: '贡献', path: 'contributing' },
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
    return findTitle(TABLE_OF_CONTENTS, currentSection) || '用户手册';
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
        <p>完整的用户手册可在 <code>docs/</code> 文件夹中找到。</p>
        <p>构建并查看手册:</p>
        <pre><code>cd docs
mdbook serve --open</code></pre>
        <p>或点击"打开在线"在 GitHub 上查看。</p>
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
            <span>FCoreTuner 用户手册</span>
          </div>
          <div className="manual-header-right">
            <button className="manual-icon-btn" onClick={handleOpenOnline} title="打开在线">
              <ExternalLink size={18} />
            </button>
            <button className="manual-icon-btn" onClick={onClose} title="关闭">
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
                  首页
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
              title={sidebarOpen ? '隐藏侧栏' : '显示侧栏'}
            >
              {sidebarOpen ? <ChevronLeft size={20} /> : <ChevronRight size={20} />}
            </button>

            <div className="manual-content-inner">
              {loading ? (
                <div className="manual-loading">加载中...</div>
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
