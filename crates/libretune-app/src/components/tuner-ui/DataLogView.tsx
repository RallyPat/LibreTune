import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
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

interface DataLogViewProps {
  realtimeData: Record<string, number>;
}

// Simple line chart component using canvas
const LineChart: React.FC<{
  data: { x: number; values: Record<string, number> }[];
  channels: string[];
  selectedChannels: string[];
  width: number;
  height: number;
}> = ({ data, channels, selectedChannels, width, height }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
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
    
  }, [data, channels, selectedChannels, width, height]);
  
  return <canvas ref={canvasRef} width={width} height={height} className="log-chart-canvas" />;
};

export const DataLogView: React.FC<DataLogViewProps> = ({ realtimeData }) => {
  const [isRecording, setIsRecording] = useState(false);
  const [status, setStatus] = useState<LoggingStatus | null>(null);
  const [logData, setLogData] = useState<{ x: number; values: Record<string, number> }[]>([]);
  const [availableChannels, setAvailableChannels] = useState<string[]>([]);
  const [selectedChannels, setSelectedChannels] = useState<string[]>(['RPM', 'MAP', 'AFR']);
  const [sampleRate, setSampleRate] = useState(10);
  const [chartSize, setChartSize] = useState({ width: 800, height: 400 });
  const chartContainerRef = useRef<HTMLDivElement>(null);
  
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
  
  return (
    <div className="datalog-view">
      <div className="datalog-header">
        <h2>📊 Data Logging</h2>
        
        <div className="datalog-controls">
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
            💾 Save CSV
          </button>
        </div>
      </div>
      
      {status && (
        <div className="log-status">
          <span className={`status-indicator ${isRecording ? 'recording' : 'stopped'}`}>
            {isRecording ? '🔴 Recording' : '⏸ Stopped'}
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
                  {realtimeData[channel]?.toFixed(2) ?? '-'}
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
          />
        </div>
      </div>
    </div>
  );
};

export default DataLogView;
