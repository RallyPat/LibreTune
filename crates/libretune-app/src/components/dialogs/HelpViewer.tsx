/**
 * HelpViewer - Displays context-sensitive help from INI files.
 *
 * Shows help content defined in the ECU's INI file with options
 * to open web help or the full user manual.
 *
 * @example
 * ```tsx
 * <HelpViewer
 *   topic={{ name: 'fuel_table', title: 'Fuel Table', text_lines: ['...'] }}
 *   onClose={() => setHelpOpen(false)}
 *   onOpenManual={() => setManualOpen(true)}
 * />
 * ```
 */

import { X, ExternalLink, Book } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';
import './HelpViewer.css';

/** Help topic data from INI files */
export interface HelpTopicData {
  /** Internal name/identifier */
  name: string;
  /** Display title */
  title: string;
  /** Optional URL for external web help */
  web_url?: string;
  /** HTML content lines */
  text_lines: string[];
}

/** Props for HelpViewer component */
interface HelpViewerProps {
  /** Help topic to display */
  topic: HelpTopicData;
  /** Callback when viewer is closed */
  onClose: () => void;
  /** Callback to open the full user manual */
  onOpenManual?: () => void;
}

export default function HelpViewer({ topic, onClose, onOpenManual }: HelpViewerProps) {
  const handleWebHelp = async () => {
    if (topic.web_url) {
      try {
        await openUrl(topic.web_url);
      } catch (err) {
        console.error('Failed to open URL:', err);
      }
    }
  };

  // Join text lines and render as HTML (content is from trusted INI files)
  const htmlContent = topic.text_lines.join('\n');

  return (
    <div className="help-viewer-overlay" onClick={onClose}>
      <div className="help-viewer-modal" onClick={(e) => e.stopPropagation()}>
        <div className="help-viewer-header">
          <h2>{topic.title}</h2>
          <button className="help-close-btn" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        <div className="help-viewer-content">
          {topic.text_lines.length > 0 ? (
            <div
              className="help-text"
              dangerouslySetInnerHTML={{ __html: htmlContent }}
            />
          ) : (
            <p className="help-no-content">No help content available.</p>
          )}
        </div>

        <div className="help-viewer-footer">
          {onOpenManual && (
            <button className="help-manual-btn" onClick={onOpenManual}>
              <Book size={16} />
              User Manual
            </button>
          )}
          {topic.web_url && (
            <button className="help-web-btn" onClick={handleWebHelp}>
              <ExternalLink size={16} />
              Open Web Help
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
