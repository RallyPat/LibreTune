import { useState, useEffect } from 'react';
import { StatusItem } from './TunerLayout';
import './StatusBar.css';

interface StatusBarProps {
  items: StatusItem[];
  connected: boolean;
  ecuName?: string;
  unitsSystem?: 'metric' | 'imperial';
}

export function StatusBar({ items, connected, ecuName }: StatusBarProps) {
  const [currentPage, setCurrentPage] = useState(0);
  const itemsPerPage = 6;

  // Get center items (channel indicators)
  const centerItems = items.filter((item) => item.align === 'center' || !item.align);
  const rightItems = items.filter((item) => item.align === 'right');

  // Calculate pagination for center items
  const totalPages = Math.ceil(centerItems.length / itemsPerPage);
  const safeCurrentPage = totalPages > 0 ? Math.min(currentPage, totalPages - 1) : 0;
  const visibleItems = centerItems.slice(
    safeCurrentPage * itemsPerPage,
    (safeCurrentPage + 1) * itemsPerPage
  );

  // Keyboard shortcuts for pagination
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!e.altKey) return;
      
      if (e.key === '>' || e.key === '.') {
        // Alt+> to next page
        e.preventDefault();
        setCurrentPage((p) => (p + 1) % (totalPages || 1));
      } else if (e.key === '<' || e.key === ',') {
        // Alt+< to previous page
        e.preventDefault();
        setCurrentPage((p) => (p - 1 + (totalPages || 1)) % (totalPages || 1));
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [totalPages]);

  return (
    <div className="statusbar">
      {/* Connection status (always on left) */}
      <div className="statusbar-section statusbar-section-left">
        <div className={`statusbar-connection ${connected ? 'connected' : 'disconnected'}`}>
          <ConnectionIcon connected={connected} />
          <span>{connected ? (ecuName || 'Connected') : 'Disconnected'}</span>
        </div>
      </div>

      {/* Center items with pagination */}
      <div className="statusbar-section statusbar-section-center">
        {centerItems.length > 0 ? (
          <>
            {/* Pagination previous button */}
            {totalPages > 1 && (
              <button
                className="pagination-btn pagination-prev"
                onClick={() => setCurrentPage((p) => (p - 1 + totalPages) % totalPages)}
                title="Previous page (Alt+<)"
                aria-label="Previous status page"
              >
                ‹
              </button>
            )}

            {/* Visible items */}
            {visibleItems.map((item) => (
              <StatusBarItem key={item.id} item={item} />
            ))}

            {/* Pagination next button */}
            {totalPages > 1 && (
              <button
                className="pagination-btn pagination-next"
                onClick={() => setCurrentPage((p) => (p + 1) % totalPages)}
                title="Next page (Alt+>)"
                aria-label="Next status page"
              >
                ›
              </button>
            )}

            {/* Page indicator */}
            {totalPages > 1 && (
              <span className="pagination-indicator" title="Current page">
                {safeCurrentPage + 1}/{totalPages}
              </span>
            )}
          </>
        ) : null}
      </div>

      {/* Right items */}
      <div className="statusbar-section statusbar-section-right">
        {rightItems.map((item) => (
          <StatusBarItem key={item.id} item={item} />
        ))}
      </div>
    </div>
  );
}

function StatusBarItem({ item }: { item: StatusItem }) {
  const style = item.width ? { width: item.width } : {};
  
  if (item.onClick) {
    return (
      <button
        className="statusbar-item statusbar-item-clickable"
        style={style}
        onClick={item.onClick}
      >
        {item.content}
      </button>
    );
  }

  return (
    <div className="statusbar-item" style={style}>
      {item.content}
    </div>
  );
}

function ConnectionIcon({ connected }: { connected: boolean }) {
  if (connected) {
    return (
      <svg viewBox="0 0 16 16" fill="currentColor" className="statusbar-icon">
        <circle cx="8" cy="8" r="4" fill="var(--success)" />
        <circle cx="8" cy="8" r="6" fill="none" stroke="var(--success)" strokeWidth="1" opacity="0.5" />
      </svg>
    );
  }

  return (
    <svg viewBox="0 0 16 16" fill="currentColor" className="statusbar-icon">
      <circle cx="8" cy="8" r="4" fill="var(--text-muted)" />
      <path d="M4 4l8 8M12 4l-8 8" stroke="var(--error)" strokeWidth="1.5" />
    </svg>
  );
}

// Reusable status channel indicators for realtime data (TS-style grid cells)
export function StatusIndicator({
  label,
  value,
  unit,
  color,
}: {
  label: string;
  value: string | number;
  unit?: string;
  color?: 'success' | 'warning' | 'error' | 'info';
}) {
  return (
    <div className={`statusbar-cell ${color ? `statusbar-cell-${color}` : ''}`}>
      <span className="statusbar-cell-label">{label}</span>
      <span className="statusbar-cell-value">
        {value}
        {unit && <span className="statusbar-cell-unit">{unit}</span>}
      </span>
    </div>
  );
}

export function LoggingIndicator({
  isLogging,
  duration,
}: {
  isLogging: boolean;
  duration?: string;
}) {
  if (!isLogging) {
    return (
      <span className="logging-indicator logging-inactive">
        <svg viewBox="0 0 16 16" className="statusbar-icon">
          <circle cx="8" cy="8" r="5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
        <span>Not Logging</span>
      </span>
    );
  }

  return (
    <span className="logging-indicator logging-active">
      <svg viewBox="0 0 16 16" className="statusbar-icon">
        <circle cx="8" cy="8" r="5" fill="var(--error)">
          <animate attributeName="opacity" values="1;0.5;1" dur="1s" repeatCount="indefinite" />
        </circle>
      </svg>
      <span>Logging</span>
      {duration && <span className="logging-duration">{duration}</span>}
    </span>
  );
}
