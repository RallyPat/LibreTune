import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save, open } from '@tauri-apps/plugin-dialog';
import { useRealtimeStore } from '../../stores/realtimeStore';
import LoggerStatsPanel from './LoggerStatsPanel';
import './DataLogView.css';

interface LoggingStatus {
  is_recording: boolean;
  entry_count: number;
  duration_ms: number;
  channels: string[];
}

interface LogEntry {
  timestamp_ms: number;
  values: Record<string, number>;
}

type ViewMode = 'live' | 'playback';
type PlaybackSpeed = 0.25 | 0.5 | 1 | 2 | 4;

// Simple line chart component using canvas
const LineChart: React.FC<{
  data: { x: number; values: Record<string, number> }[];
  channels: string[];
  selectedChannels: string[];
  width: number;
  height: number;
  cursorPosition?: number; // 0-1 for playback position
  onSeek?: (position: number) => void; // Click to seek callback
}> = ({ data, channels, selectedChannels, width, height, cursorPosition, onSeek }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
  const handleClick = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!onSeek || data.length < 2) return;
    const canvas = e.currentTarget;
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const padding = { left: 60, right: 80 };
    const chartWidth = width - padding.left - padding.right;
    const position = Math.max(0, Math.min(1, (x - padding.left) / chartWidth));
    onSeek(position);
  }, [onSeek, data.length, width]);
  
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    
    // Clear canvas
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, width, height);
    
    if (data.length < 2) {
      ctx.fillStyle = '#666';
      ctx.font = '14px system-ui';
      ctx.textAlign = 'center';
      ctx.fillText('Waiting for data...', width / 2, height / 2);
      return;
    }
    
    const padding = { top: 20, right: 80, bottom: 40, left: 60 };
    const chartWidth = width - padding.left - padding.right;
    const chartHeight = height - padding.top - padding.bottom;
    
    // Get time range
    const minTime = data[0].x;
    const maxTime = data[data.length - 1].x;
    const timeRange = maxTime - minTime || 1;
    
    // Colors for different channels
    const colors = [
      '#00ff88', '#00aaff', '#ff6644', '#ffcc00', '#ff44ff',
      '#44ffff', '#88ff00', '#ff8844', '#aa44ff', '#44ff88'
    ];
    
    // Draw grid
    ctx.strokeStyle = '#333';
    ctx.lineWidth = 1;
    
    // Vertical grid lines (time)
    for (let i = 0; i <= 5; i++) {
      const x = padding.left + (i / 5) * chartWidth;
      ctx.beginPath();
      ctx.moveTo(x, padding.top);
      ctx.lineTo(x, height - padding.bottom);
      ctx.stroke();
      
      // Time labels
      const time = minTime + (i / 5) * timeRange;
      ctx.fillStyle = '#888';
      ctx.font = '11px system-ui';
      ctx.textAlign = 'center';
      ctx.fillText(`${(time / 1000).toFixed(1)}s`, x, height - padding.bottom + 20);
    }
    
    // Draw each selected channel
    selectedChannels.forEach((channel, channelIndex) => {
      const channelData = data.map(d => d.values[channel]).filter(v => v !== undefined);
      if (channelData.length < 2) return;
      
      // Auto-scale for this channel
      const minVal = Math.min(...channelData);
      const maxVal = Math.max(...channelData);
      const range = maxVal - minVal || 1;
      const scale = chartHeight / range;
      
      const color = colors[channelIndex % colors.length];
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.beginPath();
      
      data.forEach((point, i) => {
        const val = point.values[channel];
        if (val === undefined) return;
        
        const x = padding.left + ((point.x - minTime) / timeRange) * chartWidth;
        const y = height - padding.bottom - ((val - minVal) * scale);
        
        if (i === 0 || data[i - 1].values[channel] === undefined) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      });
      
      ctx.stroke();
      
      // Draw channel label with current value
      const lastVal = channelData[channelData.length - 1];
      const labelY = padding.top + 20 + channelIndex * 20;
      ctx.fillStyle = color;
      ctx.font = 'bold 12px system-ui';
      ctx.textAlign = 'left';
      ctx.fillText(`${channel}: ${lastVal?.toFixed(2) ?? '-'}`, width - padding.right + 8, labelY);
    });
    
    // Draw axes
    ctx.strokeStyle = '#666';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(padding.left, padding.top);
    ctx.lineTo(padding.left, height - padding.bottom);
    ctx.lineTo(width - padding.right, height - padding.bottom);
    ctx.stroke();
    
    // Draw playback cursor if in playback mode
    if (cursorPosition !== undefined && cursorPosition >= 0 && cursorPosition <= 1) {
      const cursorX = padding.left + cursorPosition * chartWidth;
      ctx.strokeStyle = '#ff4444';
      ctx.lineWidth = 2;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(cursorX, padding.top);
      ctx.lineTo(cursorX, height - padding.bottom);
      ctx.stroke();
      ctx.setLineDash([]);
      
      // Draw cursor time label
      const cursorTime = minTime + cursorPosition * timeRange;
      ctx.fillStyle = '#ff4444';
      ctx.font = 'bold 11px system-ui';
      ctx.textAlign = 'center';
      ctx.fillText(`${(cursorTime / 1000).toFixed(2)}s`, cursorX, padding.top - 6);
    }
    
  }, [data, channels, selectedChannels, width, height, cursorPosition]);
  
  return (
    <canvas 
      ref={canvasRef} 
      width={width} 
      height={height} 
      className="log-chart-canvas"
      onClick={handleClick}
      style={{ cursor: onSeek ? 'crosshair' : 'default' }}
    />
  );
};

