/**
 * SwingRenderer - Renders Swing component trees as React components.
 */

import React, { useState, useCallback, useMemo } from 'react';
import { SwingComponent, GridBagConstraint } from './types';
import { getLayoutContainerStyle, getLayoutChildStyle, calculateGridBagTemplate } from './LayoutTranslator';
import { eventBridge } from './EventBridge';
import './SwingRenderer.css';

interface SwingRendererProps {
  pluginId: string;
  component: SwingComponent;
  className?: string;
}

/**
 * Main component that renders a Swing component tree.
 */
export const SwingRenderer: React.FC<SwingRendererProps> = ({ pluginId, component, className }) => {
  return (
    <div className={`swing-renderer ${className || ''}`}>
      <SwingComponentRenderer pluginId={pluginId} component={component} parentLayout={undefined} />
    </div>
  );
};

interface SwingComponentRendererProps {
  pluginId: string;
  component: SwingComponent;
  parentLayout: SwingComponent['layout'];
}

/**
 * Recursive renderer for individual Swing components.
 */
const SwingComponentRenderer: React.FC<SwingComponentRendererProps> = ({ 
  pluginId, 
  component, 
  parentLayout 
}) => {
  const { id, componentType, properties, layout, layoutConstraint, children } = component;

  // Get styles based on parent layout constraints
  const childStyle = useMemo(
    () => getLayoutChildStyle(parentLayout, layoutConstraint),
    [parentLayout, layoutConstraint]
  );

  // Render based on component type
  const renderComponent = () => {
    switch (componentType) {
      case 'JPanel':
      case 'JScrollPane':
      case 'Box':
        return renderContainer();
      case 'JButton':
        return renderButton();
      case 'JLabel':
        return renderLabel();
      case 'JTextField':
        return renderTextField();
      case 'JTextArea':
        return renderTextArea();
      case 'JCheckBox':
        return renderCheckBox();
      case 'JRadioButton':
        return renderRadioButton();
      case 'JComboBox':
        return renderComboBox();
      case 'JSlider':
        return renderSlider();
      case 'JProgressBar':
        return renderProgressBar();
      case 'JTabbedPane':
        return renderTabbedPane();
      case 'JTable':
        return renderTable();
      case 'JSeparator':
        return <hr className="swing-separator" style={childStyle} />;
      case 'Box.Filler':
      case 'Filler':
        return <div className="swing-filler" style={{ ...childStyle, flex: 1 }} />;
      default:
        // Fallback: render as container if it has children, otherwise as div
        if (children && children.length > 0) {
          return renderContainer();
        }
        return (
          <div className="swing-unknown" style={childStyle} title={`Unknown: ${componentType}`}>
            {componentType}
          </div>
        );
    }
  };

  // Container rendering (JPanel, JScrollPane, etc.)
  const renderContainer = () => {
    const containerStyle: React.CSSProperties = {
      ...childStyle,
      ...getLayoutContainerStyle(layout),
    };

    // For GridBagLayout, calculate grid template
    if (layout?.type === 'GridBagLayout' && children) {
      const gridInfo = calculateGridBagTemplate(
        children.map(c => ({ constraint: c.layoutConstraint as GridBagConstraint }))
      );
      containerStyle.gridTemplateColumns = gridInfo.columns;
      containerStyle.gridTemplateRows = gridInfo.rows;
    }

    return (
      <div className={`swing-container swing-${componentType.toLowerCase()}`} style={containerStyle}>
        {children?.map(child => (
          <SwingComponentRenderer
            key={child.id}
            pluginId={pluginId}
            component={child}
            parentLayout={layout}
          />
        ))}
      </div>
    );
  };

  // Button rendering
  const renderButton = () => {
    const handleClick = useCallback(() => {
      eventBridge.sendAction(pluginId, id);
    }, []);

    return (
      <button
        className="swing-button"
        style={childStyle}
        onClick={handleClick}
        disabled={!properties.enabled}
        title={properties.tooltip as string}
      >
        {properties.text as string}
      </button>
    );
  };

  // Label rendering
  const renderLabel = () => {
    return (
      <span 
        className="swing-label" 
        style={childStyle}
        title={properties.tooltip as string}
      >
        {properties.text as string}
      </span>
    );
  };

  // TextField rendering
  const renderTextField = () => {
    const [value, setValue] = useState(properties.text as string || '');

    const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
      setValue(e.target.value);
    }, []);

    const handleBlur = useCallback(() => {
      eventBridge.sendTextChange(pluginId, id, value);
    }, [value]);

    return (
      <input
        type="text"
        className="swing-textfield"
        style={childStyle}
        value={value}
        onChange={handleChange}
        onBlur={handleBlur}
        disabled={!properties.enabled}
        readOnly={!properties.editable}
        size={properties.columns as number}
        title={properties.tooltip as string}
      />
    );
  };

  // TextArea rendering
  const renderTextArea = () => {
    const [value, setValue] = useState(properties.text as string || '');

    const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setValue(e.target.value);
    }, []);

    const handleBlur = useCallback(() => {
      eventBridge.sendTextChange(pluginId, id, value);
    }, [value]);

    return (
      <textarea
        className="swing-textarea"
        style={childStyle}
        value={value}
        onChange={handleChange}
        onBlur={handleBlur}
        disabled={!properties.enabled}
        readOnly={!properties.editable}
        rows={properties.rows as number}
        cols={properties.columns as number}
        title={properties.tooltip as string}
      />
    );
  };

  // CheckBox rendering
  const renderCheckBox = () => {
    const [checked, setChecked] = useState(properties.selected as boolean || false);

    const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
      const newChecked = e.target.checked;
      setChecked(newChecked);
      eventBridge.sendStateChange(pluginId, id, newChecked);
    }, []);

    return (
      <label className="swing-checkbox" style={childStyle}>
        <input
          type="checkbox"
          checked={checked}
          onChange={handleChange}
          disabled={!properties.enabled}
        />
        <span>{properties.text as string}</span>
      </label>
    );
  };

  // RadioButton rendering
  const renderRadioButton = () => {
    const [checked, setChecked] = useState(properties.selected as boolean || false);

    const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
      const newChecked = e.target.checked;
      setChecked(newChecked);
      eventBridge.sendStateChange(pluginId, id, newChecked);
    }, []);

    return (
      <label className="swing-radio" style={childStyle}>
        <input
          type="radio"
          checked={checked}
          onChange={handleChange}
          disabled={!properties.enabled}
        />
        <span>{properties.text as string}</span>
      </label>
    );
  };

  // ComboBox rendering
  const renderComboBox = () => {
    const items = (properties.items as string[]) || [];
    const [selectedIndex, setSelectedIndex] = useState(properties.selectedIndex as number || 0);

    const handleChange = useCallback((e: React.ChangeEvent<HTMLSelectElement>) => {
      const index = parseInt(e.target.value, 10);
      setSelectedIndex(index);
      eventBridge.sendItemSelect(pluginId, id, index, items[index] || '');
    }, [items]);

    return (
      <select
        className="swing-combobox"
        style={childStyle}
        value={selectedIndex}
        onChange={handleChange}
        disabled={!properties.enabled}
        title={properties.tooltip as string}
      >
        {items.map((item, index) => (
          <option key={index} value={index}>
            {item}
          </option>
        ))}
      </select>
    );
  };

  // Slider rendering
  const renderSlider = () => {
    const min = (properties.min as number) || 0;
    const max = (properties.max as number) || 100;
    const [value, setValue] = useState((properties.value as number) || min);

    const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
      const newValue = parseInt(e.target.value, 10);
      setValue(newValue);
      eventBridge.sendSliderChange(pluginId, id, newValue);
    }, []);

    return (
      <div className="swing-slider" style={childStyle}>
        <input
          type="range"
          min={min}
          max={max}
          value={value}
          onChange={handleChange}
          disabled={!properties.enabled}
        />
        <span className="swing-slider-value">{value}</span>
      </div>
    );
  };

  // ProgressBar rendering
  const renderProgressBar = () => {
    const max = (properties.max as number) || 100;
    const value = (properties.value as number) || 0;
    const indeterminate = properties.indeterminate as boolean;

    return (
      <div className="swing-progressbar" style={childStyle}>
        <progress
          value={indeterminate ? undefined : value}
          max={max}
          className={indeterminate ? 'indeterminate' : ''}
        />
      </div>
    );
  };

  // TabbedPane rendering
  const renderTabbedPane = () => {
    const tabTitles = (properties.tabTitles as string[]) || [];
    const [selectedTab, setSelectedTab] = useState((properties.selectedIndex as number) || 0);

    return (
      <div className="swing-tabbedpane" style={childStyle}>
        <div className="swing-tab-header">
          {tabTitles.map((title, index) => (
            <button
              key={index}
              className={`swing-tab ${index === selectedTab ? 'active' : ''}`}
              onClick={() => setSelectedTab(index)}
            >
              {title}
            </button>
          ))}
        </div>
        <div className="swing-tab-content">
          {children && children[selectedTab] && (
            <SwingComponentRenderer
              pluginId={pluginId}
              component={children[selectedTab]}
              parentLayout={undefined}
            />
          )}
        </div>
      </div>
    );
  };

  // Table rendering
  const renderTable = () => {
    const columnNames = (properties.columnNames as string[]) || [];
    const data = (properties.data as unknown[][]) || [];
    const [selectedRows, setSelectedRows] = useState<number[]>([]);

    const handleRowClick = useCallback((rowIndex: number, event: React.MouseEvent) => {
      let newSelection: number[];
      if (event.ctrlKey || event.metaKey) {
        // Toggle selection
        if (selectedRows.includes(rowIndex)) {
          newSelection = selectedRows.filter(r => r !== rowIndex);
        } else {
          newSelection = [...selectedRows, rowIndex];
        }
      } else {
        newSelection = [rowIndex];
      }
      setSelectedRows(newSelection);
      eventBridge.sendTableSelect(pluginId, id, newSelection);
    }, [selectedRows]);

    return (
      <div className="swing-table-container" style={childStyle}>
        <table className="swing-table">
          <thead>
            <tr>
              {columnNames.map((name, index) => (
                <th key={index}>{name}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.map((row, rowIndex) => (
              <tr
                key={rowIndex}
                className={selectedRows.includes(rowIndex) ? 'selected' : ''}
                onClick={(e) => handleRowClick(rowIndex, e)}
              >
                {row.map((cell, cellIndex) => (
                  <td key={cellIndex}>{String(cell)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  };

  return renderComponent();
};

export default SwingRenderer;
