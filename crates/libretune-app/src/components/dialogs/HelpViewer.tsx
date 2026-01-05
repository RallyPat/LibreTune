import { X, ExternalLink } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';
import './HelpViewer.css';

export interface HelpTopicData {
  name: string;
  title: string;
  web_url?: string;
  text_lines: string[];
}

interface HelpViewerProps {
  topic: HelpTopicData;
  onClose: () => void;
}

export default function HelpViewer({ topic, onClose }: HelpViewerProps) {
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

        {topic.web_url && (
          <div className="help-viewer-footer">
            <button className="help-web-btn" onClick={handleWebHelp}>
              <ExternalLink size={16} />
              Open Web Help
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