// DataLogView no longer requires props - uses Zustand store for realtime data
export const DataLogView: React.FC = () => {
  // Get realtime data from Zustand store
  const realtimeData = useRealtimeStore((state) => state.channels);

  const [isRecording, setIsRecording] = useState(false);
  const [status, setStatus] = useState<LoggingStatus | null>(null);
  const [logData, setLogData] = useState<{ x: number; values: Record<string, number> }[]>([]);
  const [availableChannels, setAvailableChannels] = useState<string[]>([]);
  const [selectedChannels, setSelectedChannels] = useState<string[]>(['RPM', 'MAP', 'AFR']);
  const [sampleRate, setSampleRate] = useState(10);
  const [chartSize, setChartSize] = useState({ width: 800, height: 400 });
  const chartContainerRef = useRef<HTMLDivElement>(null);
  
  // Playback state
  const [viewMode, setViewMode] = useState<ViewMode>('live');
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackPosition, setPlaybackPosition] = useState(0); // 0-1
  const [playbackSpeed, setPlaybackSpeed] = useState<PlaybackSpeed>(1);
  const [loadedFileName, setLoadedFileName] = useState<string | null>(null);
  const [showStats, setShowStats] = useState(false);
  const [selectedStatsChannel, setSelectedStatsChannel] = useState<string | null>(null);
  const playbackIntervalRef = useRef<number | null>(null);
  
  // Update chart size based on container
  useEffect(() => {
    const updateSize = () => {
      if (chartContainerRef.current) {
        const rect = chartContainerRef.current.getBoundingClientRect();
        setChartSize({
          width: Math.max(400, rect.width - 20),
          height: Math.max(300, rect.height - 20)
        });
      }
    };
    
    updateSize();
    window.addEventListener('resize', updateSize);
    return () => window.removeEventListener('resize', updateSize);
  }, []);
  
  // Poll status while recording
  useEffect(() => {
    if (!isRecording) return;
    
    const interval = setInterval(async () => {
      try {
        const newStatus = await invoke<LoggingStatus>('get_logging_status');
        setStatus(newStatus);
        
        // Fetch latest entries for chart
        const entries = await invoke<LogEntry[]>('get_log_entries', {
          startIndex: Math.max(0, newStatus.entry_count - 500),
          count: 500
        });
        
        setLogData(entries.map(e => ({
          x: e.timestamp_ms,
          values: e.values
        })));
        
      } catch (err) {
        console.error('Failed to get logging status:', err);
      }
    }, 200);
    
    return () => clearInterval(interval);
  }, [isRecording]);
  
  // Update available channels from realtime data
  useEffect(() => {
    const channels = Object.keys(realtimeData);
    if (channels.length > 0 && availableChannels.length === 0) {
      setAvailableChannels(channels);
      // Set default selected channels
      const defaults = ['RPM', 'MAP', 'AFR', 'coolant', 'TPS'].filter(c => channels.includes(c));
      if (defaults.length > 0) {
        setSelectedChannels(defaults.slice(0, 4));
      } else {
        setSelectedChannels(channels.slice(0, 4));
      }
    }
  }, [realtimeData, availableChannels]);
  
  const handleStartLogging = useCallback(async () => {
    try {
      await invoke('start_logging', { sampleRate });
      setIsRecording(true);
      setLogData([]);
    } catch (err) {
      console.error('Failed to start logging:', err);
    }
  }, [sampleRate]);
  
  const handleStopLogging = useCallback(async () => {
    try {
      await invoke('stop_logging');
      setIsRecording(false);
      
      // Fetch final status
      const finalStatus = await invoke<LoggingStatus>('get_logging_status');
      setStatus(finalStatus);
    } catch (err) {
      console.error('Failed to stop logging:', err);
    }
  }, []);
  
  const handleClearLog = useCallback(async () => {
    try {
      await invoke('clear_log');
      setLogData([]);
      setStatus(null);
    } catch (err) {
      console.error('Failed to clear log:', err);
    }
  }, []);
  
  const handleSaveLog = useCallback(async () => {
    try {
      const path = await save({
        defaultPath: `datalog_${new Date().toISOString().split('T')[0]}.csv`,
        filters: [{ name: 'CSV Files', extensions: ['csv'] }]
      });
      
      if (path) {
        await invoke('save_log', { path });
      }
    } catch (err) {
      console.error('Failed to save log:', err);
    }
  }, []);
  
  // Parse CSV file - supports both LibreTune and TunerStudio formats
  const parseLogCsv = useCallback((content: string, _fileName: string): { 
    data: { x: number; values: Record<string, number> }[];
    channels: string[];
  } => {
    const lines = content.trim().split('\n');
    if (lines.length < 2) return { data: [], channels: [] };
    
    // Parse header - handle both formats
    const headerLine = lines[0];
    const headers = headerLine.split(',').map(h => h.trim().replace(/^"|"$/g, ''));
    
    // Detect format: TunerStudio uses "Time" column, LibreTune uses "timestamp_ms"
    const timeColIndex = headers.findIndex(h => 
      h.toLowerCase() === 'time' || 
      h.toLowerCase() === 'timestamp_ms' ||
      h.toLowerCase() === 'timestamp'
    );
    
    const isTunerStudioFormat = headers.some(h => h.toLowerCase() === 'time');
    const channels = headers.filter((_, i) => i !== timeColIndex);
    
    const data: { x: number; values: Record<string, number> }[] = [];
    
    for (let i = 1; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line) continue;
      
      // Parse CSV values (handle quoted values)
      const values: string[] = [];
      let current = '';
      let inQuotes = false;
      for (const char of line) {
        if (char === '"') {
          inQuotes = !inQuotes;
        } else if (char === ',' && !inQuotes) {
          values.push(current.trim());
          current = '';
        } else {
          current += char;
        }
      }
      values.push(current.trim());
      
      if (values.length < headers.length) continue;
      
      // Parse timestamp
      let timestamp: number;
      if (timeColIndex >= 0) {
        const timeStr = values[timeColIndex];
        if (isTunerStudioFormat) {
          // TunerStudio format: seconds with decimals
          timestamp = parseFloat(timeStr) * 1000;
        } else {
          // LibreTune format: milliseconds
          timestamp = parseFloat(timeStr);
        }
      } else {
        // No time column - use index * 100ms
        timestamp = (i - 1) * 100;
      }
      
      if (isNaN(timestamp)) continue;
      
      const entry: Record<string, number> = {};
      let channelIdx = 0;
      for (let j = 0; j < headers.length; j++) {
        if (j === timeColIndex) continue;
        const val = parseFloat(values[j]);
        if (!isNaN(val)) {
          entry[channels[channelIdx]] = val;
        }
        channelIdx++;
      }
      
      data.push({ x: timestamp, values: entry });
    }
    
    return { data, channels };
  }, []);
  
  const handleLoadLog = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Log Files', extensions: ['csv', 'msl', 'log'] }]
      });
      
      if (!selected) return;
      
      // Read and parse the file
      const content = await invoke<string>('read_text_file', { path: selected });
      const fileName = typeof selected === 'string' 
        ? selected.split('/').pop() || selected.split('\\').pop() || 'log.csv'
        : 'log.csv';
      
      const { data, channels } = parseLogCsv(content, fileName);
      
      if (data.length === 0) {
        console.error('No valid data found in log file');
        return;
      }
      
      // Switch to playback mode
      setLogData(data);
      setAvailableChannels(channels);
      setSelectedChannels(channels.slice(0, 4));
      setViewMode('playback');
      setPlaybackPosition(0);
      setIsPlaying(false);
      setLoadedFileName(fileName);
      
      // Create a status-like object for display
      const duration = data.length > 0 ? data[data.length - 1].x - data[0].x : 0;
      setStatus({
        is_recording: false,
        entry_count: data.length,
        duration_ms: duration,
        channels: channels
      });
      
    } catch (err) {
      console.error('Failed to load log:', err);
    }
  }, [parseLogCsv]);
  
  // Playback controls
  const handlePlayPause = useCallback(() => {
    setIsPlaying(prev => !prev);
  }, []);
  
  const handleSeek = useCallback((position: number) => {
    setPlaybackPosition(Math.max(0, Math.min(1, position)));
  }, []);
  
  const handleBackToLive = useCallback(() => {
    setViewMode('live');
    setIsPlaying(false);
    setLoadedFileName(null);
    setPlaybackPosition(0);
  }, []);
  
  // Playback timer
  useEffect(() => {
    if (viewMode !== 'playback' || !isPlaying || logData.length < 2) {
      if (playbackIntervalRef.current) {
        clearInterval(playbackIntervalRef.current);
        playbackIntervalRef.current = null;
      }
      return;
    }
    
    const totalDuration = logData[logData.length - 1].x - logData[0].x;
    const updateInterval = 50; // 20 updates per second
    const positionIncrement = (updateInterval * playbackSpeed) / totalDuration;
    
    playbackIntervalRef.current = window.setInterval(() => {
      setPlaybackPosition(prev => {
        const next = prev + positionIncrement;
        if (next >= 1) {
          setIsPlaying(false);
          return 1;
        }
        return next;
      });
    }, updateInterval);
    
    return () => {
      if (playbackIntervalRef.current) {
        clearInterval(playbackIntervalRef.current);
        playbackIntervalRef.current = null;
      }
    };
  }, [viewMode, isPlaying, logData, playbackSpeed]);
  
  // Get current playback values for display
  const getCurrentPlaybackValues = useCallback((): Record<string, number> => {
    if (viewMode !== 'playback' || logData.length < 2) return {};
    
    const currentTime = logData[0].x + playbackPosition * (logData[logData.length - 1].x - logData[0].x);
    
    // Find the closest data point
    let closest = logData[0];
    for (const point of logData) {
      if (Math.abs(point.x - currentTime) < Math.abs(closest.x - currentTime)) {
        closest = point;
      }
    }
    
    return closest.values;
  }, [viewMode, logData, playbackPosition]);
  
  const toggleChannel = useCallback((channel: string) => {
    setSelectedChannels(prev => 
      prev.includes(channel)
        ? prev.filter(c => c !== channel)
        : [...prev, channel].slice(-6) // Max 6 channels
    );
  }, []);
  
  const formatDuration = (ms: number) => {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  };
  
  // Get display values - use playback or realtime based on mode
  const displayValues = viewMode === 'playback' ? getCurrentPlaybackValues() : realtimeData;
  
  return (
    <div className="datalog-view">
      <div className="datalog-header">
        <div className="header-left">
          <h2>📊 Data Logging</h2>
          <span className={`mode-badge ${viewMode}`}>
            {viewMode === 'live' ? '🔴 Live' : '📂 Playback'}
          </span>
          {loadedFileName && (
            <span className="loaded-file" title={loadedFileName}>
              {loadedFileName.length > 25 ? '...' + loadedFileName.slice(-22) : loadedFileName}
            </span>
          )}
        </div>
        
        <div className="datalog-controls">
          {viewMode === 'live' ? (
            <>
              <div className="control-group">
                <label>Sample Rate:</label>
                <select 
                  value={sampleRate} 
                  onChange={e => setSampleRate(Number(e.target.value))}
                  disabled={isRecording}
                >
                  <option value={1}>1 Hz</option>
                  <option value={5}>5 Hz</option>
                  <option value={10}>10 Hz</option>
                  <option value={20}>20 Hz</option>
                  <option value={50}>50 Hz</option>
                  <option value={100}>100 Hz</option>
                </select>
              </div>
              
              <button 
                className={`log-button ${isRecording ? 'stop' : 'start'}`}
                onClick={isRecording ? handleStopLogging : handleStartLogging}
              >
                {isRecording ? '⏹ Stop' : '⏺ Record'}
              </button>
              
              <button 
                className="log-button secondary"
                onClick={handleClearLog}
                disabled={isRecording}
              >
                🗑️ Clear
              </button>
              
              <button 
                className="log-button secondary"
                onClick={handleSaveLog}
                disabled={isRecording || logData.length === 0}
              >
                💾 Save
              </button>
              
              <button 
                className="log-button secondary"
                onClick={handleLoadLog}
                disabled={isRecording}
              >
                📂 Load
              </button>
            </>
          ) : (
            <>
              <button 
                className={`log-button ${isPlaying ? 'stop' : 'start'}`}
                onClick={handlePlayPause}
              >
                {isPlaying ? '⏸ Pause' : '▶ Play'}
              </button>
              
              <div className="control-group">
                <label>Speed:</label>
                <select 
                  value={playbackSpeed} 
                  onChange={e => setPlaybackSpeed(Number(e.target.value) as PlaybackSpeed)}
                >
                  <option value={0.25}>0.25x</option>
                  <option value={0.5}>0.5x</option>
                  <option value={1}>1x</option>
                  <option value={2}>2x</option>
                  <option value={4}>4x</option>
                </select>
              </div>
              
              <button 
                className="log-button secondary"
                onClick={handleLoadLog}
              >
                📂 Load Another
              </button>
              
              <button 
                className="log-button secondary"
                onClick={handleBackToLive}
              >
                🔴 Back to Live
              </button>
            </>
          )}
        </div>
      </div>
      
      {/* Playback seek bar */}
      {viewMode === 'playback' && logData.length > 0 && (
        <div className="playback-bar">
          <span className="playback-time">
            {formatDuration(logData[0].x + playbackPosition * (logData[logData.length - 1].x - logData[0].x))}
          </span>
          <input
            type="range"
            className="playback-slider"
            min={0}
            max={1}
            step={0.001}
            value={playbackPosition}
            onChange={e => handleSeek(parseFloat(e.target.value))}
          />
          <span className="playback-time">
            {formatDuration(logData[logData.length - 1].x - logData[0].x)}
          </span>
        </div>
      )}
      
      {status && (
        <div className="log-status">
          <span className={`status-indicator ${isRecording ? 'recording' : viewMode === 'playback' ? 'playback' : 'stopped'}`}>
            {isRecording ? '🔴 Recording' : viewMode === 'playback' ? '📂 Loaded' : '⏸ Stopped'}
          </span>
          <span className="status-stat">{status.entry_count.toLocaleString()} samples</span>
          <span className="status-stat">{formatDuration(status.duration_ms)}</span>
          <span className="status-stat">{status.channels.length} channels</span>
        </div>
      )}
      
      <div className="datalog-content">
        <div className="channel-selector">
          <h4>Channels</h4>
          <div className="channel-list">
            {availableChannels.map((channel) => (
              <label 
                key={channel} 
                className={`channel-item ${selectedChannels.includes(channel) ? 'selected' : ''}`}
              >
                <input
                  type="checkbox"
                  checked={selectedChannels.includes(channel)}
                  onChange={() => toggleChannel(channel)}
                />
                <span 
                  className="channel-color"
                  style={{ 
                    background: selectedChannels.includes(channel) 
                      ? ['#00ff88', '#00aaff', '#ff6644', '#ffcc00', '#ff44ff', '#44ffff'][
                          selectedChannels.indexOf(channel) % 6
                        ]
                      : '#444'
                  }}
                />
                <span className="channel-name">{channel}</span>
                <span className="channel-value">
                  {displayValues[channel]?.toFixed(2) ?? '-'}
                </span>
              </label>
            ))}
          </div>
        </div>
        
        <div className="chart-container" ref={chartContainerRef}>
          <LineChart
            data={logData}
            channels={availableChannels}
            selectedChannels={selectedChannels}
            width={chartSize.width}
            height={chartSize.height}
            cursorPosition={viewMode === 'playback' ? playbackPosition : undefined}
            onSeek={viewMode === 'playback' ? handleSeek : undefined}
          />
        </div>

        {showStats && (
          <LoggerStatsPanel
            data={logData}
            selectedChannels={selectedStatsChannel ? [selectedStatsChannel] : selectedChannels}
            onChannelSelect={setSelectedStatsChannel}
          />
        )}
      </div>

      <div className="stats-toggle">
        <button 
          className={`stat-button ${showStats ? 'active' : ''}`}
          onClick={() => setShowStats(!showStats)}
          title="Toggle statistics panel"
        >
          📊 {showStats ? 'Hide' : 'Show'} Stats
        </button>
      </div>
    </div>
  );
};

export default DataLogView;
