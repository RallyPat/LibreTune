import { useState } from 'react';
import './StatusBarChannelSelector.css';

interface ChannelInfo {
  name?: string;
  label?: string;
  units?: string;
}

export interface StatusBarChannelSelectorProps {
  selectedChannels: string[];
  availableChannels: (string | ChannelInfo)[];
  onChannelsChange: (channels: string[]) => void;
  maxChannels?: number;
}

/**
 * StatusBarChannelSelector Component
 * 
 * Allows users to select which ECU channels to display in status bar.
 * Features:
 * - Add/remove channels
 * - Reorder via drag-drop
 * - Preview how many fit per row
 * - Keyboard navigation
 * - Remember selection
 */
export default function StatusBarChannelSelector({
  selectedChannels,
  availableChannels,
  onChannelsChange,
  maxChannels = 8,
}: StatusBarChannelSelectorProps) {
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);
  const [filterText, setFilterText] = useState('');

  // Normalize channels to ChannelInfo format - ensure name is always a string
  const normalizedChannels = availableChannels.map(ch => 
    typeof ch === 'string' ? { name: ch, label: ch } : { name: ch?.name ?? '', label: ch?.label, units: ch?.units }
  ).filter(ch => ch.name); // Filter out any with empty names

  const selectedSet = new Set(selectedChannels);
  const unselectedChannels = normalizedChannels.filter(ch => !selectedSet.has(ch.name || ''));
  const filteredUnselected = unselectedChannels.filter(ch =>
    (ch.name ?? '').toLowerCase().includes(filterText.toLowerCase()) ||
    (ch.label && ch.label.toLowerCase().includes(filterText.toLowerCase()))
  );

  const handleAddChannel = (channelName: string) => {
    if (selectedChannels.length < maxChannels) {
      onChannelsChange([...selectedChannels, channelName]);
      setFilterText('');
    }
  };

  const handleRemoveChannel = (index: number) => {
    const newChannels = selectedChannels.filter((_, i) => i !== index);
    onChannelsChange(newChannels);
  };

  const handleDragStart = (index: number) => {
    setDraggedIndex(index);
  };

  const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.currentTarget.classList.add('drag-over');
  };

  const handleDragLeave = (e: React.DragEvent<HTMLDivElement>) => {
    e.currentTarget.classList.remove('drag-over');
  };

  const handleDrop = (index: number, e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.currentTarget.classList.remove('drag-over');
    
    if (draggedIndex === null || draggedIndex === index) return;

    const newChannels = [...selectedChannels];
    const [draggedItem] = newChannels.splice(draggedIndex, 1);
    newChannels.splice(index, 0, draggedItem);
    onChannelsChange(newChannels);
    setDraggedIndex(null);
  };

  const handleMoveUp = (index: number) => {
    if (index > 0) {
      const newChannels = [...selectedChannels];
      [newChannels[index - 1], newChannels[index]] = [newChannels[index], newChannels[index - 1]];
      onChannelsChange(newChannels);
    }
  };

  const handleMoveDown = (index: number) => {
    if (index < selectedChannels.length - 1) {
      const newChannels = [...selectedChannels];
      [newChannels[index], newChannels[index + 1]] = [newChannels[index + 1], newChannels[index]];
      onChannelsChange(newChannels);
    }
  };

  const handleResetToDefaults = () => {
    // Default safe channels that fit in most status bars
    const defaultChannels = ['rpm', 'afr', 'map', 'coolant'].filter(
      name => normalizedChannels.some(ch => ch.name === name)
    );
    onChannelsChange(defaultChannels);
  };

  const channelLabels = new Map(normalizedChannels.map(ch => [ch.name, ch]));

  return (
    <div className="channel-selector">
      <div className="selector-header">
        <h4>Status Bar Channels</h4>
        <p className="selector-description">
          Select up to {maxChannels} channels to display in the status bar. 
          Drag to reorder. Click edit buttons to change order.
        </p>
      </div>

      {/* Selected Channels */}
      <div className="selected-section">
        <div className="section-header">
          <label>Selected Channels ({selectedChannels.length}/{maxChannels})</label>
          <button
            className="reset-btn"
            onClick={handleResetToDefaults}
            title="Reset to default channels"
          >
            🔄 Reset to Defaults
          </button>
        </div>

        {selectedChannels.length === 0 ? (
          <div className="empty-state">
            No channels selected. Click "Add channel" below to get started.
          </div>
        ) : (
          <div className="selected-list">
            {selectedChannels.map((channelName, index) => {
              const ch = channelLabels.get(channelName);
              return (
                <div
                  key={`selected-${index}`}
                  className="channel-item selected"
                  draggable
                  onDragStart={() => handleDragStart(index)}
                  onDragOver={handleDragOver}
                  onDragLeave={handleDragLeave}
                  onDrop={(e) => handleDrop(index, e)}
                >
                  <span className="drag-handle">⋮⋮</span>
                  <div className="channel-info">
                    <span className="channel-name">{ch?.name}</span>
                    {ch?.label && <span className="channel-label">{ch.label}</span>}
                    {ch?.units && <span className="channel-units">{ch.units}</span>}
                  </div>
                  <div className="channel-controls">
                    <button
                      className="arrow-btn"
                      onClick={() => handleMoveUp(index)}
                      disabled={index === 0}
                      title="Move up"
                      aria-label={`Move ${ch?.name} up`}
                    >
                      ▲
                    </button>
                    <button
                      className="arrow-btn"
                      onClick={() => handleMoveDown(index)}
                      disabled={index === selectedChannels.length - 1}
                      title="Move down"
                      aria-label={`Move ${ch?.name} down`}
                    >
                      ▼
                    </button>
                    <button
                      className="remove-btn"
                      onClick={() => handleRemoveChannel(index)}
                      title="Remove channel"
                      aria-label={`Remove ${ch?.name}`}
                    >
                      ✕
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Available Channels */}
      {selectedChannels.length < maxChannels && (
        <div className="available-section">
          <div className="section-header">
            <label>Available Channels</label>
            <span className="available-count">{filteredUnselected.length} available</span>
          </div>

          <input
            type="text"
            className="search-input"
            placeholder="Search channels..."
            value={filterText}
            onChange={(e) => setFilterText(e.target.value)}
            aria-label="Search available channels"
          />

          {filteredUnselected.length === 0 ? (
            <div className="empty-state">
              {filterText ? 'No channels match your search.' : 'All available channels are already selected.'}
            </div>
          ) : (
            <div className="available-list">
              {filteredUnselected.map((ch) => (
                <div key={ch.name} className="channel-item available">
                  <div className="channel-info">
                    <span className="channel-name">{ch.name}</span>
                    {ch.label && <span className="channel-label">{ch.label}</span>}
                    {ch.units && <span className="channel-units">{ch.units}</span>}
                  </div>
                  <button
                    className="add-btn"
                    onClick={() => handleAddChannel(ch.name ?? '')}
                    title="Add channel"
                    aria-label={`Add ${ch.name}`}
                  >
                    ＋
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Info Box */}
      <div className="info-box">
        <p>
          💡 <strong>Tip:</strong> The status bar shows {Math.floor(12 / 1.5)} channels by default. 
          More channels may require pagination when connected to ECU.
        </p>
      </div>
    </div>
  );
}
