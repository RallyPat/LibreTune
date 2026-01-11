/**
 * Port Editor Component
 * 
 * Allows users to configure ECU pin assignments for various functions:
 * - Injector outputs (INJ1-INJ8+)
 * - Ignition outputs (IGN1-IGN8+)
 * - Auxiliary outputs (AUX1-AUX16+)
 * - Analog inputs (ADC0-ADC15)
 * - Digital inputs (DIN1-DIN8)
 * - CAN bus configuration
 * 
 * Displays a visual pin diagram and allows drag-and-drop assignment
 * or dropdown selection for each function.
 */

import { useState, useCallback, useMemo } from 'react';
import {
  Cpu,
  Zap,
  Flame,
  Gauge,
  ToggleLeft,
  Save,
  RotateCcw,
  AlertTriangle,
  Check,
  ChevronDown,
  ChevronRight,
  Search,
} from 'lucide-react';
import './PortEditor.css';

// Pin function types
type PinFunction = 
  | 'injector'
  | 'ignition'
  | 'aux_output'
  | 'analog_input'
  | 'digital_input'
  | 'vr_input'
  | 'can_bus'
  | 'unused';

// Pin configuration
interface PinConfig {
  id: string;
  name: string;
  physicalPin: string;
  function: PinFunction;
  channel: number;
  inverted: boolean;
  pullup: boolean;
  description: string;
}

// Available hardware pins (example for common ECU)
interface HardwarePin {
  id: string;
  name: string;
  capabilities: PinFunction[];
  voltage: '5V' | '12V' | 'GND' | 'VR';
  description: string;
}

interface PortEditorProps {
  ecuType?: string;
  initialConfig?: PinConfig[];
  onSave?: (config: PinConfig[]) => void;
  onCancel?: () => void;
}

