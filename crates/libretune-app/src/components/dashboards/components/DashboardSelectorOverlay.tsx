
import { FolderOpen } from 'lucide-react';
import { DashFileInfo } from '../dashTypes';

interface Props {
  availableDashes: DashFileInfo[];
  selectedPath: string;
  onSelect: (path: string) => void;
  onClose: () => void;
  onImportClick: () => void;
}

/**
 * Modal dashboard selector grouped by category.
 * Extracted from TsDashboard during Phase C4.
 */
export default function DashboardSelectorOverlay({
  availableDashes,
  selectedPath,
  onSelect,
  onClose,
  onImportClick,
}: Props) {
  // Group dashboards by category
  const categories = new Map<string, DashFileInfo[]>();
  availableDashes.forEach((dash) => {
    const cat = dash.category || 'Other';
    if (!categories.has(cat)) {
      categories.set(cat, []);
    }
    categories.get(cat)!.push(dash);
  });

  // Sort categories: User first, then Reference, then others
  const sortedCats = Array.from(categories.keys()).sort((a, b) => {
    if (a === 'User') return -1;
    if (b === 'User') return 1;
    if (a === 'Reference') return -1;
    if (b === 'Reference') return 1;
    return a.localeCompare(b);
  });

  return (
    <div className="ts-dashboard-selector-overlay" onClick={onClose}>
      <div className="ts-dashboard-selector" onClick={(e) => e.stopPropagation()}>
        <h3>Select Dashboard</h3>
        <div className="ts-dashboard-list">
          {sortedCats.map((category) => (
            <div key={category} className="ts-dashboard-category">
              <div className="ts-dashboard-category-header">
                {category}
                <span className="ts-dashboard-category-count">
                  ({categories.get(category)!.length})
                </span>
              </div>
              <div className="ts-dashboard-category-items">
                {categories.get(category)!.map((dash) => (
                  <button
                    key={dash.path}
                    className={`ts-dashboard-option ${dash.path === selectedPath ? 'selected' : ''}`}
                    onClick={() => onSelect(dash.path)}
                    title={dash.path}
                  >
                    {dash.name.replace(/\.(ltdash\.xml|dash|gauge)$/i, '')}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>

        {/* Import button */}
        <div className="ts-dashboard-import-section">
          <button className="ts-dashboard-import-btn" onClick={onImportClick}>
            <FolderOpen size={14} /> Import TS Dashboard Files...
          </button>
        </div>
      </div>
    </div>
  );
}
