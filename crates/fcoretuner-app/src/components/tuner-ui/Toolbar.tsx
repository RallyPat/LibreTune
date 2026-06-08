import {
  FilePlus,
  FolderOpen,
  Save,
  Zap,
  Plug,
  Unplug,
  Gauge,
  Circle,
  Square,
  Settings,
  Undo2,
  Redo2,
  Copy,
  ClipboardPaste,
  HelpCircle,
  LucideIcon,
} from 'lucide-react';
import { ToolbarItem } from './TunerLayout';
import './Toolbar.css';

interface ToolbarProps {
  items: ToolbarItem[];
}

// Map icon string names to lucide-react components
const iconMap: Record<string, LucideIcon> = {
  'new': FilePlus,
  'open': FolderOpen,
  'save': Save,
  'burn': Zap,
  'connect': Plug,
  'disconnect': Unplug,
  'realtime': Gauge,
  'log-start': Circle,
  'log-stop': Square,
  'settings': Settings,
  'undo': Undo2,
  'redo': Redo2,
  'copy': Copy,
  'paste': ClipboardPaste,
  'default': HelpCircle,
};

export function Toolbar({ items }: ToolbarProps) {
  return (
    <div className="toolbar" role="toolbar">
      {items.map((item, index) => {
        if (item.separator) {
          return <div key={`sep-${index}`} className="toolbar-separator" />;
        }

        const IconComponent = iconMap[item.icon] || iconMap['default'];
        const isRecording = item.icon === 'log-start' && item.active;

        return (
          <button
            key={item.id}
            className={`toolbar-button ${item.active ? 'toolbar-button-active' : ''}`}
            onClick={item.onClick}
            disabled={item.disabled}
            title={item.tooltip}
            aria-label={item.tooltip}
          >
            <IconComponent
              size={18}
              strokeWidth={1.75}
              className={isRecording ? 'icon-recording' : undefined}
            />
          </button>
        );
      })}
    </div>
  );
}