// Default hardware pins for a generic ECU (can be loaded from INI)
const DEFAULT_HARDWARE_PINS: HardwarePin[] = [
  // High-side outputs (injectors)
  { id: 'HSO1', name: 'High-Side 1', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 1' },
  { id: 'HSO2', name: 'High-Side 2', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 2' },
  { id: 'HSO3', name: 'High-Side 3', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 3' },
  { id: 'HSO4', name: 'High-Side 4', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 4' },
  { id: 'HSO5', name: 'High-Side 5', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 5' },
  { id: 'HSO6', name: 'High-Side 6', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 6' },
  { id: 'HSO7', name: 'High-Side 7', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 7' },
  { id: 'HSO8', name: 'High-Side 8', capabilities: ['injector', 'aux_output'], voltage: '12V', description: 'High-side output 8' },
  
  // Low-side outputs (ignition)
  { id: 'LSO1', name: 'Low-Side 1', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 1' },
  { id: 'LSO2', name: 'Low-Side 2', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 2' },
  { id: 'LSO3', name: 'Low-Side 3', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 3' },
  { id: 'LSO4', name: 'Low-Side 4', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 4' },
  { id: 'LSO5', name: 'Low-Side 5', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 5' },
  { id: 'LSO6', name: 'Low-Side 6', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 6' },
  { id: 'LSO7', name: 'Low-Side 7', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 7' },
  { id: 'LSO8', name: 'Low-Side 8', capabilities: ['ignition', 'aux_output'], voltage: '5V', description: 'Low-side output 8' },
  
  // Analog inputs
  { id: 'ADC0', name: 'Analog 0', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 0 (TPS)' },
  { id: 'ADC1', name: 'Analog 1', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 1 (MAP)' },
  { id: 'ADC2', name: 'Analog 2', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 2 (CLT)' },
  { id: 'ADC3', name: 'Analog 3', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 3 (IAT)' },
  { id: 'ADC4', name: 'Analog 4', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 4 (O2)' },
  { id: 'ADC5', name: 'Analog 5', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 5 (BATT)' },
  { id: 'ADC6', name: 'Analog 6', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 6' },
  { id: 'ADC7', name: 'Analog 7', capabilities: ['analog_input'], voltage: '5V', description: 'Analog input 7' },
  
  // Digital inputs
  { id: 'DIN1', name: 'Digital 1', capabilities: ['digital_input'], voltage: '5V', description: 'Digital input 1' },
  { id: 'DIN2', name: 'Digital 2', capabilities: ['digital_input'], voltage: '5V', description: 'Digital input 2' },
  { id: 'DIN3', name: 'Digital 3', capabilities: ['digital_input'], voltage: '5V', description: 'Digital input 3' },
  { id: 'DIN4', name: 'Digital 4', capabilities: ['digital_input'], voltage: '5V', description: 'Digital input 4' },
  
  // VR inputs (crank/cam)
  { id: 'VR1+', name: 'VR1+', capabilities: ['vr_input'], voltage: 'VR', description: 'VR input 1 positive (Crank)' },
  { id: 'VR1-', name: 'VR1-', capabilities: ['vr_input'], voltage: 'VR', description: 'VR input 1 negative' },
  { id: 'VR2+', name: 'VR2+', capabilities: ['vr_input'], voltage: 'VR', description: 'VR input 2 positive (Cam)' },
  { id: 'VR2-', name: 'VR2-', capabilities: ['vr_input'], voltage: 'VR', description: 'VR input 2 negative' },
  
  // CAN bus
  { id: 'CANH', name: 'CAN-H', capabilities: ['can_bus'], voltage: '5V', description: 'CAN bus high' },
  { id: 'CANL', name: 'CAN-L', capabilities: ['can_bus'], voltage: '5V', description: 'CAN bus low' },
];

// Function assignments (what logical functions can be assigned)
interface FunctionAssignment {
  id: string;
  name: string;
  function: PinFunction;
  channel: number;
  required: boolean;
  description: string;
}

const FUNCTION_ASSIGNMENTS: FunctionAssignment[] = [
  // Injectors
  { id: 'inj1', name: 'Injector 1', function: 'injector', channel: 1, required: true, description: 'Fuel injector 1' },
  { id: 'inj2', name: 'Injector 2', function: 'injector', channel: 2, required: false, description: 'Fuel injector 2' },
  { id: 'inj3', name: 'Injector 3', function: 'injector', channel: 3, required: false, description: 'Fuel injector 3' },
  { id: 'inj4', name: 'Injector 4', function: 'injector', channel: 4, required: false, description: 'Fuel injector 4' },
  { id: 'inj5', name: 'Injector 5', function: 'injector', channel: 5, required: false, description: 'Fuel injector 5' },
  { id: 'inj6', name: 'Injector 6', function: 'injector', channel: 6, required: false, description: 'Fuel injector 6' },
  { id: 'inj7', name: 'Injector 7', function: 'injector', channel: 7, required: false, description: 'Fuel injector 7' },
  { id: 'inj8', name: 'Injector 8', function: 'injector', channel: 8, required: false, description: 'Fuel injector 8' },
  
  // Ignition
  { id: 'ign1', name: 'Ignition 1', function: 'ignition', channel: 1, required: true, description: 'Ignition coil 1' },
  { id: 'ign2', name: 'Ignition 2', function: 'ignition', channel: 2, required: false, description: 'Ignition coil 2' },
  { id: 'ign3', name: 'Ignition 3', function: 'ignition', channel: 3, required: false, description: 'Ignition coil 3' },
  { id: 'ign4', name: 'Ignition 4', function: 'ignition', channel: 4, required: false, description: 'Ignition coil 4' },
  { id: 'ign5', name: 'Ignition 5', function: 'ignition', channel: 5, required: false, description: 'Ignition coil 5' },
  { id: 'ign6', name: 'Ignition 6', function: 'ignition', channel: 6, required: false, description: 'Ignition coil 6' },
  { id: 'ign7', name: 'Ignition 7', function: 'ignition', channel: 7, required: false, description: 'Ignition coil 7' },
  { id: 'ign8', name: 'Ignition 8', function: 'ignition', channel: 8, required: false, description: 'Ignition coil 8' },
  
  // Auxiliary outputs
  { id: 'aux1', name: 'Aux Out 1', function: 'aux_output', channel: 1, required: false, description: 'Auxiliary output 1 (Fuel pump)' },
  { id: 'aux2', name: 'Aux Out 2', function: 'aux_output', channel: 2, required: false, description: 'Auxiliary output 2 (Fan)' },
  { id: 'aux3', name: 'Aux Out 3', function: 'aux_output', channel: 3, required: false, description: 'Auxiliary output 3' },
  { id: 'aux4', name: 'Aux Out 4', function: 'aux_output', channel: 4, required: false, description: 'Auxiliary output 4' },
  
  // Analog inputs
  { id: 'tps', name: 'TPS', function: 'analog_input', channel: 0, required: true, description: 'Throttle position sensor' },
  { id: 'map', name: 'MAP', function: 'analog_input', channel: 1, required: true, description: 'Manifold absolute pressure' },
  { id: 'clt', name: 'CLT', function: 'analog_input', channel: 2, required: true, description: 'Coolant temperature' },
  { id: 'iat', name: 'IAT', function: 'analog_input', channel: 3, required: true, description: 'Intake air temperature' },
  { id: 'o2', name: 'O2 Sensor', function: 'analog_input', channel: 4, required: false, description: 'Oxygen sensor' },
  { id: 'batt', name: 'Battery', function: 'analog_input', channel: 5, required: true, description: 'Battery voltage' },
  
  // VR inputs
  { id: 'crank', name: 'Crank', function: 'vr_input', channel: 1, required: true, description: 'Crankshaft position sensor' },
  { id: 'cam', name: 'Cam', function: 'vr_input', channel: 2, required: false, description: 'Camshaft position sensor' },
];

// Get color for function type
function getFunctionColor(func: PinFunction): string {
  switch (func) {
    case 'injector': return '#22c55e';
    case 'ignition': return '#f59e0b';
    case 'aux_output': return '#3b82f6';
    case 'analog_input': return '#8b5cf6';
    case 'digital_input': return '#06b6d4';
    case 'vr_input': return '#ec4899';
    case 'can_bus': return '#6366f1';
    default: return '#666';
  }
}

export default function PortEditor({
  ecuType = 'Generic ECU',
  initialConfig = [],
  onSave,
  onCancel,
}: PortEditorProps) {
  const [hardwarePins] = useState<HardwarePin[]>(DEFAULT_HARDWARE_PINS);
  const [assignments, setAssignments] = useState<Map<string, string>>(new Map());
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set(['injectors', 'ignition', 'inputs']));
  const [_selectedPin, setSelectedPin] = useState<string | null>(null);
  const [searchFilter, setSearchFilter] = useState('');
  const [hasChanges, setHasChanges] = useState(false);

  // Initialize assignments from initial config
  useState(() => {
    if (initialConfig.length > 0) {
      const map = new Map<string, string>();
      initialConfig.forEach(pin => {
        const assignment = FUNCTION_ASSIGNMENTS.find(
          a => a.function === pin.function && a.channel === pin.channel
        );
        if (assignment) {
          map.set(assignment.id, pin.physicalPin);
        }
      });
      setAssignments(map);
    }
  });

  // Group functions by type
  const functionGroups = useMemo(() => [
    {
      id: 'injectors',
      name: 'Injectors',
      icon: <Flame size={16} />,
      assignments: FUNCTION_ASSIGNMENTS.filter(a => a.function === 'injector'),
    },
    {
      id: 'ignition',
      name: 'Ignition',
      icon: <Zap size={16} />,
      assignments: FUNCTION_ASSIGNMENTS.filter(a => a.function === 'ignition'),
    },
    {
      id: 'aux_outputs',
      name: 'Auxiliary Outputs',
      icon: <ToggleLeft size={16} />,
      assignments: FUNCTION_ASSIGNMENTS.filter(a => a.function === 'aux_output'),
    },
    {
      id: 'inputs',
      name: 'Inputs',
      icon: <Gauge size={16} />,
      assignments: FUNCTION_ASSIGNMENTS.filter(a => 
        a.function === 'analog_input' || a.function === 'digital_input' || a.function === 'vr_input'
      ),
    },
  ], []);

  // Toggle group expansion
  const toggleGroup = useCallback((groupId: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  }, []);

  // Get available pins for a function type
  const getAvailablePins = useCallback((func: PinFunction): HardwarePin[] => {
    return hardwarePins.filter(pin => 
      pin.capabilities.includes(func) &&
      !Array.from(assignments.values()).includes(pin.id)
    );
  }, [hardwarePins, assignments]);

  // Assign a pin to a function
  const assignPin = useCallback((functionId: string, pinId: string | null) => {
    setAssignments(prev => {
      const next = new Map(prev);
      if (pinId) {
        // Remove any existing assignment for this pin
        for (const [key, value] of next.entries()) {
          if (value === pinId && key !== functionId) {
            next.delete(key);
          }
        }
        next.set(functionId, pinId);
      } else {
        next.delete(functionId);
      }
      return next;
    });
    setHasChanges(true);
  }, []);

  // Reset to defaults
  const handleReset = useCallback(() => {
    setAssignments(new Map());
    setHasChanges(false);
  }, []);

  // Save configuration
  const handleSave = useCallback(() => {
    const config: PinConfig[] = [];
    
    for (const [functionId, pinId] of assignments.entries()) {
      const assignment = FUNCTION_ASSIGNMENTS.find(a => a.id === functionId);
      const pin = hardwarePins.find(p => p.id === pinId);
      
      if (assignment && pin) {
        config.push({
          id: functionId,
          name: assignment.name,
          physicalPin: pinId,
          function: assignment.function,
          channel: assignment.channel,
          inverted: false,
          pullup: false,
          description: assignment.description,
        });
      }
    }
    
    onSave?.(config);
  }, [assignments, hardwarePins, onSave]);

  // Check for warnings
  const warnings = useMemo(() => {
    const warns: string[] = [];
    
    // Check for unassigned required functions
    FUNCTION_ASSIGNMENTS.filter(a => a.required).forEach(assignment => {
      if (!assignments.has(assignment.id)) {
        warns.push(`${assignment.name} is not assigned`);
      }
    });
    
    return warns;
  }, [assignments]);

  // Filter assignments by search
  const filteredGroups = useMemo(() => {
    if (!searchFilter) return functionGroups;
    
    const filter = searchFilter.toLowerCase();
    return functionGroups.map(group => ({
      ...group,
      assignments: group.assignments.filter(a => 
        a.name.toLowerCase().includes(filter) ||
        a.description.toLowerCase().includes(filter)
      ),
    })).filter(g => g.assignments.length > 0);
  }, [functionGroups, searchFilter]);

  return (
    <div className="port-editor">
      {/* Header */}
      <div className="port-editor-header">
        <div className="header-title">
          <Cpu size={24} />
          <div>
            <h2>Pin Configuration</h2>
            <span className="ecu-type">{ecuType}</span>
          </div>
        </div>
        <div className="header-actions">
          <button className="btn-secondary" onClick={handleReset} disabled={!hasChanges}>
            <RotateCcw size={14} />
            Reset
          </button>
          <button className="btn-primary" onClick={handleSave}>
            <Save size={14} />
            Save Configuration
          </button>
        </div>
      </div>

      {/* Warnings */}
      {warnings.length > 0 && (
        <div className="warnings-banner">
          <AlertTriangle size={16} />
          <div className="warnings-list">
            {warnings.slice(0, 3).map((warn, i) => (
              <span key={i}>{warn}</span>
            ))}
            {warnings.length > 3 && (
              <span className="more">+{warnings.length - 3} more</span>
            )}
          </div>
        </div>
      )}

      {/* Main content */}
      <div className="port-editor-content">
        {/* Left panel: Function assignments */}
        <div className="assignments-panel">
          <div className="search-box">
            <Search size={14} />
            <input
              type="text"
              placeholder="Search functions..."
              value={searchFilter}
              onChange={(e) => setSearchFilter(e.target.value)}
            />
          </div>

          <div className="function-groups">
            {filteredGroups.map(group => (
              <div key={group.id} className="function-group">
                <div 
                  className="group-header"
                  onClick={() => toggleGroup(group.id)}
                >
                  {expandedGroups.has(group.id) ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                  {group.icon}
                  <span className="group-name">{group.name}</span>
                  <span className="group-count">{group.assignments.length}</span>
                </div>
                
                {expandedGroups.has(group.id) && (
                  <div className="group-assignments">
                    {group.assignments.map(assignment => {
                      const assignedPin = assignments.get(assignment.id);
                      const availablePins = getAvailablePins(assignment.function);
                      
                      return (
                        <div 
                          key={assignment.id} 
                          className={`assignment-row ${assignment.required ? 'required' : ''} ${assignedPin ? 'assigned' : 'unassigned'}`}
                        >
                          <div className="assignment-info">
                            <span 
                              className="function-indicator"
                              style={{ backgroundColor: getFunctionColor(assignment.function) }}
                            />
                            <span className="assignment-name">{assignment.name}</span>
                            {assignment.required && <span className="required-badge">Required</span>}
                          </div>
                          
                          <select
                            value={assignedPin || ''}
                            onChange={(e) => assignPin(assignment.id, e.target.value || null)}
                            className={assignedPin ? 'has-value' : ''}
                          >
                            <option value="">Not Assigned</option>
                            {assignedPin && (
                              <option value={assignedPin}>
                                {hardwarePins.find(p => p.id === assignedPin)?.name || assignedPin}
                              </option>
                            )}
                            {availablePins.map(pin => (
                              <option key={pin.id} value={pin.id}>
                                {pin.name}
                              </option>
                            ))}
                          </select>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Right panel: Pin diagram */}
        <div className="pinout-panel">
          <h3>Hardware Pinout</h3>
          
          <div className="pinout-diagram">
            {/* Organize pins by category */}
            <div className="pin-section">
              <h4>High-Side Outputs (12V)</h4>
              <div className="pin-grid">
                {hardwarePins.filter(p => p.id.startsWith('HSO')).map(pin => {
                  const assignedFunc = Array.from(assignments.entries()).find(([_, v]) => v === pin.id);
                  const assignment = assignedFunc ? FUNCTION_ASSIGNMENTS.find(a => a.id === assignedFunc[0]) : null;
                  
                  return (
                    <div 
                      key={pin.id} 
                      className={`pin-box ${assignment ? 'assigned' : 'available'}`}
                      style={{ borderColor: assignment ? getFunctionColor(assignment.function) : undefined }}
                      onClick={() => setSelectedPin(pin.id)}
                    >
                      <span className="pin-id">{pin.id}</span>
                      {assignment && (
                        <span 
                          className="pin-function"
                          style={{ color: getFunctionColor(assignment.function) }}
                        >
                          {assignment.name}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="pin-section">
              <h4>Low-Side Outputs (5V)</h4>
              <div className="pin-grid">
                {hardwarePins.filter(p => p.id.startsWith('LSO')).map(pin => {
                  const assignedFunc = Array.from(assignments.entries()).find(([_, v]) => v === pin.id);
                  const assignment = assignedFunc ? FUNCTION_ASSIGNMENTS.find(a => a.id === assignedFunc[0]) : null;
                  
                  return (
                    <div 
                      key={pin.id} 
                      className={`pin-box ${assignment ? 'assigned' : 'available'}`}
                      style={{ borderColor: assignment ? getFunctionColor(assignment.function) : undefined }}
                    >
                      <span className="pin-id">{pin.id}</span>
                      {assignment && (
                        <span 
                          className="pin-function"
                          style={{ color: getFunctionColor(assignment.function) }}
                        >
                          {assignment.name}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="pin-section">
              <h4>Analog Inputs</h4>
              <div className="pin-grid">
                {hardwarePins.filter(p => p.id.startsWith('ADC')).map(pin => {
                  const assignedFunc = Array.from(assignments.entries()).find(([_, v]) => v === pin.id);
                  const assignment = assignedFunc ? FUNCTION_ASSIGNMENTS.find(a => a.id === assignedFunc[0]) : null;
                  
                  return (
                    <div 
                      key={pin.id} 
                      className={`pin-box ${assignment ? 'assigned' : 'available'}`}
                      style={{ borderColor: assignment ? getFunctionColor(assignment.function) : undefined }}
                    >
                      <span className="pin-id">{pin.id}</span>
                      {assignment && (
                        <span 
                          className="pin-function"
                          style={{ color: getFunctionColor(assignment.function) }}
                        >
                          {assignment.name}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="pin-section">
              <h4>Digital & VR Inputs</h4>
              <div className="pin-grid">
                {hardwarePins.filter(p => p.id.startsWith('DIN') || p.id.startsWith('VR')).map(pin => {
                  const assignedFunc = Array.from(assignments.entries()).find(([_, v]) => v === pin.id);
                  const assignment = assignedFunc ? FUNCTION_ASSIGNMENTS.find(a => a.id === assignedFunc[0]) : null;
                  
                  return (
                    <div 
                      key={pin.id} 
                      className={`pin-box ${assignment ? 'assigned' : 'available'}`}
                      style={{ borderColor: assignment ? getFunctionColor(assignment.function) : undefined }}
                    >
                      <span className="pin-id">{pin.id}</span>
                      {assignment && (
                        <span 
                          className="pin-function"
                          style={{ color: getFunctionColor(assignment.function) }}
                        >
                          {assignment.name}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>

          {/* Legend */}
          <div className="pinout-legend">
            <h4>Legend</h4>
            <div className="legend-items">
              <div className="legend-item">
                <span className="legend-color" style={{ backgroundColor: getFunctionColor('injector') }} />
                <span>Injector</span>
              </div>
              <div className="legend-item">
                <span className="legend-color" style={{ backgroundColor: getFunctionColor('ignition') }} />
                <span>Ignition</span>
              </div>
              <div className="legend-item">
                <span className="legend-color" style={{ backgroundColor: getFunctionColor('aux_output') }} />
                <span>Aux Output</span>
              </div>
              <div className="legend-item">
                <span className="legend-color" style={{ backgroundColor: getFunctionColor('analog_input') }} />
                <span>Analog Input</span>
              </div>
              <div className="legend-item">
                <span className="legend-color" style={{ backgroundColor: getFunctionColor('vr_input') }} />
                <span>VR Input</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Footer */}
      <div className="port-editor-footer">
        <div className="status">
          {hasChanges ? (
            <span className="unsaved">Unsaved changes</span>
          ) : (
            <span className="saved"><Check size={14} /> Configuration saved</span>
          )}
        </div>
        <button className="btn-cancel" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}
