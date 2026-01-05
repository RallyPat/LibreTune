import { ToolbarItem } from './TunerLayout';
import './Toolbar.css';

type IconElement = React.ReactElement;

interface ToolbarProps {
  items: ToolbarItem[];
}

export function Toolbar({ items }: ToolbarProps) {
  return (
    <div className="toolbar" role="toolbar">
      {items.map((item, index) => {
        if (item.separator) {
          return <div key={`sep-${index}`} className="toolbar-separator" />;
        }

        return (
          <button
            key={item.id}
            className={`toolbar-button ${item.active ? 'toolbar-button-active' : ''}`}
            onClick={item.onClick}
            disabled={item.disabled}
            title={item.tooltip}
            aria-label={item.tooltip}
          >
            <ToolbarIcon icon={item.icon} />
          </button>
        );
      })}
    </div>
  );
}

// SVG icons for common toolbar actions
function ToolbarIcon({ icon }: { icon: string }) {
  const icons: Record<string, IconElement> = {
    // File operations
    'new': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M9 1H4a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V5L9 1zm0 1.5L12.5 6H9V2.5zM4 14V2h4v5h5v7H4z"/>
      </svg>
    ),
    'open': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M6.5 2H2a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V5a1 1 0 0 0-1-1H8l-1.5-2zm0 1.5L8 5h6v8H2V3h4.5z"/>
      </svg>
    ),
    'save': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M12 1H2a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V4l-3-3zm-1 1v3H5V2h6zm2 12H3V2h1v4h8V2h.5L14 4v10z"/>
      </svg>
    ),
    'burn': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M8 1c-1 2-2 3-2 5a2 2 0 1 0 4 0c0-2-1-3-2-5zm0 9a3 3 0 0 1-3-3c0-1.5.5-2.5 1-3.5-.5 1.5-1 2.5-1 3.5a3 3 0 0 0 6 0c0-1-.5-2-1-3.5.5 1 1 2 1 3.5a3 3 0 0 1-3 3z"/>
        <path d="M4 12h8v2H4z"/>
      </svg>
    ),
    // Connection
    'connect': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M12 3a1 1 0 1 1 0 2 1 1 0 0 1 0-2zm-3 3a1 1 0 1 1 0 2 1 1 0 0 1 0-2zM6 9a1 1 0 1 1 0 2 1 1 0 0 1 0-2zM3 12a1 1 0 1 1 0 2 1 1 0 0 1 0-2z"/>
        <path d="M11.5 4.5l-2 2m-1 1l-2 2m-1 1l-2 2" stroke="currentColor" strokeWidth="1" fill="none"/>
      </svg>
    ),
    'disconnect': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M12 3a1 1 0 1 1 0 2 1 1 0 0 1 0-2zM3 12a1 1 0 1 1 0 2 1 1 0 0 1 0-2z"/>
        <path d="M11 5l-6 6" stroke="currentColor" strokeWidth="2" fill="none"/>
        <path d="M2 2l12 12" stroke="var(--error)" strokeWidth="1.5" fill="none"/>
      </svg>
    ),
    // Realtime
    'realtime': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zm0 1a6 6 0 1 1 0 12A6 6 0 0 1 8 2z"/>
        <path d="M8 3v5l3 2" stroke="currentColor" strokeWidth="1.5" fill="none"/>
      </svg>
    ),
    // Logging
    'log-start': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <circle cx="8" cy="8" r="6" fill="var(--error)"/>
      </svg>
    ),
    'log-stop': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <rect x="4" y="4" width="8" height="8" fill="currentColor"/>
      </svg>
    ),
    // Navigation
    'settings': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M8 4a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm0 1a3 3 0 1 1 0 6 3 3 0 0 1 0-6z"/>
        <path d="M7 1h2v2H7zm0 12h2v2H7zM1 7h2v2H1zm12 0h2v2h-2zM2.5 2.5l1.4 1.4-1.4 1.4-1.4-1.4zm9.2 9.2l1.4 1.4-1.4 1.4-1.4-1.4zm0-9.2l1.4 1.4-1.4 1.4-1.4-1.4zM2.5 11.7l1.4 1.4-1.4 1.4-1.4-1.4z"/>
      </svg>
    ),
    // Table operations
    'undo': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M4 7l-3 3 3 3v-2h5a4 4 0 0 0 0-8H5v2h4a2 2 0 0 1 0 4H4V7z"/>
      </svg>
    ),
    'redo': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M12 7l3 3-3 3v-2H7a4 4 0 0 1 0-8h4v2H7a2 2 0 0 0 0 4h5V7z"/>
      </svg>
    ),
    'copy': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M5 3V1h9v10h-2v2H3V5h2V3zm1 0h6v7h1V2H6v1zm-2 3v8h7V6H4z"/>
      </svg>
    ),
    'paste': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <path d="M10 1H6v1H4v12h8V2h-2V1zm-1 1v1H7V2h2zm2 1v10H5V3h1v1h4V3h1z"/>
      </svg>
    ),
    // Default fallback
    'default': (
      <svg viewBox="0 0 16 16" fill="currentColor">
        <rect x="2" y="2" width="12" height="12" rx="2" fill="none" stroke="currentColor"/>
      </svg>
    ),
  };

  return icons[icon] || icons['default'];
}
