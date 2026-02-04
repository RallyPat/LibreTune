import React, { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './ActionRecorder.css';

interface Action {
  type: string;
  data: Record<string, any>;
}

interface ActionSet {
  id: string;
  name: string;
  description: string;
  version: string;
  actions: Action[];
  metadata: {
    created_by: string;
    created_at: string;
    modified_at: string;
    tags: string[];
    compatible_ecus: string[];
  };
}

interface ActionRecorderProps {
  isOpen: boolean;
  onClose: () => void;
}

export const ActionRecorder: React.FC<ActionRecorderProps> = ({ isOpen, onClose }) => {
  const [isRecording, setIsRecording] = useState(false);
  const [recordingName, setRecordingName] = useState('');
  const [recordingDescription, setRecordingDescription] = useState('');
  const [actionSets, setActionSets] = useState<ActionSet[]>([]);
  const [selectedSet, setSelectedSet] = useState<ActionSet | null>(null);
  const [playbackSpeed, setPlaybackSpeed] = useState(1);
  const [isPlaying, setIsPlaying] = useState(false);

  const handleStartRecording = useCallback(() => {
    if (!recordingName.trim()) {
      alert('Please enter a name for the action set');
      return;
    }

    setIsRecording(true);
    // In real implementation, this would call backend to start recording
    console.log('Starting action recording:', recordingName);
  }, [recordingName]);

  const handleStopRecording = useCallback(() => {
    setIsRecording(false);
    // In real implementation, this would call backend to stop recording and get ActionSet
    console.log('Stopped recording');
    
    // Reset form
    setRecordingName('');
    setRecordingDescription('');
  }, []);

  const handlePlayAction = useCallback(async (set: ActionSet) => {
    setIsPlaying(true);
    try {
      // Validate before playing
      const result = await invoke('validate_action_set', { actionSet: set });
      console.log('Action set validated:', result);
      
      // In real implementation, this would play the actions
      console.log('Playing action set:', set.name);
      
      // Simulate playback completion
      setTimeout(() => setIsPlaying(false), 1000);
    } catch (err) {
      console.error('Failed to play action set:', err);
      setIsPlaying(false);
    }
  }, []);

  const handleExportActionSet = useCallback(async (set: ActionSet) => {
    try {
      const json = JSON.stringify(set, null, 2);
      
      // Use Tauri save dialog
      const filePath = await invoke('save_file_dialog', {
        defaultName: `${set.name}.ltaction`,
        filters: [{ name: 'LibreTune Action', extensions: ['ltaction'] }],
      }).catch(() => null);
      
      if (filePath) {
        await invoke('write_text_file', { path: filePath, content: json });
        console.log('Action set exported to:', filePath);
      }
    } catch (err) {
      console.error('Failed to export action set:', err);
    }
  }, []);

  const handleImportActionSet = useCallback(async () => {
    try {
      const filePath = await invoke('open_file_dialog', {
        filters: [{ name: 'LibreTune Action', extensions: ['ltaction'] }],
      }).catch(() => null);
      
      if (filePath) {
        const content = await invoke('read_text_file', { path: filePath });
        const set = JSON.parse(content as string) as ActionSet;
        setActionSets([...actionSets, set]);
        console.log('Action set imported:', set.name);
      }
    } catch (err) {
      console.error('Failed to import action set:', err);
    }
  }, [actionSets]);

  const handleInsertLuaScript = useCallback(() => {
    if (!selectedSet) {
      alert('Please select an action set first');
      return;
    }
    
    const script = `-- Custom Lua script
-- Available variables: rpm, map, iat, clt, afr, etc.
-- Return modified values or nil for no change

local new_afr = 14.0
return new_afr`;
    
    const luaAction: Action = {
      type: 'ExecuteLuaScript',
      data: {
        script,
        description: 'Custom calculation',
      },
    };
    
    selectedSet.actions.push(luaAction);
    setSelectedSet({ ...selectedSet });
  }, [selectedSet]);

  if (!isOpen) return null;

  return (
    <div className="action-recorder-overlay" onClick={onClose}>
      <div className="action-recorder-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="action-recorder-header">
          <h2>Action Recorder</h2>
          <button className="close-btn" onClick={onClose}>✕</button>
        </div>

        <div className="action-recorder-content">
          {/* Recording Panel */}
          <div className="recorder-panel">
            <h3>Record Actions</h3>
            
            {!isRecording ? (
              <>
                <div className="form-group">
                  <label>Action Set Name:</label>
                  <input
                    type="text"
                    value={recordingName}
                    onChange={(e) => setRecordingName(e.target.value)}
                    placeholder="e.g., Fuel Tuning Workflow"
                    disabled={isRecording}
                  />
                </div>

                <div className="form-group">
                  <label>Description:</label>
                  <textarea
                    value={recordingDescription}
                    onChange={(e) => setRecordingDescription(e.target.value)}
                    placeholder="What does this action set do?"
                    rows={3}
                    disabled={isRecording}
                  />
                </div>

                <button
                  className="btn btn-primary"
                  onClick={handleStartRecording}
                  disabled={!recordingName.trim()}
                >
                  🔴 Start Recording
                </button>
              </>
            ) : (
              <>
                <div className="recording-indicator">
                  <span className="recording-dot"></span>
                  Recording: {recordingName}
                </div>
                <button
                  className="btn btn-danger"
                  onClick={handleStopRecording}
                >
                  ⏹ Stop Recording
                </button>
              </>
            )}
          </div>

          {/* Action Sets Panel */}
          <div className="action-sets-panel">
            <h3>Action Sets</h3>
            
            <div className="action-sets-list">
              {actionSets.length === 0 ? (
                <div className="empty-state">No action sets yet. Record one to get started.</div>
              ) : (
                actionSets.map((set) => (
                  <div
                    key={set.id}
                    className={`action-set-item ${selectedSet?.id === set.id ? 'selected' : ''}`}
                    onClick={() => setSelectedSet(set)}
                  >
                    <div className="set-header">
                      <div className="set-name">{set.name}</div>
                      <div className="set-actions-count">{set.actions.length} actions</div>
                    </div>
                    <div className="set-description">{set.description}</div>
                    <div className="set-meta">
                      <span>by {set.metadata.created_by}</span>
                      <span>{new Date(set.metadata.created_at).toLocaleDateString()}</span>
                    </div>
                  </div>
                ))
              )}
            </div>

            <div className="button-group">
              <button className="btn btn-secondary" onClick={handleImportActionSet}>
                📁 Import .ltaction
              </button>
              {selectedSet && (
                <>
                  <button className="btn btn-secondary" onClick={() => handleExportActionSet(selectedSet)}>
                    💾 Export
                  </button>
                  <button
                    className="btn btn-secondary"
                    onClick={() => handleInsertLuaScript()}
                  >
                    + Insert Lua Script
                  </button>
                  <button
                    className="btn btn-primary"
                    onClick={() => handlePlayAction(selectedSet)}
                    disabled={isPlaying}
                  >
                    {isPlaying ? '⏳ Playing...' : '▶ Play'}
                  </button>
                </>
              )}
            </div>

            {selectedSet && (
              <div className="playback-controls">
                <label>Playback Speed:</label>
                <select value={playbackSpeed} onChange={(e) => setPlaybackSpeed(Number(e.target.value))}>
                  <option value={0.5}>0.5x</option>
                  <option value={1}>1x</option>
                  <option value={2}>2x</option>
                  <option value={5}>5x</option>
                </select>
              </div>
            )}
          </div>

          {/* Action Timeline */}
          {selectedSet && (
            <div className="action-timeline-panel">
              <h3>Actions Timeline</h3>
              <div className="timeline">
                {selectedSet.actions.map((action, idx) => (
                  <div key={idx} className="timeline-item">
                    <div className="timeline-marker">{idx + 1}</div>
                    <div className="timeline-content">
                      <div className="action-type">{action.type}</div>
                      <div className="action-details">
                        {action.type === 'ExecuteLuaScript' && (
                          <code>{(action.data.description || 'Lua Script').substring(0, 50)}...</code>
                        )}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        <div className="action-recorder-footer">
          <button className="btn btn-secondary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
