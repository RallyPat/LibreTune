import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Plus, Trash2, Save, X, Calculator, AlertCircle, CheckCircle } from 'lucide-react';
import './MathChannelsDialog.css';

interface UserMathChannel {
  name: string;
  units: string;
  expression: string;
}

interface MathChannelsDialogProps {
  onClose: () => void;
}

export default function MathChannelsDialog({ onClose }: MathChannelsDialogProps) {
  const [channels, setChannels] = useState<UserMathChannel[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null); // Use name as ID
  
  // Form state
  const [name, setName] = useState('');
  const [units, setUnits] = useState('');
  const [expression, setExpression] = useState('');
  
  const [validationMsg, setValidationMsg] = useState<string | null>(null);
  const [isValid, setIsValid] = useState(false);
  const [isDirty, setIsDirty] = useState(false);

  useEffect(() => {
    loadChannels();
  }, []);

  // Debounced validation
  useEffect(() => {
    if (!expression.trim()) {
        setIsValid(false);
        setValidationMsg(null);
        return;
    }
    // Don't validate if we just loaded the form and haven't touched it (unless it's new)
    if (editingId && !isDirty && editingId !== '__NEW__') {
        const current = channels.find(c => c.name === editingId);
        if (current && current.expression === expression) {
            setIsValid(true);
            return;
        }
    }

    const timer = setTimeout(() => validate(expression), 500);
    return () => clearTimeout(timer);
  }, [expression, editingId]);

  const loadChannels = async () => {
    try {
      const result = await invoke<UserMathChannel[]>('get_math_channels');
      setChannels(result);
    } catch (err) {
      console.error('Failed to load channels:', err);
    }
  };

  const resetForm = () => {
    setName('');
    setUnits('');
    setExpression('');
    setEditingId(null);
    setValidationMsg(null);
    setIsValid(false);
    setIsDirty(false);
  };

  const handleSelect = (channel: UserMathChannel) => {
    setEditingId(channel.name);
    setName(channel.name);
    setUnits(channel.units);
    setExpression(channel.expression);
    setValidationMsg(null);
    setIsValid(true); // Assume saved channels are valid
    setIsDirty(false);
  };

  const handleNew = () => {
    resetForm();
    setName('NewChannel');
    setEditingId('__NEW__'); 
    setIsDirty(true);
  };

  const handleDelete = async (targetName: string) => {
    if (!confirm(`Delete channel "${targetName}"?`)) return;
    
    try {
      await invoke('delete_math_channel', { name: targetName });
      await loadChannels();
      if (editingId === targetName) {
        resetForm();
      }
    } catch (err) {
      alert(`Failed to delete: ${err}`);
    }
  };

  const validate = async (expr: string) => {
    try {
      await invoke('validate_math_expression', { expr });
      setValidationMsg("Valid expression");
      setIsValid(true);
    } catch (err) {
      setValidationMsg(String(err));
      setIsValid(false);
    }
  };

  const handleSave = async () => {
    if (!name.trim() || !isValid) return;

    const channel: UserMathChannel = {
      name: name.trim(),
      units: units.trim(),
      expression: expression.trim()
    };

    try {
      await invoke('set_math_channel', { channel });
      await loadChannels();
      setEditingId(channel.name); // Switch to edit mode for the saved channel
      setIsDirty(false);
    } catch (err) {
      alert(`Failed to saving: ${err}`);
    }
  };

  return (
    <div className="math-channels-dialog">
      <div className="math-channels-content">
        <div className="math-channels-header">
          <h2><Calculator size={20} /> Math Channels</h2>
          <button className="btn-icon" onClick={onClose}><X size={20} /></button>
        </div>
        
        <div className="math-channels-body">
          <div className="channels-list">
            <div className="channels-list-header">
              <h3>Defined Channels</h3>
              <button className="btn-add" onClick={handleNew} title="Add New Channel">
                <Plus size={18} />
              </button>
            </div>
            <div className="channels-list-items">
              {channels.map(c => (
                <div 
                  key={c.name} 
                  className={`channel-item ${editingId === c.name ? 'active' : ''}`}
                  onClick={() => handleSelect(c)}
                >
                  <span className="channel-name">{c.name}</span>
                  <span className="channel-expr">{c.expression}</span>
                </div>
              ))}
              {channels.length === 0 && (
                <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '0.9rem' }}>
                  No custom channels defined
                </div>
              )}
            </div>
          </div>

          <div className="channel-editor">
            {!editingId ? (
              <div className="editor-placeholder">
                <Calculator size={48} />
                <p>Select a channel to edit or create a new one</p>
                <button className="btn btn-primary" onClick={handleNew}>Create New Channel</button>
              </div>
            ) : (
              <div className="editor-form">
                <div className="form-row">
                  <div className="form-group" style={{ flex: 2 }}>
                    <label>Channel Name</label>
                    <input 
                      value={name} 
                      onChange={e => { setName(e.target.value); setIsDirty(true); }}
                      placeholder="e.g. Boost_PSI"
                      // Allow rename if it's new, otherwise simplistic lock for now
                      disabled={editingId !== '__NEW__' && editingId !== name} 
                    />
                  </div>
                  <div className="form-group" style={{ flex: 1 }}>
                    <label>Units</label>
                    <input 
                      value={units} 
                      onChange={e => { setUnits(e.target.value); setIsDirty(true); }}
                      placeholder="psi"
                    />
                  </div>
                </div>

                <div className="form-group">
                  <label>Expression</label>
                  <input 
                    className="font-mono" 
                    value={expression} 
                    onChange={e => { setExpression(e.target.value); setIsDirty(true); }}
                    placeholder="(map - 100) * 0.145"
                  />
                </div>

                {expression && (
                  <div className={`validation-status ${isValid ? 'valid' : 'invalid'}`}>
                    {isValid ? <CheckCircle size={16} /> : <AlertCircle size={16} />}
                    {validationMsg || "Checking..."}
                  </div>
                )}

                <div className="editor-actions">
                  <button 
                    className="btn btn-primary" 
                    onClick={handleSave} 
                    disabled={!isValid || !name || !isDirty}
                  >
                    <Save size={16} />
                    Save Channel
                  </button>
                  {editingId !== '__NEW__' && (
                    <button 
                      className="btn btn-danger" 
                      onClick={() => handleDelete(editingId)}
                      style={{ marginLeft: 'auto' }}
                    >
                      <Trash2 size={16} />
                      Delete
                    </button>
                  )}
                </div>
                
                <div className="formula-help">
                    <h4>Formula Reference</h4>
                    <ul>
                        <li>Use channel names: <code>rpm</code>, <code>map</code>, <code>tps</code>, <code>clt</code></li>
                        <li>Basic operators: <code>+</code>, <code>-</code>, <code>*</code>, <code>/</code>, <code>%</code></li>
                        <li>Comparison: <code>&gt;</code>, <code>&lt;</code>, <code>==</code> (returns 1.0 or 0.0)</li>
                        <li>Functions: <code>sin(x)</code>, <code>cos(x)</code>, <code>min(a,b)</code>, <code>max(a,b)</code></li>
                        <li>Logic: <code>(conditions) ? 1 : 0</code> for simple logic</li>
                    </ul>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
