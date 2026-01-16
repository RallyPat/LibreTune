import './StatusBar.css';
import { useChannels } from '../../stores/realtimeStore';

interface StatusBarProps {
  ecuStatus: 'connected' | 'connecting' | 'error' | 'disconnected';
  ecuSignature?: string;
  currentPage?: string;
  unitsSystem?: 'metric' | 'imperial';
}

export function StatusBar({
  ecuStatus,
  ecuSignature,
  currentPage,
  unitsSystem = 'metric',
}: StatusBarProps) {
  // Get realtime data from Zustand store
  const realtimeData = useChannels(['rpm', 'afr', 'map', 'coolant']);
  
  const getStatusLabel = () => {
    switch (ecuStatus) {
      case 'connected': return 'Connected';
      case 'connecting': return 'Connecting...';
      case 'error': return 'Error';
      case 'disconnected': return 'Disconnected';
      default: return 'Unknown';
    }
  };

  const getStatusClass = () => {
    switch (ecuStatus) {
      case 'connected': return 'status-connected';
      case 'connecting': return 'status-connecting';
      case 'error': return 'status-error';
      case 'disconnected': return 'status-disconnected';
      default: return '';
    }
  };

  // Unit conversion functions
  const convertTemperature = (value: number) =>
    unitsSystem === 'imperial' ? (value * 9/5) + 32 : value;

  const convertPressure = (value: number) =>
    unitsSystem === 'imperial' ? value * 0.145038 : value;

  const getTemperatureUnit = () =>
    unitsSystem === 'imperial' ? '°F' : '°C';

  const getPressureUnit = () =>
    unitsSystem === 'imperial' ? 'psi' : 'kPa';

  // Format realtime values for display
  const rpm = realtimeData?.rpm ?? 0;
  const afr = realtimeData?.afr ?? 0;
  const map = realtimeData?.map ?? 0;
  const coolant = realtimeData?.coolant ?? 0;

  const displayCoolant = convertTemperature(coolant);
  const displayMap = convertPressure(map);

  return (
    <div className="statusbar">
      {/* Left section: Connection status */}
      <div className="statusbar-section statusbar-left">
        <div className={`statusbar-indicator ${getStatusClass()}`}>
          <span className="statusbar-dot"></span>
          <span className="statusbar-text">{getStatusLabel()}</span>
        </div>
        {ecuSignature && (
          <div className="statusbar-item statusbar-signature">
            {ecuSignature}
          </div>
        )}
      </div>

      {/* Center section: Current page/table */}
      <div className="statusbar-section statusbar-center">
        {currentPage && (
          <div className="statusbar-item statusbar-page">
            {currentPage}
          </div>
        )}
      </div>

      {/* Right section: Realtime values */}
      <div className="statusbar-section statusbar-right">
        {ecuStatus === 'connected' && (
          <>
            <div className="statusbar-value">
              <span className="statusbar-label">RPM:</span>
              <span className="statusbar-number">{rpm.toFixed(0)}</span>
            </div>
            <div className="statusbar-value">
              <span className="statusbar-label">AFR:</span>
              <span className="statusbar-number">{afr.toFixed(1)}</span>
            </div>
            <div className="statusbar-value">
              <span className="statusbar-label">MAP:</span>
              <span className="statusbar-number">{displayMap.toFixed(0)} {getPressureUnit()}</span>
            </div>
            <div className="statusbar-value">
              <span className="statusbar-label">CLT:</span>
              <span className="statusbar-number">{displayCoolant.toFixed(0)}{getTemperatureUnit()}</span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default StatusBar;
